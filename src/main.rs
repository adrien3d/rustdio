use anyhow::Result;
use chrono::{DateTime, Utc};
use core::str;
use embedded_svc::{
    http::{Headers, Method},
    io::Write,
};
use esp_idf_hal::{
    gpio::AnyOutputPin,
    io::Read,
    spi::{
        config::{Config as SpiConfig, DriverConfig},
        Dma, SpiDeviceDriver, SpiDriver,
    },
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        i2c::{I2cConfig, I2cDriver},
        io::EspIOError,
        prelude::*,
    },
    http::server::{Configuration, EspHttpServer},
    nvs::*,
};
use log::{info, warn};
use vs1053::VS1053;
mod ntp;
use postcard::{from_bytes, to_vec};
use radios::Station;
use rgb_led::{RGB8, WS2812RMT};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::{Duration, SystemTime},
};
use tea5767::defs::{BandLimits, SoundMode, TEA5767};
mod vs1053;
use wifi::wifi;

mod radios;

#[derive(Debug)]
#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

#[derive(Debug, Deserialize)]
struct FormData<'a> {
    // fm_frequency: f32,
    station: &'a str,
    is_webradio: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct LastConfiguration<'a> {
    last_source: &'a str,
    last_station: &'a str,
    last_volume: u8,
}

// struct ProgramAppState {
//     /// A Network Time Protocol used as a time source.
//     //ntp: ntp::Ntp,
// }

