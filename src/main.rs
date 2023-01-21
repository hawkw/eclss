// If using the `binstart` feature of `esp-idf-sys`, always keep this module
// imported
use anyhow::Context;
use eclss::{http, scd30, wifi::EclssWifi};
use esp_idf_hal::{
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    prelude::*,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, log::EspLogger, nvs::EspDefaultNvsPartition, sntp::EspSntp,
};
use esp_idf_sys as _;

static METRICS: eclss::SensorMetrics = eclss::SensorMetrics::new();

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the
    // runtime implemented by esp-idf-sys might not link properly. See
    // https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();

    // let logger = EspLogger;
    EspLogger::initialize_default();
    // logger.set_target_level("eclss", log::LevelFilter::Debug);
    // logger.set_target_level("", log::LevelFilter::Info);
    // logger.initialize();

    log::info!("ECLSS is go!");

    let peripherals = Peripherals::take().unwrap();
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio5;
    let scl = peripherals.pins.gpio6;

    let _sntp = EspSntp::new_default().context("failed to initialize SNTP")?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi =
        EclssWifi::new(peripherals.modem, &sysloop, nvs).context("failed to bring up WiFi")?;

    let server = http::start_server(wifi.access_points.clone(), &METRICS)
        .context("failed to start HTTP server")?;

    // Maximal I2C speed is 100 kHz and the master has to support clock
    // stretching. Sensirion recommends to operate the SCD30
    // at a baud rate of 50 kHz or smaller.
    let config = I2cConfig::new().baudrate(50u32.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config).context("constructing I2C driver")?;
    let bus = shared_bus::new_std!(I2cDriver = i2c).expect("bus manager is only initialized once!");

    let scd30 = scd30::bringup(&bus).context("bringing up SCD30")?;
    let scd30_thread = std::thread::Builder::new()
        .stack_size(7000)
        .spawn(move || scd30::run(scd30, &METRICS));

    loop {
        if let Ok(creds) = server.wifi_credentials.recv() {
            log::info!("received WiFi credentials: {creds:?}");
            match wifi.connect_to(&sysloop, creds) {
                Ok(_) => log::info!("connected to WiFi access point"),
                Err(e) => log::error!("failed to connect to WiFi access point: {e}"),
            }
        }
    }
}
