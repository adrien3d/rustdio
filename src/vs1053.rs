use std::{thread::sleep, time::Duration};

use esp_idf_hal::{
    gpio::{InputPin, OutputPin, PinDriver},
    spi::{config, SpiDeviceDriver, SpiDriver, SpiError},
    units::FromValueType,
};
use esp_idf_sys::EspError;
use log::{info, warn};

pub struct VS1053<'d, /*XRST,*/ XCS, XDCS, DREQ>
where
    //XRST: OutputPin,
    XCS: OutputPin,
    XDCS: OutputPin,
    DREQ: InputPin,
{
    pub(crate) spi: SpiDriver<'d>,
    //pub(crate) xrst_pin: XRST,
    pub(crate) xcs_pin: XCS,
    #[allow(dead_code)]
    pub(crate) xdcs_pin: XDCS,
    #[allow(dead_code)]
    pub(crate) dreq_pin: DREQ,
}

impl<'d, /*XRST,*/ XCS, XDCS, DREQ> VS1053<'d, /*XRST, */ XCS, XDCS, DREQ>
where
    //XRST: OutputPin,
    XCS: OutputPin,
    XDCS: OutputPin,
    DREQ: InputPin,
{
    pub fn new(
        spi: SpiDriver<'d>,
        //xrst_pin: XRST,
        xcs_pin: XCS,
        xdcs_pin: XDCS,
        dreq_pin: DREQ,
    ) -> Self {
        VS1053 {
            spi,
            //xrst_pin,
            xcs_pin,
            xdcs_pin,
            dreq_pin,
        }
    }

    fn _await_data_request(&mut self) {
        let dreq = match PinDriver::input(&mut self.dreq_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Get DREQ pin failed because: {:?}", err);
                return;
            }
        };
        for _i in 0..=2000 {
            if !dreq.is_high() {
                sleep(Duration::from_millis(10));
            } else {
                break;
            }
        }
    }

    fn _control_mode_on(&mut self) -> Result<(), EspError> {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XCS pin failed because: {:?}", err);
                return Err(err);
            }
        };
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return Err(err);
            }
        };
        let _ = xcs.set_low();
        xdcs.set_high()
    }

    fn _control_mode_off(&mut self) -> Result<(), EspError> {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XCS pin failed because: {:?}", err);
                return Err(err);
            }
        };
        xcs.set_high()
    }

    fn _data_mode_on(&mut self) -> Result<(), EspError> {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XCS pin failed because: {:?}", err);
                return Err(err);
            }
        };
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return Err(err);
            }
        };
        let _ = xcs.set_high();
        xdcs.set_low()
    }

    fn _data_mode_off(&mut self) -> Result<(), EspError> {
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return Err(err);
            }
        };
        xdcs.set_high()
    }

    fn _read_register(&mut self, _address: u8) -> Result<u16, SpiError> {
        Ok(0)
    }

    fn _sdi_send_buffer(&mut self, _data: &u8, _length: usize) {}

    fn _sdi_send_fillers(&mut self, _length: usize) {}

    fn _wram_write(&mut self, _address: u16, _data: u16) {}

    fn _wram_read(&mut self, _address: u16) -> Result<u16, SpiError> {
        Ok(0)
    }

    //https://github.com/baldram/ESP_VS1053_Library/blob/master/src/VS1053.cpp

    // pub fn reset(&mut self) {
    //     let mut xrst = match PinDriver::output(&mut self.xrst_pin) {
    //         Ok(pin) => pin,
    //         Err(err) => {
    //             warn!("Set XRST pin failed because: {:?}", err);
    //             return;
    //         }
    //     };
    //     xrst.set_low().ok();
    //     sleep(Duration::from_millis(10));
    //     xrst.set_high().ok();
    //     sleep(Duration::from_millis(10));
    // }

    pub fn begin(&mut self) {
        let config_1 = config::Config::new().baudrate(26_u32.MHz().into());
        let mut spi_device =
            match SpiDeviceDriver::new(&self.spi, Some(&mut self.xcs_pin), &config_1) {
                Ok(spi_conn) => spi_conn,
                Err(err) => {
                    warn!("VS1053 begin failed because: {:?}", err);
                    return;
                }
            };

        let mut buffer = [0u8; 4];
        match spi_device.transfer(&mut buffer, &[0xAA, 0xBB, 0xCC, 0xDD]) {
            Ok(_res) => info!("spi transfer succeded"),
            Err(err) => {
                warn!("VS1053 begin failed because: {:?}", err);
                return;
            }
        }

        println!("Received: {:?}", buffer);
    }

    pub fn start_song(&mut self) {}

    pub fn play_chunk(&mut self, _data: &u8, _size: usize) {}

    pub fn stop_song(&mut self) {}

    pub fn set_volume(&mut self) {}

    pub fn set_balance(&mut self) {}

    pub fn set_tone(&mut self) {}

    pub fn get_volume(&mut self) {}

    pub fn get_balance(&mut self) {}

    pub fn print_details(&mut self) {}

    pub fn soft_reset(&mut self) {}

    pub fn test_comm(&mut self) {}

    pub fn data_request(&mut self) {}

    pub fn adjust_rate(&mut self) {}

    pub fn stream_mode_on(&mut self) {}

    pub fn stream_mode_off(&mut self) {}

    pub fn switch_to_mp3_mode(&mut self) {}

    pub fn disable_i2s_out(&mut self) {}

    pub fn enable_i2s_out(&mut self) {}

    pub fn is_chip_connected(&mut self) {}

    pub fn get_chip_version(&mut self) {}

    pub fn get_decoded_time(&mut self) {}

    pub fn clear_decoded_time(&mut self) {}

    pub fn write_register(&mut self) {}

    pub fn load_user_code(&mut self) {}

    pub fn load_default_vs1053_patches(&mut self) {}
}

