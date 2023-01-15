use crate::atomic::{AtomicF32, Ordering};
use std::fmt;

#[derive(Debug)]
pub struct Gauge<'a> {
    name: &'a str,
    help: Option<&'a str>,
    value: AtomicF32,
    // labels: Option<&'a [(&'a str, &'a str)]>,
}

impl<'a> Gauge<'a> {
    pub const fn new(name: &'a str) -> Self {
        Self {
            name,
            help: None,
            value: AtomicF32::zero(),
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
    }

    pub fn value(&self) -> f32 {
        self.value.load(Ordering::Acquire)
    }

    pub fn render_prometheus(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        let name = self.name;
        if let Some(help) = self.help {
            writeln!(writer, "# HELP {name} {help}",)?;
        }

        writeln!(writer, "# TYPE {name} gauge")?;
        writeln!(writer, "{name} {}", self.value())?;
        Ok(())
    }
}
