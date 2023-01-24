use anyhow::Context;
use embedded_svc::wifi::{
    AccessPointConfiguration, AccessPointInfo, ClientConfiguration, Configuration, Wifi,
};
use esp_idf_hal::{modem::Modem, peripheral::Peripheral};
use esp_idf_svc::{
    eventloop::*,
    mdns::EspMdns,
    netif::{EspNetif, EspNetifWait},
    nvs::EspDefaultNvsPartition,
    wifi::{EspWifi, WifiWait},
};

use std::{
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
        // log::info!("bringing up WiFi...");
        let mut wifi = Box::new(EspWifi::new(modem, sysloop.clone(), Some(nvs))?);

        wifi.start()?;
        // log::info!("wifi started");

        log::info!("scanning for access points...");
        let access_points = Wifi::scan(&mut *wifi).context("failed to scan for access points")?;

        // restore a previous client configuration from NVS.
        let config = match wifi.get_configuration() {
            // if a previous client configuration was saved in NVS, map it to a
            // mixed config so we can continue running an AP as well as connecting.
            Ok(Configuration::Client(client_config)) => {
                Configuration::Mixed(client_config, Self::access_point_config())
            }
            // if no previous configuration was saved, start in AP mode.
            Ok(Configuration::None) => {
                // log::info!("no WiFi configuration saved; starting in access point mode");
                Configuration::AccessPoint(Self::access_point_config())
            }
            // restore the previous access point or mixed configuration.
            Ok(config) => config,
            Err(error) => {
                log::warn!("failed to load existing wifi configuration: {error}; starting in access point mode");
                Configuration::AccessPoint(Self::access_point_config())
            }
        };
        wifi.set_configuration(&config)
            .context("failed to set WiFi configuration")?;

        let mut this = Self {
            wifi,
            access_points: Arc::new(RwLock::new(access_points)),
            wait_timeout: Duration::from_secs(20),
            config,
        };

        match (this.start_connect(sysloop), &this.config) {
            // if we tried to start WiFi in a mixed client/AP configuration, and
            // failed to connect, it's possible that we were previously
            // connected to an AP that no longer exists. in that case, just go
            // to AP mode.
            (Err(error), Configuration::Mixed(_, ap)) => {
                log::warn!("no joy connecting to previous WiFi network: {error}");
                // log::info!("maybe it no longer exists? switching to AP mode");
                this.configure(sysloop, Configuration::AccessPoint(ap.clone()))
                    .context("failed to start WiFi in AP mode")?;
            }
            // otherwise, if we can't start the wifi, that's bad.
            (Err(error), _) => return Err(error).context("failed to start WiFi"),
            (Ok(_), _) => log::info!("WiFi started!"),
        }
        // TODO(eliza): if we can't connect to a previous wifi AP, just start in AP mode?

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
            .set_configuration(&config)
            .context("failed to set wifi config")?;
        self.config = config;
        self.start_connect(sysloop)
    }

    fn start_connect(&mut self, sysloop: &EspSystemEventLoop) -> anyhow::Result<()> {
        self.wifi.start().context("failed to start WiFi")?;

        log::debug!("Waiting for wifi to start ({:?})...", self.wait_timeout);
        let wait = WifiWait::new(sysloop)
            .context("failed to create wifi wait")?
            .wait_with_timeout(self.wait_timeout, || {
                self.wifi.is_started().unwrap_or_default()
            });
        anyhow::ensure!(wait, "WiFi did not start within {:?}", self.wait_timeout);

        // log::info!("WiFi started with configuration={:#?}", self.config);

        // nowhere to connect
        if let Configuration::AccessPoint(_) = self.config {
            return Ok(());
        }

        self.wifi
            .connect()
            .context("failed to connect to WiFi network")?;

        // log::debug!("Waiting for netif ({:?})...", self.wait_timeout);
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

pub fn init_mdns(mdns: &mut EspMdns) -> anyhow::Result<()> {
    let txt = &[("board", "esp32c3"), ("version", env!("CARGO_PKG_VERSION"))];
    mdns.set_hostname("eclss").context("set mDNS hostname")?;
    mdns.set_instance_name("Environmental Control and Life Support Systems")
        .context("set mDNS instance name")?;
    mdns.add_service(None, "_http", "_tcp", crate::http::HTTP_PORT, txt)
        .context("add HTTP mDNS service")?;
    mdns.add_service(None, "_https", "_tcp", crate::http::HTTPS_PORT, txt)
        .context("add HTTPS mDNS service")?;
    mdns.add_service(
        None,
        "_prometheus-http",
        "_tcp",
        crate::http::HTTP_PORT,
        txt,
    )
    .context("add Prometheus HTTP mDNS service")?;
    mdns.add_service(
        None,
        "_prometheus-https",
        "_tcp",
        crate::http::HTTPS_PORT,
        txt,
    )
    .context("add Prometheus HTTPS mDNS service")?;

    log::info!("advertising mDNS services");

    Ok(())
}
