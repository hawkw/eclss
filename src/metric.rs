use crate::atomic::{AtomicF32, AtomicU64, Ordering};
use embedded_svc::io;
use esp_idf_svc::systime::EspSystemTime;
#[derive(Debug)]
pub struct Gauge<'a, S> {
    pub name: &'a str,
    pub help: &'a str,
    pub sensors: S,
}

#[derive(Debug)]
pub struct SensorGauge<'a> {
    value: AtomicF32,
    timestamp: AtomicU64,
    name: &'a str,
}

impl<'a, S> Gauge<'a, S> {
    pub fn sensors(&self) -> &S {
        &self.sensors
    }

    pub fn render_prometheus<'metrics, R: io::Write>(
        &'metrics self,
        writer: &mut R,
    ) -> Result<(), io::WriteFmtError<R::Error>>
    where
        &'metrics S: IntoIterator<Item = &'metrics SensorGauge<'a>>,
    {
        let Self {
            sensors,
            name,
            help,
        } = self;

        writeln!(writer, "# HELP {name} {help}\n# TYPE {name} gauge")?;
        for sensor in sensors {
            let value = sensor.value();
            let time = sensor.timestamp.load(Ordering::Acquire);
            let sensor_name = sensor.name;

            writeln!(writer, "{name}{{sensor=\"{sensor_name}\"}} {value} {time}")?;
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
            name,
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

impl<'metric, 'a> IntoIterator for &'a SensorGauge<'metric> {
    type Item = &'a SensorGauge<'metric>;

    type IntoIter = std::iter::Once<&'a SensorGauge<'metric>>;
    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}
