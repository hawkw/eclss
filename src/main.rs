// If using the `binstart` feature of `esp-idf-sys`, always keep this module
// imported
use anyhow::Context;
use eclss::{actor, http, net, sensor, ws2812};
use embassy_time::Duration;
use esp_idf_hal::{
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    prelude::*,
    reset::WakeupReason,
    task,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop, log::EspLogger, mdns::EspMdns, nvs::EspDefaultNvsPartition,
    sntp::EspSntp,
};
use esp_idf_sys as _;

static METRICS: eclss::SensorMetrics = eclss::SensorMetrics::new();

// Make sure that the firmware will contain
// up-to-date build time and package info coming from the binary crate
esp_idf_sys::esp_app_desc!();

#[cfg(not(any(
    feature = "sensor-bme680",
    feature = "sensor-pmsa003i",
    feature = "sensor-sgp30",
    feature = "sensor-scd30",
)))]
compile_error!(
    "compiling without any of the 'sensor-*' features enabled is probably not very useful!"
);

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the
    // runtime implemented by esp-idf-sys might not link properly. See
    // https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();
    esp_idf_hal::task::critical_section::link();
    esp_idf_svc::timer::embassy_time::driver::link();
    esp_idf_svc::timer::embassy_time::queue::link();

    // let logger = EspLogger;
    EspLogger::initialize_default();
    // logger.set_target_level("eclss", log::LevelFilter::Debug);
    // logger.set_target_level("", log::LevelFilter::Info);
    // logger.initialize();

    let wakeup = WakeupReason::get();
    log::info!("Wakeup reason: {wakeup:?}");
    log::info!("ECLSS is go!");

    let peripherals = Peripherals::take().unwrap();
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio5;
    let scl = peripherals.pins.gpio6;
    // QT Py C3 neopixel is on GPIO 2
    let mut neopixel = ws2812::NeoPixel::new(peripherals.pins.gpio2, peripherals.rmt.channel0)?;
    neopixel.set_color(255, 0, 0).context("set neopixel red")?;

    let _sntp = EspSntp::new_default().context("failed to initialize SNTP")?;
    let mut sysloop =
        EspSystemEventLoop::take().context("failed to initialize system event loop")?;
    let nvs =
        EspDefaultNvsPartition::take().context("failed to initialize non-volatile storage")?;
    let mut mdns = EspMdns::take().context("failed to initialize mDNS")?;

    let wifi = net::EclssWifi::new(peripherals.modem, &mut sysloop, nvs)?;
    net::init_mdns(&mut mdns)?;

    let (scd30_ctrl, scd30_rx) = actor::channel(10);

    let _server = http::start_server(&wifi, &METRICS, scd30_ctrl)?;

    // Maximal I2C speed is 100 kHz and the master has to support clock
    // stretching. Sensirion recommends to operate the SCD30
    // at a baud rate of 50 kHz or smaller.
    let config = I2cConfig::new().baudrate(50u32.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config)?;
    let bus = shared_bus::new_std!(I2cDriver = i2c).unwrap();

    // bring up sensors
    // TODO(eliza): use the sensors to calibrate each other...
    let sensor_mangler = sensor::Manager {
        metrics: &METRICS,
        busman: bus,
        retry_backoff: Duration::from_secs(1),
    };

    let exec: task::executor::EspExecutor<8, edge_executor::Local> =
        task::executor::EspExecutor::new();
    let mut tasks = heapless::Vec::new();
    exec.spawn_local_collect(wifi.run(sysloop.clone(), neopixel), &mut tasks)
        .context("failed to spawn wifi bg task")?;

    #[cfg(feature = "sensor-scd30")]
    exec.spawn_local_collect(sensor_mangler.run::<scd30::Scd30>(scd30_rx), &mut tasks)
        .context("failed to spawn SCD30 task")?;

    #[cfg(feature = "sensor-pmsa003i")]
    let _pmsa003i_control = {
        let (tx, rx) = actor::channel(10);
        exec.spawn_local_collect(sensor_mangler.run::<pmsa003i::Pmsa003i>(rx), &mut tasks)
            .context("failed to spawn PMSA003I task")?;
        tx
    };

    #[cfg(feature = "sensor-bme680")]
    let _bme680_control = {
        let (tx, rx) = actor::channel(10);
        exec.spawn_local_collect(sensor_mangler.run::<bme680::Bme680>(rx), &mut tasks)
            .context("failed to spawn BME680 task")?;
        tx
    };

    #[cfg(feature = "sensor-sgp30")]
    let _sgp30_control = {
        let (tx, rx) = actor::channel(10);
        exec.spawn_local_collect(sensor_mangler.run::<sgp30::Sgp30>(rx), &mut tasks)
            .context("failed to spawn SGP30 task")?;
        tx
    };

    exec.run_tasks(|| true, &mut tasks);
    Ok(())
}
