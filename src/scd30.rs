use crate::{sensor::Sensor, I2cBus, I2cRef, SensorMetrics};
use anyhow::anyhow;
use esp_idf_hal::{delay::Ets, i2c::I2cError};

pub type Scd30 = sensor_scd30::Scd30<I2cRef<'static>, Ets, I2cError>;

impl Sensor for Scd30 {
    const NAME: &'static str = "SCD30";

    fn bringup(busman: &'static I2cBus) -> anyhow::Result<Self> {
        log::debug!("connecting to SCD30");

        let i2c = busman.acquire_i2c();
        let mut scd30 = sensor_scd30::Scd30::new(i2c, Ets)
            .map_err(|error| anyhow!("failed to connect to SCD30: {error:?}"))?;

        let firmware = scd30
            .firmware_version()
            .map_err(|error| anyhow!("failed to read SCD30 firmware version: {error:?}"))?;
        log::info!("connected to SCD30; firmware: {firmware}");
        // println!("reset: {:?}", scd30.soft_reset());
        // retry(10, || scd30.set_afc(false)).expect("failed to enable automatic calibration mode");
        // println!("enabled SCD30 automatic calibration");

        scd30
            .set_measurement_interval(2)
            .map_err(|error| anyhow!("failed to set SCD30 measurement interval: {error:?}"))?;
        log::info!("set SCD30 measurement interval to 2");
        scd30.start_continuous(0).map_err(|error| {
            anyhow!("failed to start SCD30 continuous sampling mode: {error:?}")
        })?; // TODO(eliza): figure out pressure compensation.
        log::info!("enabled SCD30 continuous sampling mode");

        Ok(scd30)
    }

    fn poll(&mut self, metrics: &SensorMetrics) -> anyhow::Result<()> {
        // Keep looping until ready
        while !self
            .data_ready()
            .map_err(|err| anyhow::anyhow!("error waiting for data: {err:?}"))?
        {}

        // Fetch data when available
        let sensor_scd30::Measurement { co2, temp, rh } = self
            .read_data()
            .map_err(|err| anyhow::anyhow!("error reading data: {err:?}"))?;
        metrics.co2.sensors().set_value(co2);
        metrics.humidity.sensors().scd30.set_value(rh);
        metrics.temp.sensors().scd30.set_value(temp);
        log::info!(
            "[SCD30] CO2: {co2:>8.3} ppm, Temp: {temp:>3.3} \u{00B0}C, Humidity: {rh:>3.3}%"
        );

        Ok(())
    }

    fn incr_error(metrics: &SensorMetrics) {
        metrics.sensor_errors.sensors().scd30.incr();
    }
}
