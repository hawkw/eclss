use crate::{
    atomic::{AtomicF32, AtomicU64, Ordering},
    registry::RegistryMap,
};
use embedded_svc::io;
use esp_idf_svc::systime::EspSystemTime;

#[derive(Debug)]
pub struct MetricDef<'a> {
    name: &'a str,
    help: Option<&'a str>,
    unit: Option<&'a str>,
}

#[derive(Debug)]
pub struct MetricFamily<'a, M, const METRICS: usize> {
    def: MetricDef<'a>,
    metrics: RegistryMap<Labels<'a>, M, METRICS>,
}

pub type GaugeFamily<'a, const METRICS: usize> = MetricFamily<'a, Gauge, METRICS>;
pub type CounterFamily<'a, const METRICS: usize> = MetricFamily<'a, Counter, METRICS>;
pub type Labels<'a> = &'a [(&'a str, &'a str)];

pub trait Metric {
    const TYPE: &'static str;

    fn render_value<R: io::Write>(&self, writer: &mut R)
        -> Result<(), io::WriteFmtError<R::Error>>;
}

#[derive(Debug, serde::Serialize)]
pub struct Gauge {
    value: AtomicF32,
    timestamp: AtomicU64,
}

#[derive(Debug, serde::Serialize)]
pub struct Counter {
    value: AtomicU64,
    timestamp: AtomicU64,
}

// === impl MetricDef ===

impl<'a> MetricDef<'a> {
    pub const fn new(name: &'a str) -> Self {
        Self {
            name,
            help: None,
            unit: None,
        }
    }

    pub const fn with_help(self, help: &'a str) -> Self {
        Self {
            help: Some(help),
            ..self
        }
    }

    pub const fn with_unit(self, help: &'a str) -> Self {
        Self {
            help: Some(help),
            ..self
        }
    }

    pub const fn with_sensors<M, const SENSORS: usize>(self) -> MetricFamily<'a, M, SENSORS> {
        MetricFamily {
            def: self,
            metrics: RegistryMap::new(),
        }
    }
}

// === impl MetricFamily ===

impl<'a, M, const METRICS: usize> MetricFamily<'a, M, METRICS>
where
    M: Metric + Default,
{
    pub fn register<'fam>(&'fam self, labels: Labels<'a>) -> Option<&'fam M> {
        self.metrics.register_default(labels)
    }

    pub fn metrics(&self) -> &RegistryMap<Labels<'a>, M, METRICS> {
        &self.metrics
    }

    pub fn render_prometheus<'metrics, R>(
        &'metrics self,
        writer: &mut R,
    ) -> Result<(), io::WriteFmtError<R::Error>>
    where
        R: io::Write,
    {
        let Self {
            metrics: sensors,
            def: MetricDef { name, help, unit },
        } = self;

        writeln!(writer, "# TYPE {name} {}", M::TYPE)?;

        if let Some(help) = help {
            writeln!(writer, "# HELP {name} {help}")?;
        }

        if let Some(unit) = unit {
            writeln!(writer, "# UNIT {name} {unit}")?;
        }

        for (labels, metric) in sensors.iter() {
            writer
                .write(name.as_bytes())
                .map_err(io::WriteFmtError::Other)?;

            let mut labels = labels.iter();
            if let Some(&(k, v)) = labels.next() {
                write!(writer, "{{{k}=\"{v}\"")?;

                for &(k, v) in labels {
                    write!(writer, ",{k}=\"{v}\"")?;
                }

                writer.write(b"}").map_err(io::WriteFmtError::Other)?;
            }

            metric.render_value(writer)?;
            writer.write(b"\n").map_err(io::WriteFmtError::Other)?;
        }
        writer.write(b"\n").map_err(io::WriteFmtError::Other)?;

        Ok(())
    }
}

// === impl Gauge ===

impl Gauge {
    pub const fn new() -> Self {
        Self {
            value: AtomicF32::zero(),
            timestamp: AtomicU64::new(0),
        }
    }

    pub fn set_value(&self, value: f32) {
        self.value.store(value, Ordering::Release);
        let timestamp = EspSystemTime.now().as_secs();
        self.timestamp.store(timestamp, Ordering::Release)
    }

    pub fn value(&self) -> f32 {
        self.value.load(Ordering::Acquire)
    }
}

impl Metric for Gauge {
    const TYPE: &'static str = "gauge";

    fn render_value<R: io::Write>(
        &self,
        writer: &mut R,
    ) -> Result<(), io::WriteFmtError<R::Error>> {
        write!(
            writer,
            "{}",
            self.value(),
            // self.timestamp.load(Ordering::Acquire)
        )
    }
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

// === impl Counter ===

impl Counter {
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            timestamp: AtomicU64::new(0),
        }
    }

    pub fn incr(&self) {
        self.value.fetch_add(1, Ordering::Release);
        let timestamp = EspSystemTime.now().as_secs();
        self.timestamp.store(timestamp, Ordering::Release)
    }

    pub fn value(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }
}

impl Metric for Counter {
    const TYPE: &'static str = "counter";

    fn render_value<R: io::Write>(
        &self,
        writer: &mut R,
    ) -> Result<(), io::WriteFmtError<R::Error>> {
        write!(
            writer,
            "{}",
            self.value(),
            // self.timestamp.load(Ordering::Acquire)
        )
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}
