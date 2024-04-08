#![feature(type_alias_impl_trait)]
#![doc = include_str!("../docs/README.md")]
pub mod actor;
pub mod http;
pub mod metrics;
pub mod net;

pub mod retry;

pub mod sensor;

pub mod units;
pub mod ws2812;

// === sensors ===

#[cfg(feature = "sensor-bme680")]
pub mod bme680;
#[cfg(feature = "sensor-pmsa003i")]
pub mod pmsa003i;
#[cfg(feature = "sensor-scd30")]
pub mod scd30;
#[cfg(feature = "sensor-sgp30")]
pub mod sgp30;

pub type I2cRef<'bus> = shared_bus::I2cProxy<'bus, SharedI2c>;
pub type I2cBus = shared_bus::BusManager<SharedI2c>;
pub type SharedI2c = std::sync::Mutex<esp_idf_hal::i2c::I2cDriver<'static>>;

pub use metrics::SensorMetrics;
