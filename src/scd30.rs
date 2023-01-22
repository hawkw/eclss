use crate::{I2cBus, I2cRef, Retry, SensorMetrics};
use anyhow::anyhow;
use esp_idf_hal::{delay::Ets, i2c::I2cError};

pub type Sensor<'bus> = sensor_scd30::Scd30<I2cRef<'bus>, Ets, I2cError>;
pub type Error = sensor_scd30::Error<I2cError>;

pub fn bringup<'bus>(busman: &'bus I2cBus) -> anyhow::Result<Sensor<'bus>> {
    log::debug!("connecting to SCD30");
    let retry = Retry::new(10)
        .with_target("eclss::scd30")
        .with_predicate(|error| {
            // if the device is not connected, don't retry.
            !matches!(error, sensor_scd30::Error::NoDevice)
        });

    let mut scd30 = retry
        .run(|| {
            let i2c = busman.acquire_i2c();
            sensor_scd30::Scd30::new(i2c, Ets)
        })
        .map_err(|error| anyhow!("failed to connect to SCD30: {error:?}"))?;

    let firmware = retry
        .run(|| scd30.firmware_version())
        .map_err(|error| anyhow!("failed to read SCD30 firmware version: {error:?}"))?;
    log::info!("connected to SCD30; firmware: {firmware}");
    // println!("reset: {:?}", scd30.soft_reset());
    // retry(10, || scd30.set_afc(false)).expect("failed to enable automatic calibration mode");
    // println!("enabled SCD30 automatic calibration");

    retry
        .run(|| scd30.set_measurement_interval(2))
        .map_err(|error| anyhow!("failed to set SCD30 measurement interval: {error:?}"))?;
    log::info!("set SCD30 measurement interval to 2");
    retry
        .run(|| scd30.start_continuous(0))
        .map_err(|error| anyhow!("failed to start SCD30 continuous sampling mode: {error:?}"))?; // TODO(eliza): figure out pressure compensation.
    log::info!("enabled SCD30 continuous sampling mode");

    Ok(scd30)
}

pub async fn run(
    mut scd30: Sensor<'static>,
    metrics: &'static SensorMetrics,
) -> anyhow::Result<()> {
    loop {
        // Keep looping until ready
        match scd30.data_ready() {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                log::debug!("error waiting for SCD30 to become ready: {error:?}");
                continue;
            }
        }

        // Fetch data when available
        match scd30.read_data() {
            Err(error) => log::debug!("error reading from SCD30: {error:?}"),
            Ok(sensor_scd30::Measurement { co2, temp, rh }) => {
                metrics.co2.sensors().set_value(co2);
                metrics.humidity.sensors().scd30.set_value(rh);
                metrics.temp.sensors().scd30.set_value(temp);
                log::info!("[SCD30] CO2: {co2:>8.3} ppm, Temp: {temp:>3.3} \u{00B0}C, Humidity: {rh:>3.3}%")
            }
        }

        // if we've read data from the sensor, wait for 2 seconds before reading
        // again
        embassy_time::Timer::after(embassy_time::Duration::from_secs(2)).await;
    }
}
