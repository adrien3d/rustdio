use std::{thread::sleep, time::Duration};

use esp_idf_hal::{
    gpio::{InputPin, OutputPin, PinDriver},
    spi::SpiDriver,
};
use log::warn;

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
    pub(crate) xdcs_pin: XDCS,
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

    pub fn begin(&mut self) {}

    // uint8_t VS1053::printVersion(){
    //     uint16_t reg = wram_read(0x1E02) & 0xFF;
    //     return reg;
    // }

    // uint32_t VS1053::printChipID(){
    //     uint32_t chipID = 0;
    //     chipID =  wram_read(0x1E00) << 16;
    //     chipID += wram_read(0x1E01);
    //     return chipID;
    // }

    // pub fn send_command(&mut self, command: u8, argument: u16) {
    //     // Set the command mode
    //     self.cs.set_low().ok();

    //     // Send the command
    //     self.spi.write(&[command, (argument >> 8) as u8, argument as u8]).ok();

    //     // Wait for the operation to finish
    //     while self.dreq.is_low().ok() {}

    //     self.cs.set_high().ok();
    // }

    // Additional methods for VS1053 interaction
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
