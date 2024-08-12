use anyhow::{bail, Result};
// use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::nvs::{EspNvsPartition, NvsDefault};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    sntp::{EspSntp, SyncStatus},
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::info;
// use std::{thread::sleep, time::Duration};

pub fn wifi(
    ssid: &str,
    pass: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
    nvs_default_partition: EspNvsPartition<NvsDefault>,
) -> Result<Box<EspWifi<'static>>> {
    let mut auth_method = AuthMethod::WPA2Personal;
    if ssid.is_empty() {
        bail!("Missing WiFi name")
    }
    if pass.is_empty() {
        auth_method = AuthMethod::None;
        info!("Wifi password is empty");
    }
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs_default_partition))?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;

    info!("Starting wifi...");

    wifi.start()?;

    info!("Scanning...");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == ssid);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            ssid, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            ssid
        );
        None
    };

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid
            .try_into()
            .expect("Could not parse the given SSID into WiFi config"),
        password: pass
            .try_into()
            .expect("Could not parse the given password into WiFi config"),
        channel,
        auth_method,
        ..Default::default()
    }))?;

    info!("Connecting wifi...");

    wifi.connect()?;

    info!("Waiting for DHCP lease...");

    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi Connected: DHCP info: {:?}", ip_info);

    // Synchronize NTP
    println!("Synchronizing with NTP Server");
    match EspSntp::new_default() {
        Ok(ntp) => {
            while ntp.get_sync_status() != SyncStatus::Completed {}
            info!("NTP Time Sync Completed");
        }
        Err(err) => info!("NTP Time Sync not done in a sec:{:#?}", err),
    }

    Ok(Box::new(esp_wifi))
}
