use crate::SensorMetrics;
use std::time::Duration;

pub trait Sensor {
    fn name(&self) -> &str;
    fn poll_interval(&self) -> Duration;
    fn update_metrics(&self, metrics: &SensorMetrics) -> anyhow::Result<()>;
}

impl 