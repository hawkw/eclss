use super::Sensor;
use crate::{metrics::{self, Gauge, SensorLabel, SensorMetrics}, I2cRef, I2cBus};
use std::num::Wrapping;

pub struct Ens160 {
    sensor: ens160::Ens160<I2cRef<'static>>,
    eco2_gauge: &'static Gauge,
    tvoc_gauge: &'static Gauge,
    rel_humidity: &'static tinymetrics::GaugeFamily<'static, { metrics::MAX_METRICS }, SensorLabel>,
    temp: &'static tinymetrics::GaugeFamily<'static, { metrics::MAX_METRICS }, SensorLabel>,
    polls: Wrapping<usize>,
}

const NAME: &str = "ENS160";

impl Ens160 {
    fn calibrate(&mut self) -> anyhow::Result<()> {
        // TODO(eliza): add averaging functions...
        let avg_temp =            { let mut count = 0;
        let sum: f32 = self.temp.metrics().iter().map(|(_, gauge)| {
            count += 1;
            gauge.value() as f32
        }).sum();
        sum / count as f32
    };
    let avg_hum =            { let mut count = 0;
        let sum: f32 = self.rel_humidity.metrics().iter().map(|(_, gauge)| {
            count += 1;
            gauge.value() as f32
        }).sum();
        sum / count as f32
    };
    if avg_temp <= 0.0 && avg_hum <= 0.0 {
        return Ok(());
    }
    self.sensor.set_temp_and_hum(avg_temp, avg_hum).map_err(|error| anyhow::anyhow!("error calibrating {NAME}: {error:?}"))?;
    log::info!(target: NAME, "calibrated {NAME} to {avg_temp:.2}°C, {avg_hum:.2}% RH");
    Ok(())
}

}

impl Sensor for Ens160 {
    type ControlMessage = ();

    const NAME: &'static str = NAME;

    fn bringup(busman: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self> {
        // i2c address of the Adafruit breakout board
        const ADDR: u8 = 0x53;

        log::info!(target: NAME, "connecting to {NAME} (addr={ADDR:#x})...");
        let i2c = busman.acquire_i2c();
        let mut sensor = ens160::Ens160::new(i2c, ADDR);
        let part_id = sensor.get_part_id().map_err(|error| anyhow::anyhow!("error reading {NAME} part ID: {error:?}"))?;
        let status = sensor.get_status().map_err(|error| anyhow::anyhow!("error reading {NAME} status: {error:?}"))?;
        log::info!(target: NAME, "connected to {NAME}! part ID: {part_id:#x}, status: {status:#?}");

        let mut this = Self {
            sensor,
            eco2_gauge: metrics.eco2.register(Self::LABEL).unwrap(),
            tvoc_gauge: metrics.tvoc.register(Self::LABEL).unwrap(),
            rel_humidity: &metrics.abs_humidity,
            temp: &metrics.temp,
            polls: Wrapping(0),
        };
        this.calibrate()?;
        Ok(this)

    }

    fn poll(&mut self) -> anyhow::Result<()> {
        let status = self.sensor.get_status().map_err(|error| anyhow::anyhow!("error reading {NAME} status: {error:?}"))?;
        self.polls += 1;

        // if !status.running_normally() {
        //     return Err(anyhow::anyhow!("ENS160 is not running normally! status: {status:#?}"))
        // }

        if !status.data_is_ready() {
            log::info!(target: NAME, "no data yet (status={status:?})...");
            return Ok(());
        }

        if self.polls.0 % 5 == 0 {
            self.calibrate()?;
        }

        let tvoc = self.sensor.get_tvoc().map_err(|error| anyhow::anyhow!("error reading {NAME} TVOC: {error:?}"))?;
        let eco2 = *self.sensor.get_eco2().map_err(|error| anyhow::anyhow!("error reading {NAME} eCO2: {error:?}"))?;
        let aqi = self.sensor.get_airquality_index().map_err(|error| anyhow::anyhow!("error reading {NAME} AQI: {error:?}"))?;

        log::info!(target: NAME, "eCO2: {eco2:>4} ppm, tVOC: {tvoc:>3} ppb, AQI: {aqi:?}");
        if let ens160::Validity::NormalOperation = status.validity_flag() {
            self.tvoc_gauge.set_value(tvoc.into());
            self.eco2_gauge.set_value(eco2.into());
        } else {
            log::info!(target: NAME, "data is invalid! status: {status:#?}");
        }

        Ok(())
    }

    fn poll_interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(2)
    }

    fn handle_control_message(&mut self, _: &Self::ControlMessage) -> anyhow::Result<()> {
        // TODO(eliza): calibrate w using control msgs?
        anyhow::bail!("not yet implemented")
    }
}
