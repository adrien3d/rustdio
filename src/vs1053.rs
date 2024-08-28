use embedded_hal::spi::{SpiDevice, Operation};
use esp_idf_hal::gpio::{InputPin, OutputPin, PinDriver};
use esp_idf_sys::EspError;
use log::warn;
use std::{thread::sleep, time::Duration};

pub struct VS1053<SPI, /*XCS,*/ XDCS, DREQ> {
    spi: SPI,
    //xcs_pin: XCS,
    xdcs_pin: XDCS,
    dreq_pin: DREQ
}

impl<SPI, /*XCS,*/ XDCS, DREQ> VS1053<SPI, /*XCS,*/ XDCS, DREQ>
where
    SPI: SpiDevice,
    //XCS: OutputPin,
    XDCS: OutputPin,
    DREQ: InputPin,
{
    pub fn new(spi: SPI, /*xcs_pin: XCS,*/ xdcs_pin: XDCS, dreq_pin: DREQ) -> Self {
        Self { spi, /*xcs_pin,*/ xdcs_pin, dreq_pin }
    }

    fn _await_data_request(&mut self) {
        let dreq = match PinDriver::input(&mut self.dreq_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Get DREQ pin for _await_data_request failed because: {:?}", err);
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
        // let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
        //     Ok(pin) => pin,
        //     Err(err) => {
        //         warn!("Set XCS pin for _control_mode_on failed because: {:?}", err);
        //         return Err(err);
        //     }
        // };
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin for _control_mode_on failed because: {:?}", err);
                return Err(err);
            }
        };
        // let _ = xcs.set_low();
        xdcs.set_high()
    }

    fn _control_mode_off(&mut self) -> Result<(), EspError> {
        // let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
        //     Ok(pin) => pin,
        //     Err(err) => {
        //         warn!("Set XCS pin for _control_mode_off failed because: {:?}", err);
        //         return Err(err);
        //     }
        // };
        // xcs.set_high()
        Ok(())
    }

    fn _data_mode_on(&mut self) -> Result<(), EspError> {
        // let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
        //     Ok(pin) => pin,
        //     Err(err) => {
        //         warn!("Set XCS pin for _data_mode_on failed because: {:?}", err);
        //         return Err(err);
        //     }
        // };
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin for _data_mode_on failed because: {:?}", err);
                return Err(err);
            }
        };
        // let _ = xcs.set_high();
        xdcs.set_low()
    }

    fn _data_mode_off(&mut self) -> Result<(), EspError> {
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin for _data_mode_off failed because: {:?}", err);
                return Err(err);
            }
        };
        xdcs.set_high()
    }

    fn _read_register(&mut self, _address: u8) -> Result<u16, EspError> {
        Ok(0)
    }

    fn _sdi_send_buffer(&mut self, _data: &u8, _length: usize) {}

    fn _sdi_send_fillers(&mut self, _length: usize) {}

    fn _wram_write(&mut self, _address: u16, _data: u16) {}

    fn _wram_read(&mut self, _address: u16) -> Result<u16, EspError> {
        Ok(0)
    }

    pub fn begin(&mut self) -> Result<[u8; 2], MyError<SPI::Error>> {
        let mut buf = [0; 2];

        // `transaction` asserts and deasserts CS for us. No need to do it manually!
        self.spi.transaction(&mut [
            Operation::Write(&[0x90]),
            Operation::Read(&mut buf),
        ]).map_err(MyError::Spi)?;

        Ok(buf)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MyError<SPI> {
    Spi(SPI),
    // Add other errors for your driver here.
}
