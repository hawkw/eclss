pub use tinymetrics::{Counter, Gauge};

use serde::{Serialize, Serializer};
use std::fmt;
use tinymetrics::{CounterFamily, FmtLabels, GaugeFamily, MetricBuilder, MetricFamily};

const MAX_METRICS: usize = 4;

#[derive(Debug, serde::Serialize)]
pub struct SensorMetrics {
    #[serde(serialize_with = "serialize_metric")]
    pub temp: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub co2: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub humidity: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub pressure: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub gas_resistance: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub sensor_errors: CounterFamily<'static, MAX_METRICS, SensorLabel>,
}

#[derive(Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct SensorLabel(pub &'static str);

impl SensorMetrics {
    pub const fn new() -> Self {
        Self {
            temp: MetricBuilder::new("temperature_degrees_celcius")
                .with_help("Temperature in degrees Celcius.")
                .with_unit("celcius")
                .build_labeled::<_, SensorLabel, 4>(),
            co2: MetricBuilder::new("co2_ppm")
                .with_help("CO2 in parts per million (ppm).")
                .with_unit("ppm")
                .build_labeled::<_, SensorLabel, 4>(),
            humidity: MetricBuilder::new("humidity_percent")
                .with_help("Relative humidity (RH) percentage.")
                .with_unit("percent")
                .build_labeled::<_, SensorLabel, 4>(),
            pressure: MetricBuilder::new("pressure_hpa")
                .with_help("Barometric pressure, in hectopascals (hPa).")
                .with_unit("hPa")
                .build_labeled::<_, SensorLabel, 4>(),
            gas_resistance: MetricBuilder::new("gas_resistance_ohms")
                .with_help("BME680 VOC sensor resistance, in Ohms.")
                .with_unit("Ohms")
                .build_labeled::<_, SensorLabel, 4>(),
            sensor_errors: MetricBuilder::new("sensor_error_count")
                .with_help("Count of I2C errors that occurred while talking to a sensor")
                .build_labeled::<_, SensorLabel, 4>(),
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

impl fmt::Display for SensorMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_metrics(f)
    }
}

// === impl Label ===

impl FmtLabels for SensorLabel {
    fn fmt_labels(&self, writer: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(writer, "sensor=\"{}\"", self.0)
    }
}

fn serialize_metric<S, M, L, const METRICS: usize>(
    metric: &MetricFamily<M, METRICS, L>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    M: Serialize,
    L: Serialize,
{
    metric.metrics().serialize(serializer)
}
