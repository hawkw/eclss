use crate::{
    metrics::{Gauge, SensorLabel},
    sensor::Sensor,
    I2cBus, I2cRef, SensorMetrics,
};
use anyhow::anyhow;
use embassy_time::Duration;
use esp_idf_hal::delay::Ets;
use std::{num::Wrapping, time::Instant};
use tinymetrics::GaugeFamily;

pub struct Sgp30 {
    sensor: sgp30::Sgp30<I2cRef<'static>, Ets>,
    eco2_gauge: &'static Gauge,
    tvoc_gauge: &'static Gauge,
    abs_humidity: &'static GaugeFamily<'static, 4, SensorLabel>,
    started_at: Instant,
    polls: Wrapping<usize>,
    init: bool,
}

const NAME: &'static str = "SGP30";

impl Sensor for Sgp30 {
    type ControlMessage = ();

    const NAME: &'static str = NAME;

    fn init(busman: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self> {
        // the adafruit breakout board has this I2C address.
        const ADDR: u8 = 0x58;

        log::info!(target: NAME, "connecting to {NAME}...");
        let i2c = busman.acquire_i2c();
        let mut sensor = sgp30::Sgp30::new(i2c, ADDR, Ets);

        let version = sensor
            .get_feature_set()
            .map_err(|error| anyhow!("failed to get {NAME} feature set: {error:?}"))?;
        log::info!(target: NAME, "connected to {NAME}: version: {version:?}");

        // run the self-test
        let selftest = sensor
            .selftest()
            .map_err(|error| anyhow!("failed to run {NAME} self-test: {error:?}"))?;
        if !selftest {
            anyhow::bail!("{NAME} self-test failed");
        }

        // initialize the sensor.
        sensor
            .init()
            .map_err(|error| anyhow!("failed to initialize {NAME}: {error:?}"))?;

        Ok(Self {
            sensor,
            eco2_gauge: metrics.eco2.register(Self::LABEL).unwrap(),
            tvoc_gauge: metrics.tvoc.register(Self::LABEL).unwrap(),
            abs_humidity: &metrics.abs_humidity,
            polls: Wrapping(0),
            started_at: Instant::now(),
            init: true,
        })
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        let sgp30::Measurement {
            tvoc_ppb,
            co2eq_ppm,
        } = self
            .sensor
            .measure()
            .map_err(|error| anyhow!("failed to read {NAME} measurement: {error:?}"))?;
        self.polls += 1;

        // the SGP30 has a 15-second initialization phase after startup, during which it
        // calibrates itself. while the sensor is initializing, all measurements will
        // read 400 ppm eCO2 and 0 ppb tVOC. we don't want to report these values, so
        // track how long has elapsed since the sensor has initialized, and throw out
        // measurements until the init phase is done.
        if self.init {
            let elapsed = self.started_at.elapsed();
            log::info!(target: NAME, "in init phase for {elapsed:?} ({} measurements)...", self.polls);
            // ignore the measurement until we have exited the
            // initialization phase
            if self.polls.0 > 15 {
                self.init = false;
            } else {
                return Ok(());
            }
        }

        log::info!(target: NAME, "eCO2: {co2eq_ppm:>4} ppm, tVOC: {tvoc_ppb:>3} ppb");

        self.eco2_gauge.set_value(co2eq_ppm as f64);
        self.tvoc_gauge.set_value(tvoc_ppb as f64);

        if self.polls.0 % (crate::units::ABS_HUMIDITY_INTERVAL * 2) != 0 {
            return Ok(());
        }

        let humidity = {
            let mut count = 0;
            let sum: f32 = self
                .abs_humidity
                .metrics()
                .iter()
                .map(|(_, gauge)| {
                    count += 1;
                    gauge.value() as f32
                })
                .sum();
            sum / count as f32
        };

        if humidity == 0.0 {
            // no humidity readings yet...
            return Ok(());
        }

        match sgp30::Humidity::from_f32(humidity) {
            Ok(val) => {
                self.sensor.set_humidity(Some(&val)).map_err(|error| {
                    anyhow!(
                        "failed to set {NAME} absolute humidity to {humidity:3.2} g/𝑚³: {error:?}"
                    )
                })?;
                log::debug!(target: NAME, "updated absolute humidity to {humidity:3.2} g/𝑚³");
            }
            Err(err) => {
                log::warn!(target: NAME, "error converting absolute humidity {humidity:3.2} g/𝑚³ to fixpoint: {err:?}")
            }
        }

        Ok(())
    }

    fn poll_interval(&self) -> Duration {
        // per https://docs.rs/sgp30/latest/sgp30/index.html#doing-measurements,
        // the sensor MUST be polled every second, or else its dynamic baseline
        // calibration thingy gets messed up. per the datasheet, doing a reading
        // takes 12 ms, so the actual poll interval is less than 1 second.
        Duration::from_secs(1) - Duration::from_millis(12)
    }

    fn handle_control_message(&mut self, _: &Self::ControlMessage) -> anyhow::Result<()> {
        // TODO(eliza): calibrate with absolute humidity?
        anyhow::bail!("not yet implemented")
    }
}
