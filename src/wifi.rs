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

use std::{
    fmt,
    sync::{Arc, RwLock},
    time::Duration,
};

pub struct EclssWifi {
    wifi: Box<EspWifi<'static>>,
    pub access_points: AccessPoints,
    wait_timeout: Duration,
    config: Configuration,
}

#[derive(Debug, serde::Deserialize)]
pub struct Credentials {
    pub ssid: String,
    pub password: String,
}

pub type AccessPoints = Arc<RwLock<Vec<AccessPointInfo>>>;

impl EclssWifi {
    pub fn new(
        modem: impl Peripheral<P = Modem> + 'static,
        sysloop: &EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
    ) -> anyhow::Result<Self> {
        log::info!("bringing up WiFi...");
        let mut wifi = Box::new(EspWifi::new(modem, sysloop.clone(), Some(nvs))?);

        wifi.start()?;
        log::info!("wifi started");

        log::info!("scanning for access points...");
        let access_points = Wifi::scan(&mut *wifi).context("failed to scan for access points")?;

        let mut this = Self {
            wifi,
            access_points: Arc::new(RwLock::new(access_points)),
            wait_timeout: Duration::from_secs(20),
            config: Default::default(),
        };

        this.configure(
            &sysloop,
            Configuration::AccessPoint(Self::access_point_config()),
        )
        .context("configure in access point only mode")?;
        Ok(this)
    }

    pub fn connect_to(
        &mut self,
        sysloop: &EspSystemEventLoop,
        credentials: Credentials,
    ) -> anyhow::Result<()> {
        let channel = self.access_points.read().unwrap().iter().find_map(|ap| {
            if ap.ssid.as_str() == credentials.ssid {
                Some(ap.channel)
            } else {
                None
            }
        });

        let config = Configuration::Mixed(
            ClientConfiguration {
                ssid: credentials
                    .ssid
                    .parse()
                    .map_err(|_| anyhow::anyhow!("ssid too long"))?,
                password: credentials
                    .password
                    .parse()
                    .map_err(|_| anyhow::anyhow!("password too long"))?,
                channel,
                ..Default::default()
            },
            Self::access_point_config(),
        );

        self.configure(sysloop, config)
    }

    fn configure(
        &mut self,
        sysloop: &EspSystemEventLoop,
        config: Configuration,
    ) -> anyhow::Result<()> {
        self.wifi
            .set_configuration(&self.config)
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
        self.config = config;

        // nowhere to connect
        if let Configuration::AccessPoint(_) = self.config {
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