const MAX_CONTROL_PAYLOAD_LEN: usize = 128;
static CONTROL_RADIO_HTML: &str = include_str!("control-radio.html");

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let nvs_default_partition: EspNvsPartition<NvsDefault> = EspDefaultNvsPartition::take()?;

    let test_namespace = "test_ns";
    let nvs = match EspNvs::new(nvs_default_partition.clone(), test_namespace, true) {
        Ok(nvs) => {
            info!("Got namespace {:?} from default partition", test_namespace);
            nvs
        }
        Err(e) => panic!("Could't get namespace {:?}", e),
    };

    let key_raw_struct: &str = "config";
    let key_raw_struct_data: &mut [u8] = &mut [0; 100];
    let mut last_configuration = LastConfiguration {
        last_source: "fm",
        last_station: "france_info",
        last_volume: 50,
    };

    match nvs.get_raw(key_raw_struct, key_raw_struct_data) {
        Ok(v) => {
            if let Some(the_struct) = v {
                info!(
                    "{:?} = {:#?}",
                    key_raw_struct,
                    from_bytes::<LastConfiguration>(the_struct)
                );
                match from_bytes::<LastConfiguration>(the_struct) {
                    Ok(res) => last_configuration = res,
                    Err(e) => warn!("Converting {:#?} failed because: {:?}", the_struct, e),
                }
            }
        }
        Err(e) => warn!("Couldn't get key {} because {:?}", key_raw_struct, e),
    };

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;

    let app_config = CONFIG;
    warn!("app_config:{:#?}", app_config);

    info!("Pre led");
    // Wrap the led in an Arc<Mutex<...>>
    let led = Arc::new(Mutex::new(WS2812RMT::new(
        peripherals.pins.gpio8,
        peripherals.rmt.channel0,
    )?));
    {
        let mut led = led.lock().unwrap();
        led.set_pixel(RGB8::new(50, 0, 0))?;
    }
    info!("Post led");

    // Initialize radio tuner
    let sda = peripherals.pins.gpio6;
    let scl = peripherals.pins.gpio7;
    // let _sen = peripherals.pins.gpio0;
    // let _rst = peripherals.pins.gpio1;
    // let _gpio1 = peripherals.pins.gpio10;
    // let _gpio2 = peripherals.pins.gpio11;
    let config = I2cConfig::new().baudrate(400.kHz().into());
    let i2c = I2cDriver::new(peripherals.i2c0, sda, scl, &config)?;

    let default_station_frequency =
        // Station::get_fm_frequency_from_id("france_info").unwrap_or(105.5);
        Station::get_fm_frequency_from_id(last_configuration.last_station).unwrap_or(105.5);

    let fm_radio_tuner = match TEA5767::new(
        i2c,
        default_station_frequency,
        BandLimits::EuropeUS,
        SoundMode::Stereo,
    ) {
        Ok(tuner) => Arc::new(Mutex::new(tuner)),
        Err(err) => {
            warn!("Unable to initialize TEA5767 I2C:{}", err);
            return Err(err.into());
        }
    };

    // Initialize DAC MP3
    let xdcs_pin = peripherals.pins.gpio47; //(instead of 32 normally, but not available on yurobot)
    let xcs_pin = peripherals.pins.gpio5;
    // let en = EN/RST;
    //let xrst_pin = peripherals.pins.gpio0; //TODO: choose an appropriate pin, if needed
    let dreq_pin = peripherals.pins.gpio4;
    let sck_pin = peripherals.pins.gpio18;
    let mosi_pin = peripherals.pins.gpio21; //(instead of 23 normally, but not available on yurobot)
    let miso_pin = peripherals.pins.gpio19;

    // Set up SPI configuration
    let spi_config = SpiConfig::default().baudrate(4.MHz().into());
    let low_spi_config = SpiConfig::default().baudrate(200.kHz().into());

    // Initialize SPI bus
    let spi_driver = SpiDriver::new(
        peripherals.spi2,
        sck_pin,
        mosi_pin,
        Some(miso_pin),
        &DriverConfig::default().dma(Dma::Auto(4096)),
    )?;

    // Create an SPI device driver on the bus
    let spi_device = SpiDeviceDriver::new(&spi_driver, None::<AnyOutputPin>, &spi_config)?;
    let low_spi_device =
        SpiDeviceDriver::new(&spi_driver, Option::<AnyOutputPin>::None, &low_spi_config)?;
    // you can create different SpiDeviceDrivers, with different configs, and they all can have a different baudrate set on the same bus.
    // If you want to control CS yourself you can just not provide a CS pin in the new() constructor since its a option

    //VS1053 player(VS1053_CS, VS1053_DCS, VS1053_DREQ);
    // WiFiClient client;
    // uint8_t mp3buff[64];
    //let mut mp3_decoder = VS1053::new(spi_driver, /*xrst_pin,*/ xcs_pin, xdcs_pin, dreq_pin);

    let mut mp3_decoder = VS1053::new(spi_device, low_spi_device, xcs_pin, xdcs_pin, dreq_pin);
    log::info!(
        "VS1053 connected:{:?}, chip version:{:?} volume:{:?}",
        mp3_decoder.is_chip_connected(),
        mp3_decoder.get_chip_version(),
        mp3_decoder.get_volume()
    );

    // player.begin();
    // if (player.getChipVersion() == 4) { // Only perform an update if we really are using a VS1053, not. eg. VS1003
    //     player.loadDefaultVs1053Patches();
    // }
    // player.switchToMp3Mode();
    // player.setVolume(VOLUME);

    let res = mp3_decoder.begin();
    log::info!("VS1053.begin():{:#?}", res);
    mp3_decoder.switch_to_mp3_mode();
    let _ = mp3_decoder.set_volume(last_configuration.last_volume);
    mp3_decoder.set_balance(0);
    log::info!(
        "VS1053 MP3 decoder connected:{:?}, chip version:{:?} volume:{:?}",
        mp3_decoder.is_chip_connected(),
        mp3_decoder.get_chip_version(),
        mp3_decoder.get_volume()
    );

    let _wifi = wifi(
        app_config.wifi_ssid,
        app_config.wifi_psk,
        peripherals.modem,
        sysloop,
        nvs_default_partition.clone(),
    )?;

    let _default_station_url =
        // Station::get_fm_frequency_from_id("france_info").unwrap_or(105.5);
        Station::get_web_url_from_id(last_configuration.last_station).unwrap_or("http://europe2.lmn.fm/europe2.mp3");

    // mp3_decoder.play_chunk(data, len);

    // mp3_decoder.connecttohost("streambbr.ir-media-tec.com/berlin/mp3-128/vtuner_web_mp3/");
    // let mut radio = Si4703::new(i2c);
    // radio.enable_oscillator().map_err(|e| format!("Enable oscillator error: {:?}", e));
    // sleep(Duration::from_millis(500));
    // radio.enable().map_err(|e| format!("Enable error: {:?}", e));
    // sleep(Duration::from_millis(110));
    // radio.set_volume(Volume::Dbfsm28).map_err(|e| format!("Volume error: {:?}", e));
    // radio.set_deemphasis(DeEmphasis::Us50).map_err(|e| format!("Deemphasis error: {:?}", e));
    // radio.set_channel_spacing(ChannelSpacing::Khz100).map_err(|e| format!("Channel spacing error: {:?}", e));
    // radio.unmute().map_err(|e: si4703::Error<esp_idf_hal::i2c::I2cError>| format!("Unmute error: {:?}", e));

    let mut server = EspHttpServer::new(&Configuration::default())?;

    // Clone the Arc to pass to the closure
    let led_clone = led.clone();
    server.fn_handler(
        "/",
        Method::Get,
        move |request| -> core::result::Result<(), EspIOError> {
            let html = index_html();
            let mut response = request.into_ok_response()?;
            response.write_all(html.as_bytes())?;
            let mut led = led_clone.lock().unwrap();
            let _ = led.set_pixel(RGB8::new(0, 50, 0));
            Ok(())
        },
    )?;

    server.fn_handler("/radio", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(CONTROL_RADIO_HTML.as_bytes())
            .map(|_| ())
    })?;

    let led_clone = led.clone();
    let fm_radio_tuner_clone = fm_radio_tuner.clone();
    server.fn_handler::<anyhow::Error, _>("/post-radio-form", Method::Post, move |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_CONTROL_PAYLOAD_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        if let Ok(form) = serde_json::from_slice::<FormData>(&buf) {
            let station_name = Station::get_name_from_id(form.station);
            let last_source: &str;
            let last_station: &str = form.station;
            if !form.is_webradio {
                last_source = "fm";
                let fm_frequency = Station::get_fm_frequency_from_id(form.station);
                match fm_frequency {
                    Some(freq) => {
                        let mut fm_radio_tuner = fm_radio_tuner_clone
                            .lock()
                            .map_err(|_| anyhow::anyhow!("Failed to lock radio tuner mutex"))?;
                        fm_radio_tuner
                            .set_frequency(freq)
                            .map_err(|_| anyhow::anyhow!("Failed to set radio tuner frequency"))?;
                        info!("FM Radio set to: {:?}, frequency:{}", form, freq);

                        let mut led = led_clone.lock().unwrap();
                        let _ = led.set_pixel(RGB8::new(0, 0, 0));
                        sleep(Duration::from_millis(100));
                        let _ = led.set_pixel(RGB8::new(0, 50, 0));
                    }
                    None => warn!("FM Radio {:?} [{:?}] not found", station_name, form),
                }
            } else {
                last_source = "webradio";
                let station_url = Station::get_web_url_from_id(form.station);
                match station_url {
                    Some(url) => {
                        info!("WebRadio set to: {:?}, URL:{}", form, url);
                    }
                    None => warn!("Webradio {:?} [{:?}] not found", station_name, form),
                }
            }
            let key_raw_struct_data = LastConfiguration {
                last_source,
                last_station,
                last_volume: 50,
            };
            let mut nvs_clone =
                EspNvs::new(nvs_default_partition.clone(), test_namespace, true).unwrap();
            nvs_clone
                .set_str("last_station", form.station)
                .expect("Failed to set last_station at runtime");
            match nvs_clone.set_raw(
                key_raw_struct,
                &to_vec::<LastConfiguration, 100>(&key_raw_struct_data).unwrap(),
            ) {
                Ok(_) => info!("Key {} updated", key_raw_struct),
                Err(e) => info!("key {} not updated {:?}", key_raw_struct, e),
            };
            write!(
                resp,
                "Requested {} station and {} webradio",
                form.station,
                form.is_webradio // "Requested {} FM and {} station",
                                 // form.fm_frequency, form.web_station
            )?;
        } else {
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    })?;

    // fm_radio_tuner.set_frequency(fm_frequency).unwrap();
    // let _ = fm_radio_tuner.mute();
    // fm_radio_tuner.set_standby();
    // fm_radio_tuner.reset_standby();
    // fm_radio_tuner.set_soft_mute();
    // fm_radio_tuner.search_up();

    warn!("Server awaiting connection");

    loop {
        // Obtain System Time
        let st_now = SystemTime::now();
        // Convert to UTC Time
        let dt_now_utc: DateTime<Utc> = st_now.into();
        // Format Time String
        let formatted = format!("{}", dt_now_utc.format("%d/%m/%Y %H:%M:%S"));
        // Print Time
        info!("Time: {}", formatted);
        sleep(Duration::from_millis(1000));

        // if (client.available() > 0) {
        //     // The buffer size 64 seems to be optimal. At 32 and 128 the sound might be brassy.
        //     uint8_t bytesread = client.read(mp3buff, 64);
        //     player.playChunk(mp3buff, bytesread);
        // }
    }
}

fn templated(content: impl AsRef<str>) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8">
        <title>ISS web server</title>
    </head>
    <body>
        {}
    </body>
</html>
"#,
        content.as_ref()
    )
}

fn index_html() -> String {
    templated("Hello from ISS!")
}

// fn fm_frequency_page(val: f32) -> String {
//     templated(format!("Current FM frequency is: {:.2}", val))
// }
