use crate::{metrics::Gauge, sensor::Sensor, I2cBus, I2cRef, SensorMetrics};
use esp_idf_hal::delay::Ets;
use anyhow::anyhow;
use std::time::Instant;
use embassy_time::Duration;

pub struct Sgp30 {
    sensor: sgp30::Sgp30<I2cRef<'static>, Ets>,
    co2_gauge: &'static Gauge,
    tvoc_gauge: &'static Gauge,
    started_at: Instant,
    init_measurements: usize,
}

const NAME: &'static str = "SGP30";
impl Sensor for Sgp30 {
    type ControlMessage = ();

    const NAME: &'static str = NAME;

    fn bringup(busman: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self> {
        // the adafruit breakout board has this I2C address.
        const ADDR: u8 = 0x58;

        log::info!("connecting to {NAME}...");
        let i2c = busman.acquire_i2c();
        let mut sensor = sgp30::Sgp30::new(i2c, ADDR, Ets);

        let version = sensor.get_feature_set().map_err(|error| anyhow!("failed to get {NAME} feature set: {error:?}"))?;
        log::info!("connected to {NAME}: version: {version:?}");

        // run the self-test
        let selftest = sensor.selftest().map_err(|error| anyhow!("failed to run {NAME} self-test: {error:?}"))?;
        if !selftest {
            anyhow::bail!("{NAME} self-test failed");
        }

        // initialize the sensor.
        sensor.init().map_err(|error| anyhow!("failed to initialize {NAME}: {error:?}"))?;

        Ok(Self {
            sensor,
            co2_gauge: metrics.co2.register(Self::LABEL).unwrap(),
            tvoc_gauge: metrics.tvoc.register(Self::LABEL).unwrap(),
            init_measurements: 0,
            started_at: Instant::now(),

        })
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        let sgp30::Measurement { tvoc_ppb, co2eq_ppm } = self.sensor.measure()
            .map_err(|error| anyhow!("failed to read {NAME} measurement: {error:?}"))?;

        // the SGP30 has a 15-second initialization phase after startup, during which it
        // calibrates itself. while the sensor is initializing, all measurements will
        // read 400 ppm eCO2 and 0 ppb tVOC. we don't want to report these values, so
        // track how long has elapsed since the sensor has initialized, and throw out
        // measurements until the init phase is done.
        if self.init_measurements <= 15 {
            let elapsed = self.started_at.elapsed();
            log::info!("[{NAME}] in init phase for {elapsed:?} ({} measurements)...", self.init_measurements);
            self.init_measurements += 1;
                // ignore the measurement until we have exited the
                // initialization phase
            return Ok(());
        }

        log::info!("[{NAME}] eCO2: {co2eq_ppm} ppm, tVOC: {tvoc_ppb} ppb");

        self.co2_gauge.set_value(co2eq_ppm as f64);
        self.tvoc_gauge.set_value(tvoc_ppb as f64);
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
