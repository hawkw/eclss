use crate::atomic::{AtomicF32, AtomicU64, Ordering};
use embedded_svc::io;
use esp_idf_svc::systime::EspSystemTime;

#[derive(Debug)]
pub struct Gauge<'a> {
    name: &'a str,
    help: Option<&'a str>,
    value: AtomicF32,
    timestamp: AtomicU64,
    // labels: Option<&'a [(&'a str, &'a str)]>,
}

impl<'a> Gauge<'a> {
    pub const fn new(name: &'a str) -> Self {
        Self {
            name,
            help: None,
            value: AtomicF32::zero(),
            timestamp: AtomicU64::new(0),
            // labels: None,
        }
    }

    pub const fn with_help(self, help: &'a str) -> Self {
        Self {
            help: Some(help),
            ..self
        }
    }

    // pub const fn with_labels(self, labels: &'a [(&'a str, &'a str)]) -> Self {
    //     Self {
    //         labels: Some(labels),
    //         ..self
    //     }
    // }

    pub fn set_value(&self, value: f32) {
        self.value.store(value, Ordering::Release);
        let timestamp = EspSystemTime.now().as_secs();
        self.timestamp.store(timestamp, Ordering::Release)
    }

    pub fn value(&self) -> f32 {
        self.value.load(Ordering::Acquire)
    }

    pub fn render_prometheus<R: io::Write>(
        &self,
        writer: &mut R,
    ) -> Result<(), io::WriteFmtError<R::Error>> {
        let name = self.name;
        if let Some(help) = self.help {
            writeln!(writer, "# HELP {name} {help}",)?;
        }

        writeln!(writer, "# TYPE {name} gauge")?;
        let value = self.value();
        let time = self.timestamp.load(Ordering::Acquire);
        if time > 0 {
            writeln!(writer, "{name} {value} {time}")?;
        } else {
            writeln!(writer, "{name} {value}")?;
        }
        Ok(())
    }
}
