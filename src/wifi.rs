use anyhow::Context;
use embedded_svc::wifi::{
    AccessPointConfiguration, AccessPointInfo, ClientConfiguration, Configuration, Wifi,
};
use esp_idf_hal::{modem::Modem, peripheral::Peripheral};
use esp_idf_svc::{
    eventloop::*,
    netif::{EspNetif, EspNetifWait},
    nvs::EspDefaultNvsPartition,
    // ping,
    wifi::{
        // config::{self, ScanConfig},
        EspWifi,
        WifiWait,
    },
};

use std::{fmt, time::Duration};

pub struct EclssWifi {
    wifi: Box<EspWifi<'static>>,
    aps: Vec<AccessPointInfo>,
    wait_timeout: Duration,
}

impl EclssWifi {
    pub fn new(
        modem: impl Peripheral<P = Modem> + 'static,
        sysloop: &EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
    ) -> anyhow::Result<Self> {
        log::info!("bringing up WiFi...");
        let mut wifi = Box::new(EspWifi::new(modem, sysloop.clone(), None)?);

        wifi.start()?;
        log::info!("wifi started");

        log::info!("scanning for access points...");
        // let start_scan = wifi.start_scan(
        //     &ScanConfig {
        //         scan_type: config::ScanType::Active {
        //             min: Duration::from_secs(1),
        //             max: Duration::from_secs(20),
        //         },
        //         ..Default::default()
        //     },
        //     true,
        // );
        // log::info!("scan started: {:?}", start_scan);
        // let aps = wifi
        //     .get_scan_result()
        //     .context("failed to get scan result")?;
        let aps = Wifi::scan(&mut *wifi).context("failed to scan for access points")?;

        let mut this = Self {
            wifi,
            aps,
            wait_timeout: Duration::from_secs(20),
        };

        this.configure(
            &sysloop,
            Configuration::AccessPoint(Self::access_point_config()),
        )
        .context("configure in access point only mode")?;
        Ok(this)
    }

    fn configure(
        &mut self,
        sysloop: &EspSystemEventLoop,
        config: Configuration,
    ) -> anyhow::Result<()> {
        self.wifi
            .set_configuration(&config)
            .context("failed to set wifi config")?;
        self.wifi.start().context("failed to start WiFi")?;

        log::debug!("Waiting for wifi to start ({:?})...", self.wait_timeout);
        let wait = WifiWait::new(sysloop)
            .context("failed to create wifi wait")?
            .wait_with_timeout(self.wait_timeout, || {
                self.wifi.is_started().unwrap_or_default()
            });
        anyhow::ensure!(wait, "WiFi did not start within {:?}", self.wait_timeout);

        log::info!("WiFi started with configuration={config:#?}");

        // nowhere to connect
        if let Configuration::AccessPoint(_) = config {
            return Ok(());
        }

        self.wifi
            .connect()
            .context("failed to connect to WiFi network")?;

        log::debug!("Waiting for netif ({:?})...", self.wait_timeout);
        let netif_wait = EspNetifWait::new::<EspNetif>(self.wifi.sta_netif(), sysloop)
            .context("failed to create wait for STA netif")?
            .wait_with_timeout(self.wait_timeout, || {
                self.wifi.is_connected().unwrap_or_default()
                    && self
                        .wifi
                        .sta_netif()
                        .get_ip_info()
                        .map(|info| !info.ip.is_unspecified())
                        .unwrap_or_default()
            });
        anyhow::ensure!(
            netif_wait,
            "WiFi did not recieve a DHCP lease within {:?}",
            self.wait_timeout
        );

        log::info!("Wifi connected");
        Ok(())
    }

    fn access_point_config() -> AccessPointConfiguration {
        AccessPointConfiguration {
            ssid: "eclss".into(),
            channel: 1,
            ..Default::default()
        }
    }
}

fn retry<T, E: fmt::Debug>(
    mut retries: usize,
    mut f: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    loop {
        match f() {
            Ok(val) => return Ok(val),
            Err(error) if retries == 0 => return Err(error),
            Err(error) => {
                retries -= 1;
                log::warn!("wifi retrying: {error:?} ({retries} retries remaining)");
            }
        }
    }
}
