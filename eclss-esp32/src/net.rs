use anyhow::Context;
use channel_bridge::asynch::pubsub;
use embassy_time::Duration;
use embedded_svc::{
    utils::asyncify::Asyncify,
    wifi::{AccessPointConfiguration, AccessPointInfo, ClientConfiguration, Configuration, Wifi},
};
use esp_idf_hal::{modem::Modem, peripheral::Peripheral};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    mdns::EspMdns,
    netif::IpEvent,
    nvs::EspDefaultNvsPartition,
    wifi::{EspWifi, WifiEvent},
};
use futures::{future, FutureExt};
use thingbuf::mpsc;

use std::sync::{Arc, RwLock};

use crate::{retry, ws2812};

pub struct EclssWifi {
    wifi: Box<EspWifi<'static>>,
    pub access_points: AccessPoints,
    config: Configuration,
    creds_rx: mpsc::Receiver<Credentials>,
    creds_tx: mpsc::Sender<Credentials>,
    state: WifiState,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Credentials {
    pub ssid: String,
    pub password: String,
}

pub type AccessPoints = Arc<RwLock<Vec<AccessPointInfo>>>;

#[derive(Debug, Eq, PartialEq)]
enum WifiState {
    /// Waiting for an access point to be selected.
    Unconfigured,
    /// Waiting to successfully connect to an access point.
    Connecting,
    /// Connected to an access point; IP assigned.
    Connected,
    /// Disconnected from an access point; trying to reconnect.
    Disconnected,
    /// Some kind of error.
    Error,
}

impl EclssWifi {
    pub fn new(
        modem: impl Peripheral<P = Modem> + 'static,
        sysloop: &mut EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
    ) -> anyhow::Result<Self> {
        log::info!("bringing up WiFi...");
        let mut wifi = Box::new(EspWifi::new(modem, sysloop.clone(), Some(nvs))?);

        wifi.start()?;

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
                log::info!("no WiFi configuration saved; starting in access point mode");
                Configuration::AccessPoint(Self::access_point_config())
            }
            // restore the previous access point or mixed configuration.
            Ok(config) => config,
            Err(error) => {
                log::warn!("failed to load existing wifi configuration: {error}; starting in access point mode");
                Configuration::AccessPoint(Self::access_point_config())
            }
        };

        let state = match config {
            Configuration::AccessPoint(_) => WifiState::Unconfigured,
            _ => WifiState::Connecting,
        };
        wifi.set_configuration(&config)
            .context("failed to set WiFi configuration")?;
        let (creds_tx, creds_rx) = mpsc::channel(1);
        let mut this = Self {
            wifi,
            access_points: Arc::new(RwLock::new(access_points)),
            config,
            creds_rx,
            creds_tx,
            state,
        };

        this.wifi.start().context("failed to start WiFi")?;

        if this.state == WifiState::Connecting {
            log::info!("connecting to WiFi");
            this.wifi
                .connect()
                .context("failed to start WiFi connection")?;
        }

