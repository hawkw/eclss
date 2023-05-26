use crate::{metrics::Gauge, sensor::Sensor, I2cBus, I2cRef, SensorMetrics, units};
use esp_idf_hal::delay::Ets;
use std::num::Wrapping;

pub struct Bme680 {
    sensor: bosch_bme680::Bme680<I2cRef<'static>, Ets>,
    pressure_gauge: &'static Gauge,
    temp_gauge: &'static Gauge,
    rel_humidity_gauge: &'static Gauge,
    abs_humidity_gauge: &'static Gauge,
    gas_resistance_gauge: &'static Gauge,
    polls: Wrapping<usize>,
}

const NAME: &str = "BME680";

impl Sensor for Bme680 {
    type ControlMessage = ();

    const NAME: &'static str = NAME;

    fn bringup(busman: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self> {
        let config = bosch_bme680::Configuration::default();
        log::info!(target: NAME, "connecting to BME680 with config {config:#?}");
        let i2c = busman.acquire_i2c();
        let sensor = bosch_bme680::Bme680::new(
            i2c,
            // the default I2C address of the Adafruit BME680 breakout board
            // is the "secondary" address, 0x77.
            bosch_bme680::DeviceAddress::Secondary,
            Ets,
            &config,
            // TODO(eliza): can we get the ambient temperature from a SCD30 measurement?
            20,
        )
        .map_err(|error| anyhow::anyhow!("failed to connect to BME680: {error:?}"))?;

        Ok(Self {
            sensor,
            pressure_gauge: metrics
                .pressure
                .register(Self::LABEL)
                .expect("can't register"),
            temp_gauge: metrics.temp.register(Self::LABEL).expect("can't register"),
            rel_humidity_gauge: metrics
                .rel_humidity
                .register(Self::LABEL)
                .expect("can't register"),
            abs_humidity_gauge: metrics
                .abs_humidity
                .register(Self::LABEL)
                .expect("can't register"),
            gas_resistance_gauge: metrics
                .gas_resistance
                .register(Self::LABEL)
                .expect("can't register"),
            polls: Wrapping(0),
        })
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        let bosch_bme680::MeasurmentData {
            temperature,
            pressure,
            humidity,
            gas_resistance,
        } = self
            .sensor
            .measure()
            .map_err(|error| anyhow::anyhow!("error reading from BME680: {error:?}"))?;
        self.polls += 1;

        // pretty sure the `bosch-bme680` library is off by a factor of 100 when
        // representing pressures as hectopascals...
        let pressure = pressure / 100f32;

        log::info!(target: NAME, "Pressure: {pressure:>3.3} hPa, Temp: {temperature:>3.3} \
            \u{00B0}C, Rel. Humidity: ({humidity:>3.3}%)");
        self.pressure_gauge.set_value(pressure.into());
        self.temp_gauge.set_value(temperature.into());
        self.rel_humidity_gauge.set_value(humidity.into());

        if self.polls.0 % units::ABS_HUMIDITY_INTERVAL == 0 {
            let abs_humidity = units::absolute_humidity(temperature, humidity);
            self.abs_humidity_gauge.set_value(abs_humidity.into());
            log::info!(target: NAME, "Absolute Humidity: {abs_humidity:>3.3} g/𝑚³");
        }

        if let Some(gas) = gas_resistance {
            log::info!(target: NAME, "Gas resistance: {gas:>3.3} \u{2126}");
            self.gas_resistance_gauge.set_value(gas.into());
        }

        Ok(())
    }

    fn poll_interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(2)
    }

    fn handle_control_message(&mut self, _: &Self::ControlMessage) -> anyhow::Result<()> {
        // TODO(eliza): calibrate with ambient temp?
        anyhow::bail!("not yet implemented")
    }
}
