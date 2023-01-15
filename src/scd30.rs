use crate::{I2cBus, I2cRef, SensorMetrics};
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

pub fn run(mut scd30: Sensor, metrics: &'static SensorMetrics) {
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
                metrics.scd30_co2.set_value(co2);
                metrics.scd30_humidity.set_value(rh);
                metrics.scd30_temp.set_value(temp);
                log::info!("CO2: {co2:>8.3} ppm, Temp: {temp:>3.3} \u{00B0}C, Humidity: {rh:>3.3}%")
            }
        }

        // if we've read data from the sensor, wait for 2 seconds before reading
        // again
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
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