        Ok(this)
    }

    pub fn credentials_tx(&self) -> mpsc::Sender<Credentials> {
        self.creds_tx.clone()
    }

    pub async fn run(
        mut self,
        mut sysloop: EspSystemEventLoop,
        mut npx: crate::ws2812::NeoPixel<'static>,
    ) -> anyhow::Result<()> {
        let mut wifi_events = pubsub::SvcReceiver::new(
            sysloop
                .as_async()
                .subscribe::<WifiEvent>()
                .context("failed to subscribe to wifi events")?,
        );
        let mut ip_events = pubsub::SvcReceiver::new(
            sysloop
                .as_async()
                .subscribe::<IpEvent>()
                .context("failed to subscribe to IP events")?,
        );

        // exponential backoff for reconnecting, starting at 500 milliseconds.
        let mut backoff =
            retry::ExpBackoff::new(Duration::from_millis(500)).with_target("eclss::net");
        let mut current_backoff = future::Either::Left(future::pending());
        let mut has_ap_client = false;

        loop {
            // set the board's neopixel to indicate the current wifi state.
            if let Err(error) = self.state.set_neopixel_status(&mut npx) {
                log::warn!("failed to set neopixel wifi status: {error}");
            }

            log::info!("WiFi: {:?}; polling for events...", self.state);

            match self.state {
                WifiState::Error => {
                    log::info!("WiFi in error state; setting AP mode");
                    self.configure(Configuration::AccessPoint(Self::access_point_config()));
                    // clear reconnect backoff
                    current_backoff = future::Either::Left(future::pending());
                }
                // We have been disconnected from an access point. Try to reconnect...
                WifiState::Disconnected => {
                    log::info!("Wifi reconnecting in {}...", backoff.current());
                    current_backoff = future::Either::Right(backoff.wait());
                    // TODO(eliza): try scan here...
                }
                _ => {
                    current_backoff = future::Either::Left(future::pending());
                }
            }

            futures::select! {
                event = wifi_events.recv().fuse() => {
                    log::debug!("wifi event: {event:?}; state: {:?}", self.state);
                    match event {
                        WifiEvent::StaConnected => {
                            log::info!("connected to access point, waiting for IP assignment...");
                            self.state = WifiState::Connecting;
                            backoff.reset();
                        }
                        WifiEvent::StaDisconnected => {
                            log::info!("WiFi disconnected; state: {:?}", self.state);
                            self.state = WifiState::Disconnected;
                        }
                        WifiEvent::ApStaConnected => {
                            log::info!("WiFi client connected to softAP");
                            has_ap_client = true;
                        }
                        WifiEvent::ApStaDisconnected => {
                            log::info!("WiFi client disconnected from softAP");
                            has_ap_client = false;
                        }
                        other => {
                            log::info!("other WiFI event: {other:?}");
                            // TODO(eliza): handle scans here?
                        }
                    }
                },
                event = ip_events.recv().fuse() => {
                    log::debug!("network interface event: {event:?}; state: {:?}", self.state);
                    match event {
                        IpEvent::DhcpIpDeassigned(_) => {
                            log::info!("DHCP IP address deassigned by access point!");
                            self.state = WifiState::Error;
                        },
                        // any other event indicates that an IP was assigned
                        // (the variants are static IP assigned, DHCP IPv6
                        // assigned, and DHCP IPv4 assigned)
                        assigned => {
                            log::info!("IP assigned: {assigned:?}");
                            self.state = WifiState::Connected;

                        }
                    }
                },
                creds = self.creds_rx.recv().fuse() => {
                    if let Some(creds) = creds {
                        log::info!("received WiFi credentials: {creds:?}");
                        match self.connect_to(creds) {
                            Ok(_) => {
                                log::info!("connecting to WiFi access point");
                                self.state = WifiState::Connecting;
                            }
                            Err(error) => {
                                log::error!("failed to connect to WiFi access point: {error}");
                                self.state = WifiState::Error;
                            }
                        }
                    }
                }
                // time to start a reconnect?
                _ = (&mut current_backoff).fuse() => {
                    current_backoff = future::Either::Left(futures::future::pending());

                    // if a client is connected to the softAP, don't attempt to
                    // change the WiFi configuration until it's done, so that we
                    // don't break its connection.
                    if has_ap_client {
                        log::info!("cancelling reconnect attempt; a softAP client is connected");
                        continue;
                    }

                    if let Err(error) = self.wifi.connect() {
                        log::error!("WiFi failed to start reconnecting: {error}");
                        self.state = WifiState::Error;
                        continue;
                    } else {
                        log::info!("WiFi started reconnecting...");
                        self.state = WifiState::Connecting;
                    }
                }
            }
        }
    }

    fn configure(&mut self, config: Configuration) -> anyhow::Result<()> {
        (|| -> anyhow::Result<()> {
            self.wifi.set_configuration(&config)?;
            let ap_only = matches!(config, Configuration::AccessPoint(_));
            self.config = config;
            if !self
                .wifi
                .is_started()
                .context("failed to check if wifi is started")?
            {
                self.wifi.start().context("failed to start wifi")?
            }

            if ap_only {
                return Ok(());
            }

            self.wifi
                .connect()
                .context("failed to start connecting to access point")
        })()
        .context("failed to set WiFi configuration")
    }

    pub fn connect_to(&mut self, credentials: Credentials) -> anyhow::Result<()> {
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

        self.configure(config).with_context(|| {
            format!("failed to start connecting to {credentials:?}, channel: {channel:?}")
        })
    }

    fn access_point_config() -> AccessPointConfiguration {
        AccessPointConfiguration {
            ssid: "eclss".into(),
            channel: 1,
            ..Default::default()
        }
    }
}

// === impl WifiState ===

impl WifiState {
    fn set_neopixel_status(&self, npx: &mut ws2812::NeoPixel) -> anyhow::Result<()> {
        match self {
            // no wifi configured --- orange
            WifiState::Unconfigured => npx.set_color(255, 165, 0)?,
            // failed to connect --- red
            WifiState::Error => npx.set_color(255, 0, 0)?,
            // disconnected --- orange
            WifiState::Disconnected => npx.set_color(255, 165, 0)?,
            // connecting --- yellow
            WifiState::Connecting => npx.set_color(255, 255, 0)?,
            // successfully connected --- all green across the board!
            WifiState::Connected => npx.set_color(0, 255, 0)?,
        };

        Ok(())
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
