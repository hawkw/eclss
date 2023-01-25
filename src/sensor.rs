use crate::{I2cBus, SensorMetrics};
use embassy_time::{Duration, Timer};

pub trait Sensor: Sized {
    const NAME: &'static str;
    fn bringup(i2c: &'static I2cBus) -> anyhow::Result<Self>;
    fn poll(&mut self, metrics: &SensorMetrics) -> anyhow::Result<()>;
    fn incr_error(metrics: &SensorMetrics);
}

#[derive(Copy, Clone)]
pub struct Manager {
    pub metrics: &'static SensorMetrics,
    pub busman: &'static I2cBus,
    pub poll_interval: Duration,
    pub retry_backoff: Duration,
}

impl Manager {
    const MAX_BACKOFF: Duration = Duration::from_secs(60);

    pub async fn run<S: Sensor>(self) -> anyhow::Result<()> {
        let mut sensor = {
            let mut backoff = self.retry_backoff;
            loop {
                match S::bringup(self.busman) {
                    Ok(sensor) => {
                        log::info!("successfully brought up {}!", S::NAME);
                        break sensor;
                    }
                    Err(error) => {
                        log::warn!(
                            "failed to bring up {}: {error:?}; retrying in {backoff:?}...",
                            S::NAME
                        );
                        S::incr_error(self.metrics);
                    }
                }

                Timer::after(backoff).await;
                if backoff < Self::MAX_BACKOFF {
                    backoff *= 2;
                }
            }
        };

        loop {
            if let Err(error) = sensor.poll(self.metrics) {
                log::warn!("error polling {}: {error:?}", S::NAME);
                S::incr_error(self.metrics);
            }
            Timer::after(self.poll_interval).await;
        }
    }
}
