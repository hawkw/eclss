use anyhow::Context;
use embedded_svc::wifi::{AccessPointConfiguration, ClientConfiguration, Configuration, Wifi};
use esp_idf_hal::{modem::Modem, peripheral::Peripheral};
use esp_idf_svc::{
    eventloop::*,
    netif::{EspNetif, EspNetifWait},
    ping,
    wifi::{EspWifi, WifiWait},
};

use std::{net::Ipv4Addr, time::Duration};

pub fn bringup(
    modem: impl Peripheral<P = Modem> + 'static,
    sysloop: &EspSystemEventLoop,
    ssid: &str,
    pass: &str,
) -> anyhow::Result<Box<EspWifi<'static>>> {
    log::info!("bringing up WiFi...");
    // XXX(eliza): for some reason this line crashes the board...
    let mut wifi = Box::new(EspWifi::new(modem, sysloop.clone(), None)?);

    log::info!("scanning for access points...");
    let aps = Wifi::scan(&mut *wifi).context("scanning for access points")?;

    let mut channel = None;
    for ap in aps {
        log::debug!("Found AP: {ap:?}");
        if ap.ssid == ssid {
            channel = Some(ap.channel);
        }
    }

    if channel.is_none() {
        log::warn!("could not find desired AP SSID {ssid} in scan results");
    } else {
        log::info!("found acccess point for {ssid} on {channel:?}");
    }

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: ssid.into(),
            password: pass.into(),
            channel,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "eclss".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    wifi.start()?;

    log::info!("Starting wifi...");

    if !WifiWait::new(&sysloop)?
        .wait_with_timeout(Duration::from_secs(20), || wifi.is_started().unwrap())
    {
        anyhow::bail!("Wifi did not start");
    }

    log::info!("Connecting wifi...");

    wifi.connect()?;

    if !EspNetifWait::new::<EspNetif>(wifi.sta_netif(), &sysloop)?.wait_with_timeout(
        Duration::from_secs(20),
        || {
            wifi.is_connected().unwrap()
                && wifi.sta_netif().get_ip_info().unwrap().ip != Ipv4Addr::new(0, 0, 0, 0)
        },
    ) {
        anyhow::bail!("Wifi did not connect or did not receive a DHCP lease");
    }

    let ip_info = wifi.sta_netif().get_ip_info()?;

    log::info!("WiFi DHCP info: {ip_info:?}");

    ping(ip_info.subnet.gateway)?;

    Ok(wifi)
}

pub fn ping(ip: Ipv4Addr) -> anyhow::Result<()> {
    log::info!("pinging {ip}...");

    let ping_summary = ping::EspPing::default().ping(ip, &Default::default())?;
    if ping_summary.transmitted != ping_summary.received {
        anyhow::bail!("pinging IP {ip} timed out");
    }

    log::info!("pinging {ip} done");

    Ok(())
}
