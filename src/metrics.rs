pub use tinymetrics::{Counter, Gauge};

use serde::{Serialize, Serializer};
use std::fmt;
use tinymetrics::{CounterFamily, FmtLabels, GaugeFamily, MetricBuilder, MetricFamily};

pub const MAX_METRICS: usize = 4;

#[derive(Debug, serde::Serialize)]
pub struct SensorMetrics {
    #[serde(serialize_with = "serialize_metric")]
    pub temp: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub co2: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub eco2: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub rel_humidity: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub abs_humidity: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub pressure: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub gas_resistance: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub tvoc: GaugeFamily<'static, MAX_METRICS, SensorLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub pm_conc: GaugeFamily<'static, 3, DiameterLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub pm_count: GaugeFamily<'static, 6, DiameterLabel>,
    #[serde(serialize_with = "serialize_metric")]
    pub sensor_errors: CounterFamily<'static, 16, SensorLabel>,
}

#[derive(Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct SensorLabel(pub &'static str);

#[derive(Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct DiameterLabel(pub &'static str);

impl SensorMetrics {
    pub const fn new() -> Self {
        Self {
            temp: MetricBuilder::new("temperature_degrees_celcius")
                .with_help("Temperature in degrees Celcius.")
                .with_unit("celcius")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            co2: MetricBuilder::new("co2_ppm")
                .with_help("CO2 in parts per million (ppm).")
                .with_unit("ppm")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            eco2: MetricBuilder::new("eco2_ppm")
                .with_help("VOC equivalent CO2 (eCO2) calculated by a tVOC sensor, in parts per million (ppm).")
                .with_unit("ppm")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            rel_humidity: MetricBuilder::new("humidity_percent")
                .with_help("Relative humidity (RH) percentage.")
                .with_unit("percent")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            abs_humidity: MetricBuilder::new("absolute_humidity_grams_m3")
                .with_help("Absolute humidity in grams per cubic meter.")
                .with_unit("g/m^3")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            pressure: MetricBuilder::new("pressure_hpa")
                .with_help("Barometric pressure, in hectopascals (hPa).")
                .with_unit("hPa")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            gas_resistance: MetricBuilder::new("gas_resistance_ohms")
                .with_help("BME680 VOC sensor resistance, in Ohms.")
                .with_unit("Ohms")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            tvoc: MetricBuilder::new("tvoc_ppb")
                .with_help("Total Volatile Organic Compounds (VOC) in parts per billion (ppb)")
                .with_unit("ppb")
                .build_labeled::<_, SensorLabel, MAX_METRICS>(),
            pm_conc: MetricBuilder::new("pm_concentration_ug_m3")
                .with_help("Particulate matter concentration in ug/m^3")
                .with_unit("ug/m^3")
                .build_labeled::<_, DiameterLabel, 3>(),
            pm_count: MetricBuilder::new("pm_count")
                .with_help("Particulate matter count per 0.1L of air.")
                .with_unit("particulates per 0.1L")
                .build_labeled::<_, DiameterLabel, 6>(),
            sensor_errors: MetricBuilder::new("sensor_error_count")
                .with_help("Count of I2C errors that occurred while talking to a sensor")
                .build_labeled::<_, SensorLabel, 16>(),
        }
    }

    pub fn fmt_metrics(&self, f: &mut impl fmt::Write) -> fmt::Result {
        self.temp.fmt_metric(f)?;
        self.co2.fmt_metric(f)?;
        self.rel_humidity.fmt_metric(f)?;
        self.abs_humidity.fmt_metric(f)?;
        self.pressure.fmt_metric(f)?;
        self.gas_resistance.fmt_metric(f)?;
        self.tvoc.fmt_metric(f)?;
        self.pm_conc.fmt_metric(f)?;
        self.pm_count.fmt_metric(f)?;
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

impl FmtLabels for DiameterLabel {
    fn fmt_labels(&self, writer: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(writer, "diameter=\"{}\",sensor=\"PMSA003I\"", self.0)
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
