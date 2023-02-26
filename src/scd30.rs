use crate::{
    metric::{self, Gauge},
    sensor::Sensor,
    I2cBus, I2cRef, SensorMetrics,
};
use anyhow::anyhow;
use esp_idf_hal::{delay::Ets, i2c::I2cError};

pub struct Scd30 {
    sensor: sensor_scd30::Scd30<I2cRef<'static>, Ets, I2cError>,
    measurement_interval_secs: u16,
    co2_gauge: &'static Gauge,
    temp_gauge: &'static Gauge,
    humidity_gauge: &'static Gauge,
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    /// Force calibrate the sensor to the given CO2 parts per million.
    ForceCalibrate {
        ppm: u16,
    },
    SetAltOffset(u16),
    /// Sets the sensor's measurement interval (in seconds).
    SetMeasurementInterval {
        secs: u16,
    },
    SoftReset,
}

impl Sensor for Scd30 {
    type ControlMessage = ControlMessage;

    const NAME: &'static str = "SCD30";
    const LABELS: metric::Labels<'static> = &[("sensor", "SCD30")];

    fn bringup(busman: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self> {
        const INITIAL_INTERVAL_SECS: u16 = 2;
        const SHT31: metric::Labels<'static> = &[("sensor", "SHT31")];

        log::debug!("connecting to SCD30");

        let i2c = busman.acquire_i2c();
        let mut sensor = sensor_scd30::Scd30::new(i2c, Ets)
            .map_err(|error| anyhow!("failed to connect to SCD30: {error:?}"))?;

        let firmware = sensor
            .firmware_version()
            .map_err(|error| anyhow!("failed to read SCD30 firmware version: {error:?}"))?;
        log::info!("connected to SCD30; firmware: {firmware}");

        sensor
            .set_measurement_interval(INITIAL_INTERVAL_SECS)
            .map_err(|error| anyhow!("failed to set SCD30 measurement interval: {error:?}"))?;
        log::info!("set SCD30 measurement interval to {INITIAL_INTERVAL_SECS} seconds");

        sensor.start_continuous(0).map_err(|error| {
            anyhow!("failed to start SCD30 continuous sampling mode: {error:?}")
        })?; // TODO(eliza): figure out pressure compensation.

        log::info!("enabled SCD30 continuous sampling mode");

        Ok(Self {
            sensor,
            measurement_interval_secs: INITIAL_INTERVAL_SECS,
            co2_gauge: metrics
                .co2
                .register(Self::LABELS)
                .expect("couldn't register gauge"),
            temp_gauge: metrics
                .temp
                .register(SHT31)
                .expect("couldn't register gauge"),
            humidity_gauge: metrics
                .humidity
                .register(SHT31)
                .expect("couldn't register gauge"),
        })
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        // Keep looping until ready
        while !self
            .sensor
            .data_ready()
            .map_err(|err| anyhow!("error waiting for data: {err:?}"))?
        {}

        // Fetch data when available
        let sensor_scd30::Measurement { co2, temp, rh } = self
            .sensor
            .read_data()
            .map_err(|err| anyhow!("error reading data: {err:?}"))?;
        self.co2_gauge.set_value(co2);
        self.humidity_gauge.set_value(rh);
        self.temp_gauge.set_value(temp);
        log::info!("CO2: {co2:>8.3} ppm, Temp: {temp:>3.3} \u{00B0}C, Humidity: {rh:>3.3}%");

        Ok(())
    }

    fn poll_interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(self.measurement_interval_secs as u64)
    }

    fn handle_control_message(&mut self, msg: &Self::ControlMessage) -> anyhow::Result<()> {
        match msg {
            &ControlMessage::ForceCalibrate { ppm } => {
                self.sensor.set_frc(ppm).map_err(|error| {
                    anyhow!("failed to recalibrate SCD30 to {ppm} ppm: {error:?}")
                })?;
                log::info!("recalibrated SCD30 at {ppm} ppm");
            }
            &ControlMessage::SetAltOffset(altitude) => {
                self.sensor.set_alt_offset(altitude).map_err(|error| {
                    anyhow!("failed to set SCD30 altitude offset to {altitude}: {error:?}")
                })?;
                log::info!("set altitude offset to {altitude}");
            }
            &ControlMessage::SetMeasurementInterval { secs } => {
                anyhow::ensure!(secs > 0);
                self.sensor
                    .set_measurement_interval(secs)
                    .map_err(|error| {
                        anyhow!(
                            "failed to set SCD30 measurement interval {secs} seconds: {error:?}"
                        )
                    })?;
                log::info!("set measurement interval to {secs} seconds");
            }
            ControlMessage::SoftReset => {
                self.sensor
                    .soft_reset()
                    .map_err(|error| anyhow!("failed to trigger SCD30 soft reset {error:?}"))?;
                log::info!("soft reset!");
            }
        }

        Ok(())
    }
}
