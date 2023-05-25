#![feature(type_alias_impl_trait)]
#![doc = include_str!("../docs/README.md")]
pub mod actor;
pub mod bme680;
pub mod http;
pub mod metrics;
pub mod net;
// pub mod pmsa003i;
pub mod retry;
pub mod scd30;
pub mod sensor;
pub mod ws2812;

pub type I2cRef<'bus> = shared_bus::I2cProxy<'bus, SharedI2c>;
pub type I2cBus = shared_bus::BusManager<SharedI2c>;
pub type SharedI2c = std::sync::Mutex<esp_idf_hal::i2c::I2cDriver<'static>>;

pub use metrics::SensorMetrics;
