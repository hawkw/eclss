use crate::{retry::ExpBackoff, I2cBus, SensorMetrics};
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
    pub async fn run<S: Sensor>(self) -> anyhow::Result<()> {
        let mut sensor = {
            loop {
                let mut backoff = ExpBackoff::new(self.retry_backoff).with_target(S::NAME);
                match S::bringup(self.busman) {
                    Ok(sensor) => {
                        log::info!(target: S::NAME, "successfully brought up {}!", S::NAME);
                        break sensor;
                    }
                    Err(error) => {
                        log::warn!(
                            target: S::NAME,
                            "failed to bring up {}: {error:?}; retrying in {backoff:?}...",
                            S::NAME
                        );
                        S::incr_error(self.metrics);
                    }
                }

                backoff.wait().await;
            }
        };

        let mut backoff = ExpBackoff::new(self.poll_interval).with_target(S::NAME);
        loop {
            match sensor.poll(self.metrics) {
                Err(error) => {
                    log::warn!(target: S::NAME, "error polling {}: {error:?}", S::NAME);
                    S::incr_error(self.metrics);
                    backoff.wait().await;
                }
                Ok(()) => {
                    // if we have previously backed off due to repeated errors,
                    // reset the backoff now that the sensor is alive again.
                    backoff.reset();
                    Timer::after(self.poll_interval).await;
                }
            }
        }
    }
}
