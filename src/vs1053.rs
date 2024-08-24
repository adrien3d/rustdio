use std::{thread::sleep, time::Duration};

use esp_idf_hal::{
    gpio::{InputPin, OutputPin, PinDriver},
    spi::{config, SpiDeviceDriver, SpiDriver, SpiError},
    units::FromValueType,
};
use esp_idf_sys::touch_high_volt_t;
use log::{info, warn};

pub struct VS1053<'d, XRST, XCS, XDCS, DREQ>
where
    XRST: OutputPin,
    XCS: OutputPin,
    XDCS: OutputPin,
    DREQ: InputPin,
{
    pub(crate) spi: SpiDriver<'d>,
    pub(crate) xrst_pin: XRST,
    pub(crate) xcs_pin: XCS,
    #[allow(dead_code)]
    pub(crate) xdcs_pin: XDCS,
    #[allow(dead_code)]
    pub(crate) dreq_pin: DREQ,
}

impl<'d, XRST, XCS, XDCS, DREQ> VS1053<'d, XRST, XCS, XDCS, DREQ>
where
    XRST: OutputPin,
    XCS: OutputPin,
    XDCS: OutputPin,
    DREQ: InputPin,
{
    pub fn new(
        spi: SpiDriver<'d>,
        xrst_pin: XRST,
        xcs_pin: XCS,
        xdcs_pin: XDCS,
        dreq_pin: DREQ,
    ) -> Self {
        VS1053 {
            spi,
            xrst_pin,
            xcs_pin,
            xdcs_pin,
            dreq_pin,
        }
    }

    //https://github.com/baldram/ESP_VS1053_Library/blob/master/src/VS1053.cpp

    pub fn reset(&mut self) {
        let mut xrst = match PinDriver::output(&mut self.xrst_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XRST pin failed because: {:?}", err);
                return;
            }
        };
        xrst.set_low().ok();
        sleep(Duration::from_millis(10));
        xrst.set_high().ok();
        sleep(Duration::from_millis(10));
    }

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

    fn await_data_request(&mut self) {
        let mut dreq = match PinDriver::input(&mut self.dreq_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Get DREQ pin failed because: {:?}", err);
                return;
            },
        };
        for _i in 0..= 2000 {
            if !dreq.is_high() {
                sleep(Duration::from_millis(10));
            } else {
                break
            }
        } 
    }

    fn control_mode_on(&mut self) {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XCS pin failed because: {:?}", err);
                return;
            }
        };
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return;
            }
        };
        xcs.set_low();
        xdcs.set_high();
    }

    fn control_mode_off(&mut self) {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XCS pin failed because: {:?}", err);
                return;
            }
        };
        xcs.set_high();
    }

    fn data_mode_on(&mut self) {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XCS pin failed because: {:?}", err);
                return;
            }
        };
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return;
            }
        };
        xcs.set_high();
        xdcs.set_low();
    }

    fn data_mode_off(&mut self) {
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return;
            }
        };
        xdcs.set_high();
    }

    fn read_register(&mut self, address: u8) -> Result<u16, SpiError> {
        Ok(0)
    }

    fn sdi_send_buffer(&mut self, data: &u8, length: usize) {
        
    }

    fn sdi_send_fillers(&mut self, length: usize) {
        
    }

    fn wram_write(&mut self, address: u16, data: u16) {
        
    }

    fn wram_read(&mut self, address: u16) -> Result<u16, SpiError> {
        Ok(0)
    }
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
