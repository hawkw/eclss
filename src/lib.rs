pub mod atomic;
pub mod bme680;
pub mod http;
pub mod metric;
mod retry;
pub mod scd30;
pub mod wifi;

pub type I2cRef<'bus> = shared_bus::I2cProxy<'bus, SharedI2c>;
pub type I2cBus = shared_bus::BusManager<SharedI2c>;
pub type SharedI2c = std::sync::Mutex<esp_idf_hal::i2c::I2cDriver<'static>>;
pub use self::retry::Retry;

use embedded_svc::io;

#[derive(Debug)]
pub struct SensorMetrics {
    pub temp: metric::Gauge<'static, BothTemps>,
    pub co2: metric::Gauge<'static, metric::SensorGauge<'static>>,
    pub humidity: metric::Gauge<'static, BothTemps>,
    pub pressure: metric::Gauge<'static, metric::SensorGauge<'static>>,
    pub gas_resistance: metric::Gauge<'static, metric::SensorGauge<'static>>,
}

#[derive(Debug)]
pub struct BothTemps {
    pub bme680: metric::SensorGauge<'static>,
    pub scd30: metric::SensorGauge<'static>,
}

const SCD30: &str = "SCD30";
const BME680: &str = "BME680";

impl SensorMetrics {
    pub const fn new() -> Self {
        Self {
            temp: metric::Gauge {
                name: "temperature_degrees_c",
                help: "Temperature in degrees Celcius.",
                sensors: BothTemps::new(),
            },
            co2: metric::Gauge {
                name: "co2_ppm",
                help: "CO2 in parts per million.",
                sensors: metric::SensorGauge::new(SCD30),
            },
            humidity: metric::Gauge {
                name: "humidity_percent",
                help: "Relative humidity (RH) percentage.",
                sensors: BothTemps::new(),
            },
            pressure: metric::Gauge {
                name: "pressure_hpa",
                help: "BME680 pressure, in hectopascals (hPa).",
                sensors: metric::SensorGauge::new(BME680),
            },
            gas_resistance: metric::Gauge {
                name: "gas_resistance_ohms",
                help: "BME680 VOC sensor resistance, in Ohms.",
                sensors: metric::SensorGauge::new(BME680),
            },
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
        Ok(())
    }
}

impl BothTemps {
    pub const fn new() -> Self {
        Self {
            bme680: metric::SensorGauge::new("BME680"),
            scd30: metric::SensorGauge::new("SCD30"),
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
