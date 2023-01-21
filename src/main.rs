// If using the `binstart` feature of `esp-idf-sys`, always keep this module
// imported
use anyhow::Context;
use eclss::{bme680, http, net, scd30};
use esp_idf_hal::{
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    prelude::*,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, log::EspLogger, mdns::EspMdns, nvs::EspDefaultNvsPartition,
    sntp::EspSntp,
};
use esp_idf_sys as _;

static METRICS: eclss::SensorMetrics = eclss::SensorMetrics::new();

// apparently Rust tasks need more stack size than the default on ESP32C3
const STACK_SIZE: usize = 7000;

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
    let sysloop = EspSystemEventLoop::take().context("failed to initialize system event loop")?;
    let nvs =
        EspDefaultNvsPartition::take().context("failed to initialize non-volatile storage")?;
    let mut mdns = EspMdns::take().context("failed to initialize mDNS")?;

    let mut wifi = net::EclssWifi::new(peripherals.modem, &sysloop, nvs)?;
    net::init_mdns(&mut mdns)?;

    let server = http::start_server(wifi.access_points.clone(), &METRICS)?;

    // Maximal I2C speed is 100 kHz and the master has to support clock
    // stretching. Sensirion recommends to operate the SCD30
    // at a baud rate of 50 kHz or smaller.
    let config = I2cConfig::new().baudrate(50u32.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config)?;
    let bus = shared_bus::new_std!(I2cDriver = i2c).unwrap();

    // bring up sensors
    // TODO(eliza): use the sensors to calibrate each other...
    let scd30 = scd30::bringup(&bus).context("bringing up SCD30 failed");
    let bme680 = bme680::bringup(&bus).context("bringing up BME680 failed");

    let scd30_started = scd30.and_then(|sensor| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn(move || scd30::run(sensor, &METRICS))
            .context("spawning SCD30 driver thread failed")
    });
    if let Err(error) = scd30_started {
        log::error!("failed to start SCD30: {error}");
    } else {
        // has_sensors = true;
    }

    let bme680_started = bme680.and_then(|sensor| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn(move || bme680::run(sensor, &METRICS))
            .context("spawning BME680 driver thread failed")
    });
    if let Err(error) = bme680_started {
        log::error!("failed to start BME680: {error}");
    } else {
        // has_sensors = true;
    }

    // if !has_sensors {
    //     log::error!("/!\\ EXTREMELY TRAGIC ERROR ... NO SENSORS BROUGHT UP SUCCESSFULLY!")
    // }

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
