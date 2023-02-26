use crate::{
    atomic::{AtomicF32, AtomicU64, Ordering},
    registry::RegistryMap,
};
use core::fmt;
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

pub trait Metric: Default {
    const TYPE: &'static str;

    fn fmt_metric<F: fmt::Write>(&self, writer: &mut F) -> fmt::Result;
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

    pub const fn with_metrics<M, const METRICS: usize>(self) -> MetricFamily<'a, M, METRICS> {
        MetricFamily {
            def: self,
            metrics: RegistryMap::new(),
        }
    }
}

// === impl MetricFamily ===

impl<'a, M, const METRICS: usize> MetricFamily<'a, M, METRICS>
where
    M: Metric,
{
    pub fn register<'fam>(&'fam self, labels: Labels<'a>) -> Option<&'fam M> {
        self.metrics.register_default(labels)
    }

    pub fn metrics(&self) -> &RegistryMap<Labels<'a>, M, METRICS> {
        &self.metrics
    }

    pub fn fmt_metric(&self, writer: &mut impl fmt::Write) -> fmt::Result {
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
            writer.write_str(name)?;

            let mut labels = labels.iter();
            if let Some(&(k, v)) = labels.next() {
                write!(writer, "{{{k}=\"{v}\"")?;

                for &(k, v) in labels {
                    write!(writer, ",{k}=\"{v}\"")?;
                }

                writer.write_char('}')?;
            }

            metric.fmt_metric(writer)?;
            writer.write_char('\n')?;
        }
        writer.write_char('\n')?;

        Ok(())
    }
}

impl<'a, M, const METRICS: usize> fmt::Display for MetricFamily<'a, M, METRICS>
where
    M: Metric,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_metric(f)
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

    fn fmt_metric<F: fmt::Write>(&self, writer: &mut F) -> fmt::Result {
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

    fn fmt_metric<F: fmt::Write>(&self, writer: &mut F) -> fmt::Result {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge() {
        let family = MetricDef::new("test_gauge")
            .with_help("a test gauge")
            .with_unit("tests")
            .with_metrics::<2>();
        let metric1 = family
            .register(&[("metric", "1"), ("label2", "foo")])
            .expect("metric 1 must register");
        metric1.set_value(10.0);

        let metric2 = family
            .register(&[("metric", "2"), ("label2", "bar")])
            .expect("metric 2 must register");
        metric2.set_value(22.2);

        let expected = "\
        # TYPE test_gauge gauge\n\
        # HELP test_gauge a test gauge\n\
        # UNIT test_gauge tests\n\
        test_gauge{metric=\"1\",label2=\"foo\"} 10.0\n\
        test_gauge{metric=\"2\",label2=\"bar\"} 22.2\n\
        ";
        assert_eq!(family.to_string(), expected);
    }

    #[test]
    fn counter() {
        let family = MetricDef::new("test_counter")
            .with_help("a test counter")
            .with_unit("tests")
            .with_metrics::<2>();
        let metric1 = family
            .register(&[("metric", "1"), ("label2", "foo")])
            .expect("metric 1 must register");
        metric1.incr();

        let metric2 = family
            .register(&[("metric", "2"), ("label2", "bar")])
            .expect("metric 2 must register");
        metric2.incr();
        metric2.incr();

        let expected = "\
        # TYPE test_counter counter\n\
        # HELP test_counter a test counter\n\
        # UNIT test_counter tests\n\
        test_counter{metric=\"1\",label2=\"foo\"} 1\n\
        test_counter{metric=\"2\",label2=\"bar\"} 2\n\
        ";
        assert_eq!(family.to_string(), expected);
    }
}
