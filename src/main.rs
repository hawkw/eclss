// If using the `binstart` feature of `esp-idf-sys`, always keep this module
// imported
use anyhow::Context;
use eclss::scd30;
use esp_idf_hal::{
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    prelude::*,
};
use esp_idf_svc::log::EspLogger;
use esp_idf_sys as _;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the
    // runtime implemented by esp-idf-sys might not link properly. See
    // https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();

    EspLogger::initialize_default();

    log::info!("ECLSS is go!");

    let peripherals = Peripherals::take().unwrap();
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio5;
    let scl = peripherals.pins.gpio6;

    // Maximal I2C speed is 100 kHz and the master has to support clock
    // stretching. Sensirion recommends to operate the SCD30
    // at a baud rate of 50 kHz or smaller.
    let config = I2cConfig::new().baudrate(50u32.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config).context("constructing I2C driver")?;
    let bus = shared_bus::new_std!(I2cDriver = i2c).expect("bus manager is only initialized once!");

    let mut scd30 = scd30::bringup(&bus).context("bringing up SCD30")?;

    loop {
        // // mustn't forget to feed the doggy!
        // doggy0.feed();

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
                log::info!("CO2: {co2:>8.3} ppm, Temp: {temp:>3.3} \u{00B0}C, Humidity: {rh:>3.3}%")
            }
        }

        // if we've read data from the sensor, wait for 2 seconds before reading
        // again
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
