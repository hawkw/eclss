use crate::{I2cBus, I2cRef, Retry, SensorMetrics};
use esp_idf_hal::delay::Ets;
use std::{thread, time::Duration};

pub type Sensor<'bus> = bosch_bme680::Bme680<I2cRef<'bus>, Ets>;

pub fn bringup<'bus>(busman: &'bus I2cBus) -> anyhow::Result<Sensor<'bus>> {
    let config = bosch_bme680::Configuration::default();
    log::info!("connecting to BME680 with config {config:#?}");
    Retry::new(10)
        .with_target("eclss::bme680")
        .run(|| {
            let i2c = busman.acquire_i2c();
            bosch_bme680::Bme680::new(
                i2c,
                // the default I2C address of the Adafruit BME680 breakout board
                // is the "secondary" address, 0x77.
                bosch_bme680::DeviceAddress::Secondary,
                Ets,
                &config,
                // TODO(eliza): can we get the ambient temperature from a SCD30 measurement?
                20,
            )
        })
        .map_err(|error| anyhow::anyhow!("failed to connect to BME680: {error:?}"))
}

pub fn run(mut sensor: Sensor<'static>, metrics: &'static SensorMetrics) {
    thread::sleep(Duration::from_millis(100));

    loop {
        thread::sleep(Duration::from_secs(2));
        match sensor.measure() {
            Ok(bosch_bme680::MeasurmentData {
                temperature,
                pressure,
                humidity,
                gas_resistance,
            }) => {
                log::info!("[BME680]: Pressure: {pressure:>3.3} hPa, Temp: {temperature:>3.3} \u{00B0}C, Humidity: {humidity:>3.3}%");
                metrics.pressure.sensors().set_value(pressure);
                metrics.temp.sensors().bme680.set_value(temperature);
                metrics.humidity.sensors().bme680.set_value(humidity);
                if let Some(gas) = gas_resistance {
                    log::info!("[BME680]: Gas resistance: {gas:>3.3} \u{2126}");
                    metrics.gas_resistance.sensors().set_value(gas);
                }
            }
            Err(error) => {
                log::warn!("error reading from BME680: {error:?}");
                continue;
            }
        }
    }
}