// use esp_idf_hal::spi::{SpiDeviceDriver, SpiDriver};
// use esp_idf_hal::gpio::Pin;
// use esp_idf_hal::prelude::*;

// struct VS1053<'a> {
//     device: SpiDeviceDriver<'a>,
// }

// impl VS1053<'_>{
//     pub fn new<'a>(spi: &'a mut SpiDriver<'a>, cs_pin: Pin) -> Result<VS1053<'a>, esp_idf_hal::spi::SpiDeviceDriverError> {
//         let device = SpiDeviceDriver::new_single(spi, cs_pin)?;
//         Ok(VS1053 { device })
//     }

//     pub fn write_command(&mut self, command: u8, data: u16) -> Result<(), esp_idf_hal::spi::SpiDeviceDriverError> {
//         let mut buf = [0u8; 4];
//         buf[0] = command;
//         buf[1] = (data >> 8) as u8;
//         buf[2] = data as u8;
//         self.device.write(&buf)?;
//         Ok(())
//     }

//     pub fn read_command(&mut self, command: u8) -> Result<u16, esp_idf_hal::spi::SpiDeviceDriverError> {
//         let mut buf = [0u8; 4];
//         buf[0] = command;
//         self.device.transfer(&mut buf)?;
//         let data = ((buf[1] as u16) << 8) | (buf[2] as u16);
//         Ok(data)
//     }
// }

// fn main() -> Result<(), esp_idf_hal::spi::SpiDeviceDriverError> {
//     esp_idf_sys::link_patches();

//     let peripherals = Peripherals::take().unwrap();
//     let mut spi = SpiDriver::new(
//         peripherals.spi2,
//         Pin::new(23), // SCK
//         Pin::new(21), // MOSI
//         Some(Pin::new(19)), // MISO
//         None, // CS (managed by VS1053)
//         1000000, // 1 MHz
//         SpiMode::Mode0,
//     )?;

//     let mut vs1053 = VS1053::new(&mut spi, Pin::new(22))?;

//     // Initialize VS1053 here...

//     // Example: Set volume to 50%
//     vs1053.write_command(0xB0, 0x1010)?;

//     // Example: Read current volume
//     let volume = vs1053.read_command(0xB0)?;
//     println!("Volume: {:#06x}", volume);

//     Ok(())
// }
