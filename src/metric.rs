use crate::atomic::{AtomicF32, AtomicU64, Ordering};
use embedded_svc::io;
use esp_idf_svc::systime::EspSystemTime;

#[derive(Debug)]
pub struct MetricDef<'a> {
    name: &'a str,
    help: Option<&'a str>,
    unit: Option<&'a str>,
}

#[derive(Debug)]
pub struct Metric<'a, S> {
    metric: MetricDef<'a>,
    sensors: S,
}

pub trait SensorMetric<'a> {
    const TYPE: &'static str;

    fn label(&self) -> &'a str;

    fn render_value<R: io::Write>(&self, writer: &mut R)
        -> Result<(), io::WriteFmtError<R::Error>>;
}

#[derive(Debug, serde::Serialize)]
pub struct SensorGauge<'a> {
    value: AtomicF32,
    timestamp: AtomicU64,
    sensor: &'a str,
}

#[derive(Debug, serde::Serialize)]
pub struct SensorCounter<'a> {
    value: AtomicU64,
    timestamp: AtomicU64,
    sensor: &'a str,
}

// === impl Metric ===

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

    pub const fn with_sensors<S>(self, sensors: S) -> Metric<'a, S> {
        Metric {
            metric: self,
            sensors,
        }
    }
}

// === impl Gauge ===

impl<'a, S> Metric<'a, S> {
    pub fn sensors(&self) -> &S {
        &self.sensors
    }

    pub fn render_prometheus<'metrics, M, R>(
        &'metrics self,
        writer: &mut R,
    ) -> Result<(), io::WriteFmtError<R::Error>>
    where
        R: io::Write,
        M: SensorMetric<'a> + 'metrics,
        &'metrics S: IntoIterator<Item = &'metrics M>,
    {
        let Self {
            sensors,
            metric: MetricDef { name, help, unit },
        } = self;

        writeln!(writer, "# TYPE {name} {}", M::TYPE)?;

        if let Some(help) = help {
            writeln!(writer, "# HELP {name} {help}")?;
        }

        if let Some(unit) = unit {
            writeln!(writer, "# UNIT {name} {unit}")?;
        }

        for sensor in sensors {
            let sensor_name = sensor.label();

            write!(writer, "{name}{{sensor=\"{sensor_name}\"}} ")?;
            sensor.render_value(writer)?;
            writer.write(b"\n").map_err(io::WriteFmtError::Other)?;
        }
        writer.write(b"\n").map_err(io::WriteFmtError::Other)?;

        Ok(())
    }
}

// === impl SensorGauge ===

impl<'a> SensorGauge<'a> {
    pub const fn new(name: &'a str) -> Self {
        Self {
            value: AtomicF32::zero(),
            timestamp: AtomicU64::new(0),
            sensor: name,
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

impl<'a> SensorMetric<'a> for SensorGauge<'a> {
    const TYPE: &'static str = "gauge";

    fn label(&self) -> &'a str {
        self.sensor
    }

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

impl<'metric, 'a> IntoIterator for &'a SensorGauge<'metric> {
    type Item = &'a SensorGauge<'metric>;

    type IntoIter = std::iter::Once<&'a SensorGauge<'metric>>;
    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}

// === impl SensorCounter ===

impl<'a> SensorCounter<'a> {
    pub const fn new(name: &'a str) -> Self {
        Self {
            value: AtomicU64::new(0),
            timestamp: AtomicU64::new(0),
            sensor: name,
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

impl<'a> SensorMetric<'a> for SensorCounter<'a> {
    const TYPE: &'static str = "counter";

    fn label(&self) -> &'a str {
        self.sensor
    }

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

impl<'metric, 'a> IntoIterator for &'a SensorCounter<'metric> {
    type Item = &'a SensorCounter<'metric>;

    type IntoIter = std::iter::Once<&'a SensorCounter<'metric>>;
    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}
