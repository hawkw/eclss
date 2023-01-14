use crate::{I2cBus, I2cRef};
use anyhow::anyhow;
use esp_idf_hal::{delay::Ets, i2c::I2cError};

pub type Sensor<'bus> = sensor_scd30::Scd30<I2cRef<'bus>, Ets, I2cError>;
pub type Error = sensor_scd30::Error<I2cError>;

pub fn bringup<'bus>(busman: &'bus I2cBus) -> anyhow::Result<Sensor<'bus>> {
    log::debug!("connecting to SCD30");
    let mut scd30 = retry(10, || {
        let i2c = busman.acquire_i2c();
        sensor_scd30::Scd30::new(i2c, Ets)
    })
    .map_err(|error| anyhow!("failed to connect to SCD30: {error:?}"))?;

    let firmware = retry(10, || scd30.firmware_version())
        .map_err(|error| anyhow!("failed to read SCD30 firmware version: {error:?}"))?;
    log::info!("connected to SCD30; firmware: {firmware}");
    // println!("reset: {:?}", scd30.soft_reset());
    // retry(10, || scd30.set_afc(false)).expect("failed to enable automatic calibration mode");
    // println!("enabled SCD30 automatic calibration");

    retry(10, || scd30.set_measurement_interval(2))
        .map_err(|error| anyhow!("failed to set SCD30 measurement interval: {error:?}"))?;
    log::info!("set SCD30 measurement interval to 2");
    retry(10, || scd30.start_continuous(0))
        .map_err(|error| anyhow!("failed to start SCD30 continuous sampling mode: {error:?}"))?; // TODO(eliza): figure out pressure compensation.
    log::info!("enabled SCD30 continuous sampling mode");

    Ok(scd30)
}

fn retry<T>(mut retries: usize, mut f: impl FnMut() -> Result<T, Error>) -> Result<T, Error> {
    loop {
        match f() {
            Ok(val) => return Ok(val),
            Err(sensor_scd30::Error::NoDevice) => return Err(sensor_scd30::Error::NoDevice),
            Err(error) if retries == 0 => return Err(error),
            Err(error) => {
                retries -= 1;
                log::warn!("SCD30 retrying: {error:?} ({retries} retries remaining)");
            }
        }
    }
}
