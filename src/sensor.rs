use crate::{
    actor::Actor,
    metrics::{self, SensorMetrics},
    registry::RegistryMap,
    retry::ExpBackoff,
    I2cBus,
};
use embassy_time::{Duration, Timer};
use futures::{select, FutureExt};
use std::fmt;

mod status;
pub use self::status::{Status, StatusCell};
/// Represents a pollable I2C sensor.
pub trait Sensor: Sized {
    /// Messages sent to control the behavior of this sensor.
    ///
    /// These messages instruct the sensor to do something, such as calibrating
    /// itself or changing its operating parameters. The `ControlMessage` type
    /// is typically defined by each sensor implementation as an `enum` of
    /// messages specific to that sensor type.
    ///
    /// If a sensor does not respond to control messages, its `ControlMessage`
    /// type may be `()`.
    type ControlMessage: fmt::Debug;

    const NAME: &'static str;
    const LABEL: metrics::SensorLabel = metrics::SensorLabel(Self::NAME);

    fn bringup(i2c: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self>;

    fn poll(&mut self) -> anyhow::Result<()>;

    /// Returns the interval between calls to [`poll`].
    fn poll_interval(&self) -> Duration;

    /// Handle a [`ControlMessage`] sent to this sensor.
    ///
    /// This method's behavior will depend on the control messages defined by
    /// this sensor type.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the control message was handled successfully and the
    ///   sensor performed the requested behavior.
    /// - `Err(anyhow::Error)` if the sensor failed to perform the requested
    ///   behavior.
    fn handle_control_message(&mut self, msg: &Self::ControlMessage) -> anyhow::Result<()>;
}

/// A sensor mangler for pollable I2C [`Sensor`]s.
///
/// A sensor manager handles sensor bringup, polls the sensor at the provided
/// `poll_interval`, and backs off when the sensor is unavailable. This allows a
/// limited form of hot-plugability for I2C sensors: although the kinds of
/// sensors that may be on the bus must be known in advance, they can be
/// disconnected after the device starts without requiring a complete reset.
#[derive(Copy, Clone)]
pub struct Manager {
    pub metrics: &'static SensorMetrics,
    pub busman: &'static I2cBus,
    pub retry_backoff: Duration,
}

pub static STATUSES: RegistryMap<&'static str, StatusCell, 16> = RegistryMap::new();

impl Manager {
    pub async fn run<S: Sensor>(
        self,
        ctrl_rx: Actor<S::ControlMessage, anyhow::Result<()>>,
    ) -> anyhow::Result<()> {
        let status = STATUSES
            .register_default(S::NAME)
            .ok_or_else(|| anyhow::anyhow!("insufficient space in status map for {}", S::NAME))?;
        let errors = self
            .metrics
            .sensor_errors
            .register(S::LABEL)
            .ok_or_else(|| {
                anyhow::anyhow!("insufficient space in error metrics map for {}", S::NAME)
            })?;

        let mut sensor = {
            loop {
                let mut backoff = ExpBackoff::new(self.retry_backoff).with_target(S::NAME);
                match S::bringup(self.busman, self.metrics) {
                    Ok(sensor) => {
                        log::info!(target: S::NAME, "successfully brought up {}!", S::NAME);
                        status.set_status(Status::Up);
                        break sensor;
                    }
                    Err(error) => {
                        log::warn!(
                            target: S::NAME,
                            "failed to bring up {}: {error:?}; retrying in {backoff:?}...",
                            S::NAME
                        );
                        errors.fetch_add(1);
                    }
                }

                backoff.wait().await;
            }
        };

        let mut backoff = ExpBackoff::new(sensor.poll_interval()).with_target(S::NAME);

        let mut poll_wait = Timer::after(Duration::from_secs(0));
        futures::pin_mut!(ctrl_rx);

        loop {
            // wait to be notified either by a control message coming in or the
            // poll timer...
            select! {
                msg = ctrl_rx.next_request().fuse() => {
                    match msg {
                        Some(msg) => {
                            let req = msg.request();
                            log::debug!(target: S::NAME, "received control message: {req:?}");

                            let res = sensor.handle_control_message(req);
                            if let Err(ref error) = res {
                                log::warn!(target: S::NAME, "failed to respond to control message {req:?}: {error}");
                                errors.fetch_add(1);
                            }

                            if let Err(_) = msg.respond(res) {
                                log::debug!(target: S::NAME, "control message canceled");
                            }
                        },
                        None => log::warn!(target: S::NAME, "control message stream has ended, that's weird..."),
                    };
                    continue;
                },

                _ = (&mut poll_wait).fuse() => match sensor.poll() {
                    Err(error) => {
                        log::warn!(target: S::NAME, "error polling {}: {error:?}", S::NAME);
                        status.set_status(Status::Down);
                        errors.fetch_add(1);
                        poll_wait = backoff.wait();
                    }
                    Ok(()) => {
                        // if we have previously backed off due to repeated errors,
                        // reset the backoff now that the sensor is alive again.
                        backoff.reset();
                        poll_wait = Timer::after(sensor.poll_interval());
                        status.set_status(Status::Up);
                    }
                }
            }
        }
    }
}
