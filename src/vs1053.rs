use anyhow::{Error, Result};
use embedded_hal::spi::{Operation, SpiDevice};
use esp_idf_hal::{
    gpio::{Gpio4, Gpio47, Gpio5, InputPin, OutputPin, PinDriver},
    prelude::Peripherals,
    spi::{
        config::{Config as SpiConfig, DriverConfig},
        Dma, SpiDeviceDriver, SpiDriver,
    },
    units::FromValueType,
};
use log::{info, warn};
use std::{thread::sleep, time::Duration};

pub struct VS1053<'a, XCS, XDCS, DREQ> {
    spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
    low_speed_spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
    xcs_pin: XCS,
    xdcs_pin: XDCS,
    dreq_pin: DREQ,
}

impl VS1053<'_, Gpio5, Gpio47, Gpio4> {
    pub fn new(peripherals: Peripherals) -> Result<Self, Error> {
        //let peripherals = Peripherals::take().unwrap();
        let xdcs_pin = peripherals.pins.gpio47; //(instead of 32 normally, but not available on yurobot)
        let xcs_pin = peripherals.pins.gpio5;
        // let en = EN/RST;
        //let xrst_pin = peripherals.pins.gpio0; //TODO: choose an appropriate pin, if needed
        let dreq_pin = peripherals.pins.gpio4;
        let sck_pin = peripherals.pins.gpio18;
        let mosi_pin = peripherals.pins.gpio21; //(instead of 23 normally, but not available on yurobot)
        let miso_pin = peripherals.pins.gpio19;

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
        let spi_device = SpiDeviceDriver::new(spi_driver, Some(xcs_pin), &spi_config)?;
        let low_spi_device = SpiDeviceDriver::new(spi_driver, Some(xcs_pin), &low_spi_config)?;

        Ok(Self {
            spi: spi_device,
            low_speed_spi: low_spi_device,
            xcs_pin,
            xdcs_pin,
            dreq_pin,
        })
    }

    fn set_dcs_pin(&mut self, is_high: bool) -> Result<(), DSPError> {
        let mut xdcs = match PinDriver::output(&mut self.xdcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set XDCS pin failed because: {:?}", err);
                return Err(DSPError::UnableToSetDCSPin);
            }
        };
        if is_high {
            xdcs.set_high();
        } else {
            xdcs.set_low();
        }
        Ok(())
    }

    fn await_data_request(&mut self) -> Result<(), DSPError> {
        let dreq = match PinDriver::input(&mut self.dreq_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!(
                    "Get DREQ pin for _await_data_request failed because: {:?}",
                    err
                );
                Err(DSPError::UnableToGetDREQPin)
            }?,
        };
        for _i in 0..=2000 {
            if !dreq.is_high() {
                sleep(Duration::from_millis(1));
            } else {
                return Ok(());
            }
        }
        Err(DSPError::DataRequestTimeout)
    }

    fn control_mode_on(&mut self) -> Result<(), DSPError> {
        // let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
        //     Ok(pin) => pin,
        //     Err(err) => {
        //         warn!("Set XCS pin for _control_mode_on failed because: {:?}", err);
        //         return Err(err);
        //     }
        // };
        // let _ = xcs.set_low();
        self.set_dcs_pin(true)
    }

    fn control_mode_off(&mut self) -> Result<(), DSPError> {
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

    fn _data_mode_on(&mut self) -> Result<(), DSPError> {
        // let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
        //     Ok(pin) => pin,
        //     Err(err) => {
        //         warn!("Set XCS pin for _data_mode_on failed because: {:?}", err);
        //         return Err(err);
        //     }
        // };
        // let _ = xcs.set_high();
        self.set_dcs_pin(false)
    }

    fn _data_mode_off(&mut self) -> Result<(), DSPError> {
        self.set_dcs_pin(true)
    }

    fn read_register(&mut self, _address: u8) -> Result<u16, DSPError> {
        Ok(0)
    }

    fn _sdi_send_buffer(&mut self, _data: &u8, _length: usize) {}

    fn _sdi_send_fillers(&mut self, _length: usize) {}

    fn _wram_write(&mut self, _address: u16, _data: u16) {}

    fn _wram_read(&mut self, _address: u16) -> Result<u16, DSPError> {
        Ok(0)
    }

    //https://github.com/baldram/ESP_VS1053_Library/blob/master/src/VS1053.cpp#L149
    pub fn begin(&mut self) -> Result<[u8; 2], DSPError> {
        self.set_dcs_pin(true)?;
        //set cs High
        sleep(Duration::from_millis(100));
        info!("Reset VS1053... \n");
        self.set_dcs_pin(false)?;
        //set cs Low
        sleep(Duration::from_millis(500));
        info!("End reset VS1053... \n");
        self.set_dcs_pin(true)?;
        //set cs High
        sleep(Duration::from_millis(500));

        let mut buf = [0; 2];

        // `transaction` asserts and deasserts CS for us. No need to do it manually!
        self.spi
            .transaction(&mut [Operation::Write(&[0x90]), Operation::Read(&mut buf)])
            .map_err(|error| {
                log::warn!("Failed to make SPI transaction for begin: {error:?}");
                DSPError::Spi
            })?;

        Ok(buf)
    }

    fn write_register(&mut self, reg: u8, value: u16) -> Result<(), DSPError> {
        let lsb: u8 = (value & 0xFF) as u8;
        let msb: u8 = (value >> 8) as u8;
        self.control_mode_on()?;
        let mut buf = [0; 0];

        // `transaction` asserts and deasserts CS for us. No need to do it manually!
        self.spi
            .transaction(&mut [Operation::Write(&[0x2, reg, msb, lsb])])
            .map_err(|error| {
                log::warn!("Failed to make SPI transaction for write_register: {error:?}");
                DSPError::Spi
            })?;

        self.await_data_request()?;
        self.control_mode_off()?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum DSPError {
    Spi,
    UnableToSetCSPin,
    UnableToSetDCSPin,
    UnableToGetDREQPin,
    DataRequestTimeout,
}
