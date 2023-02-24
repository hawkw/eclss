#![feature(type_alias_impl_trait)]
#![doc = include_str!("../docs/README.md")]
pub mod actor;
pub mod atomic;
pub mod bme680;
pub mod http;
pub mod metric;
pub mod net;
pub mod retry;
pub mod scd30;
pub mod sensor;
pub mod ws2812;

pub type I2cRef<'bus> = shared_bus::I2cProxy<'bus, SharedI2c>;
pub type I2cBus = shared_bus::BusManager<SharedI2c>;
pub type SharedI2c = std::sync::Mutex<esp_idf_hal::i2c::I2cDriver<'static>>;
pub use self::retry::Retry;

use embedded_svc::io;

#[derive(Debug)]
pub struct SensorMetrics {
    pub temp: metric::Metric<'static, BothTemps>,
    pub co2: metric::Metric<'static, metric::SensorGauge<'static>>,
    pub humidity: metric::Metric<'static, BothTemps>,
    pub pressure: metric::Metric<'static, metric::SensorGauge<'static>>,
    pub gas_resistance: metric::Metric<'static, metric::SensorGauge<'static>>,
    pub sensor_errors: metric::Metric<'static, BothErrors>,
}

#[derive(Debug, serde::Serialize)]
pub struct BothTemps {
    pub bme680: metric::SensorGauge<'static>,
    pub scd30: metric::SensorGauge<'static>,
}
#[derive(Debug, serde::Serialize)]
pub struct BothErrors {
    pub bme680: metric::SensorCounter<'static>,
    pub scd30: metric::SensorCounter<'static>,
}

const SCD30: &str = "SCD30";
const BME680: &str = "BME680";
const SHT31: &str = "SHT31";

impl SensorMetrics {
    pub const fn new() -> Self {
        Self {
            temp: metric::MetricDef::new("temperature_degrees_celcius")
                .with_help("Temperature in degrees Celcius.")
                .with_unit("celcius")
                .with_sensors(BothTemps::new()),
            co2: metric::MetricDef::new("co2_ppm")
                .with_help("CO2 in parts per million (ppm).")
                .with_unit("ppm")
                .with_sensors(metric::SensorGauge::new(SCD30)),

            humidity: metric::MetricDef::new("humidity_percent")
                .with_help("Relative humidity (RH) percentage.")
                .with_unit("percent")
                .with_sensors(BothTemps::new()),
            pressure: metric::MetricDef::new("pressure_hpa")
                .with_help("Barometric pressure, in hectopascals (hPa).")
                .with_unit("hPa")
                .with_sensors(metric::SensorGauge::new(BME680)),
            gas_resistance: metric::MetricDef::new("gas_resistance_ohms")
                .with_help("BME680 VOC sensor resistance, in Ohms.")
                .with_unit("Ohms")
                .with_sensors(metric::SensorGauge::new(BME680)),
            sensor_errors: metric::MetricDef::new("sensor_error_count")
                .with_help("Count of I2C errors that occurred while talking to a sensor")
                .with_sensors(BothErrors::new()),
        }
    }

    pub fn render_prometheus<W: io::Write>(
        &self,
        writer: &mut W,
    ) -> Result<(), io::WriteFmtError<W::Error>> {
        self.temp.render_prometheus(writer)?;
        self.co2.render_prometheus(writer)?;
        self.humidity.render_prometheus(writer)?;
        self.pressure.render_prometheus(writer)?;
        self.gas_resistance.render_prometheus(writer)?;
        self.sensor_errors.render_prometheus(writer)?;
        Ok(())
    }
}

impl serde::Serialize for SensorMetrics {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("SensorMetrics", 5)?;
        state.serialize_field("temp", &self.temp.sensors())?;
        state.serialize_field("co2", &self.co2.sensors())?;
        state.serialize_field("humidity", &self.humidity.sensors())?;
        state.serialize_field("pressure", &self.pressure.sensors())?;
        state.serialize_field("gas_resistance", &self.gas_resistance.sensors())?;
        state.end()
    }
}

// === impl BothTemps ===

impl BothTemps {
    pub const fn new() -> Self {
        Self {
            bme680: metric::SensorGauge::new(BME680),
            scd30: metric::SensorGauge::new(SHT31),
        }
    }
}

impl<'a> IntoIterator for &'a BothTemps {
    type Item = &'a metric::SensorGauge<'static>;
    type IntoIter = std::iter::Chain<
        std::iter::Once<&'a metric::SensorGauge<'static>>,
        std::iter::Once<&'a metric::SensorGauge<'static>>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.bme680).chain(std::iter::once(&self.scd30))
    }
}

// === impl BothErrors ===

impl BothErrors {
    pub const fn new() -> Self {
        Self {
            bme680: metric::SensorCounter::new(BME680),
            scd30: metric::SensorCounter::new(SCD30),
        }
    }

    pub fn named(&self, name: &'static str) -> Option<&metric::SensorCounter<'static>> {
        match name {
            BME680 => Some(&self.bme680),
            SCD30 => Some(&self.scd30),
            _ => None,
        }
    }
}

impl<'a> IntoIterator for &'a BothErrors {
    type Item = &'a metric::SensorCounter<'static>;
    type IntoIter = std::iter::Chain<
        std::iter::Once<&'a metric::SensorCounter<'static>>,
        std::iter::Once<&'a metric::SensorCounter<'static>>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.bme680).chain(std::iter::once(&self.scd30))
    }
}
