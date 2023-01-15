pub mod atomic;
pub mod metric;
pub mod scd30;
pub mod wifi;

pub type I2cRef<'bus> = shared_bus::I2cProxy<'bus, SharedI2c>;
pub type I2cBus = shared_bus::BusManager<SharedI2c>;
pub type SharedI2c = std::sync::Mutex<esp_idf_hal::i2c::I2cDriver<'static>>;

#[derive(Debug)]
pub struct SensorMetrics {
    pub scd30_temp: metric::Gauge<'static>,
    pub scd30_co2: metric::Gauge<'static>,
    pub scd30_humidity: metric::Gauge<'static>,
}

impl SensorMetrics {
    pub const fn new() -> Self {
        Self {
            scd30_temp: metric::Gauge::new("temperature_degrees_c")
                .with_help("SCD30 temperature in degrees Celcius."),
            scd30_co2: metric::Gauge::new("co2_ppm")
                .with_help("SCD30 CO2 concentration in parts per million."),
            scd30_humidity: metric::Gauge::new("scd30_humidity").with_help("SCD30 humidity"),
        }
    }

    pub fn render_prometheus(&self, writer: &mut impl std::fmt::Write) -> std::fmt::Result {
        self.scd30_temp.render_prometheus(writer)?;
        self.scd30_co2.render_prometheus(writer)?;
        self.scd30_humidity.render_prometheus(writer)?;
        Ok(())
    }
}
