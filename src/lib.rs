#![feature(type_alias_impl_trait)]
#![doc = include_str!("../docs/README.md")]
pub mod actor;
pub mod atomic;
pub mod bme680;
pub mod http;
pub mod metric;
pub mod net;
pub mod registry;
pub mod retry;
pub mod scd30;
pub mod sensor;
pub mod ws2812;

pub type I2cRef<'bus> = shared_bus::I2cProxy<'bus, SharedI2c>;
pub type I2cBus = shared_bus::BusManager<SharedI2c>;
pub type SharedI2c = std::sync::Mutex<esp_idf_hal::i2c::I2cDriver<'static>>;

use std::fmt;

#[derive(Debug)]
pub struct SensorMetrics {
    pub temp: metric::GaugeFamily<'static, 4>,
    pub co2: metric::GaugeFamily<'static, 4>,
    pub humidity: metric::GaugeFamily<'static, 4>,
    pub pressure: metric::GaugeFamily<'static, 4>,
    pub gas_resistance: metric::GaugeFamily<'static, 4>,
    pub sensor_errors: metric::CounterFamily<'static, 4>,
}

impl SensorMetrics {
    pub const fn new() -> Self {
        Self {
            temp: metric::MetricDef::new("temperature_degrees_celcius")
                .with_help("Temperature in degrees Celcius.")
                .with_unit("celcius")
                .with_metrics(),
            co2: metric::MetricDef::new("co2_ppm")
                .with_help("CO2 in parts per million (ppm).")
                .with_unit("ppm")
                .with_metrics(),

            humidity: metric::MetricDef::new("humidity_percent")
                .with_help("Relative humidity (RH) percentage.")
                .with_unit("percent")
                .with_metrics(),
            pressure: metric::MetricDef::new("pressure_hpa")
                .with_help("Barometric pressure, in hectopascals (hPa).")
                .with_unit("hPa")
                .with_metrics(),
            gas_resistance: metric::MetricDef::new("gas_resistance_ohms")
                .with_help("BME680 VOC sensor resistance, in Ohms.")
                .with_unit("Ohms")
                .with_metrics(),
            sensor_errors: metric::MetricDef::new("sensor_error_count")
                .with_help("Count of I2C errors that occurred while talking to a sensor")
                .with_metrics(),
        }
    }

    pub fn fmt_metrics(&self, f: &mut impl fmt::Write) -> fmt::Result {
        self.temp.fmt_metric(f)?;
        self.co2.fmt_metric(f)?;
        self.humidity.fmt_metric(f)?;
        self.pressure.fmt_metric(f)?;
        self.gas_resistance.fmt_metric(f)?;
        self.sensor_errors.fmt_metric(f)?;
        Ok(())
    }
}

impl serde::Serialize for SensorMetrics {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("SensorMetrics", 5)?;
        state.serialize_field("temp", &self.temp.metrics())?;
        state.serialize_field("co2", &self.co2.metrics())?;
        state.serialize_field("humidity", &self.humidity.metrics())?;
        state.serialize_field("pressure", &self.pressure.metrics())?;
        state.serialize_field("gas_resistance", &self.gas_resistance.metrics())?;
        state.end()
    }
}

impl fmt::Display for SensorMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_metrics(f)
    }
}
