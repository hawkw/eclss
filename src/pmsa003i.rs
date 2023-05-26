use crate::{
    metrics::{DiameterLabel, Gauge, SensorMetrics},
    sensor::Sensor,
    I2cBus, I2cRef,
};

pub struct Pmsa003i {
    sensor: pmsa003i::Pmsa003i<I2cRef<'static>>,
    pm2_5: &'static Gauge,
    pm1_0: &'static Gauge,
    pm10_0: &'static Gauge,
    particles_0_3um: &'static Gauge,
    particles_0_5um: &'static Gauge,
    particles_1_0um: &'static Gauge,
    particles_2_5um: &'static Gauge,
    particles_5_0um: &'static Gauge,
    particles_10_0um: &'static Gauge,
}

const NAME: &'static str = "PMSA003I";

impl Sensor for Pmsa003i {
    type ControlMessage = ();

    const NAME: &'static str = NAME;

    fn bringup(busman: &'static I2cBus, metrics: &'static SensorMetrics) -> anyhow::Result<Self> {
        log::info!("connecting to {}", Self::NAME);
        let i2c = busman.acquire_i2c();
        Ok(Self {
            sensor: pmsa003i::Pmsa003i::new(i2c),
            pm2_5: metrics.pm_conc.register(DiameterLabel("2.5")).unwrap(),
            pm1_0: metrics.pm_conc.register(DiameterLabel("1.0")).unwrap(),
            pm10_0: metrics.pm_conc.register(DiameterLabel("10.0")).unwrap(),
            particles_0_3um: metrics.pm_count.register(DiameterLabel("0.3")).unwrap(),
            particles_0_5um: metrics.pm_count.register(DiameterLabel("0.5")).unwrap(),
            particles_1_0um: metrics.pm_count.register(DiameterLabel("1.0")).unwrap(),
            particles_2_5um: metrics.pm_count.register(DiameterLabel("2.5")).unwrap(),
            particles_5_0um: metrics.pm_count.register(DiameterLabel("5.0")).unwrap(),
            particles_10_0um: metrics.pm_count.register(DiameterLabel("10.0")).unwrap(),
        })
    }

    fn poll(&mut self) -> anyhow::Result<()> {
        let pmsa003i::Reading {
            concentrations,
            counts,
            sensor_version: _,
        } = self
            .sensor
            .read()
            .map_err(|error| anyhow::anyhow!("error reading from {NAME}: {error:?}"))?;

        log::info!("[{NAME}]: particulate concentrations:\n{concentrations:>#3}");
        log::info!("[{NAME}]: particulates {counts:>#3}");

        macro_rules! set_metrics {
            ($src:ident => $($name:ident),+) => {
                $(
                    self.$name.set_value($src.$name.into());
                )+
            }
        }
        set_metrics!(concentrations => pm1_0, pm2_5, pm10_0);
        set_metrics!(counts =>
            particles_0_3um,
            particles_0_5um,
            particles_1_0um,
            particles_2_5um,
            particles_5_0um,
            particles_10_0um
        );
        Ok(())
    }

    fn poll_interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(2)
    }

    fn handle_control_message(&mut self, _: &Self::ControlMessage) -> anyhow::Result<()> {
        Ok(())
    }
}
