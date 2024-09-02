use anyhow::Result;
use embedded_hal::spi::{Operation, SpiDevice};
use esp_idf_hal::gpio::{InputPin, OutputPin, PinDriver};
use log::warn;
use std::{thread::sleep, time::Duration};

pub struct VS1053<SPI, XCS, XDCS, DREQ> {
    spi: SPI,
    low_spi: SPI,
    xcs_pin: XCS,
    xdcs_pin: XDCS,
    dreq_pin: DREQ,
}

impl<SPI, XCS, XDCS, DREQ> VS1053<SPI, XCS, XDCS, DREQ>
where
    SPI: SpiDevice,
    XCS: OutputPin,
    XDCS: OutputPin,
    DREQ: InputPin,
{
    pub fn new(spi: SPI, low_spi: SPI, xcs_pin: XCS, xdcs_pin: XDCS, dreq_pin: DREQ) -> Self {
        Self {
            spi,
            low_spi,
            xcs_pin,
            xdcs_pin,
            dreq_pin,
        }
    }

    fn set_cs_pin(&mut self, is_high: bool) -> Result<(), DSPError> {
        let mut xcs = match PinDriver::output(&mut self.xcs_pin) {
            Ok(pin) => pin,
            Err(err) => {
                warn!("Set DCS pin failed because: {:?}", err);
                return Err(DSPError::UnableToSetCSPin);
            }
        };
        if is_high {
            xcs.set_high();
        } else {
            xcs.set_low();
        }
        Ok(())
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
        self.set_dcs_pin(true);
        self.set_cs_pin(false)
    }

    fn control_mode_off(&mut self) -> Result<(), DSPError> {
        self.set_cs_pin(true)
    }

    fn _data_mode_on(&mut self) -> Result<(), DSPError> {
        self.set_cs_pin(true);
        self.set_dcs_pin(false)
    }

    fn _data_mode_off(&mut self) -> Result<(), DSPError> {
        self.set_dcs_pin(true)
    }

    fn _sdi_send_buffer(&mut self, _data: &u8, _length: usize) {}

    fn _sdi_send_fillers(&mut self, _length: usize) {}

    fn _wram_write(&mut self, _address: u16, _data: u16) {}

    fn _wram_read(&mut self, _address: u16) -> Result<u16, DSPError> {
        Ok(0)
    }

    pub fn begin(&mut self) -> Result<(), DSPError> {
        self.set_dcs_pin(true)?;
        self.set_cs_pin(true)?;
        sleep(Duration::from_millis(100));
        log::info!("Reset VS1053... \n");
        self.set_dcs_pin(false)?;
        self.set_cs_pin(false)?;
        sleep(Duration::from_millis(500));
        log::info!("End reset VS1053... \n");
        self.set_dcs_pin(true)?;
        self.set_cs_pin(true)?;
        sleep(Duration::from_millis(500));

        if (testComm("Slow SPI,Testing VS1053 read/write registers...\n")) {
            // SLOWSPI
            self.write_register(SCI_AUDATA, 44101); // 44.1kHz stereo
                                                    // The next clocksetting allows SPI clocking at 5 MHz, 4 MHz is safe then.
            self.write_register(SCI_CLOCKF, 6 << 12); // Normal clock settings multiplyer 3.0 = 12.2 MHz
                                                      // SPI Clock to 4 MHz. Now you can set high speed SPI clock.

            // FASTSPI
            self.write_register(SCI_MODE, _BV(SM_SDINEW) | _BV(SM_LINE1));
            //TODO: testComm("Fast SPI, Testing VS1053 read/write registers again...\n");
            sleep(Duration::from_millis(10));
            self.await_data_request();
            let end_fill_byte = self._wram_read(0x1E06) & 0xFF;
            log::info!("endFillByte is %X\n", end_fill_byte);
            //printDetails("After last clocksetting") ;
            sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    fn read_register(&mut self, address: u8) -> Result<u16, DSPError> {
        let result: u16;

        self.control_mode_on();
        let mut buf = [0; 0];
        self.spi
            .transaction(&mut [Operation::Write(&[0x3, address]), Operation::Read(&mut buf)])
            .map_err(|error| {
                log::warn!("Failed to make SPI transaction for read_register: {error:?}");
                DSPError::Spi
            })?;

        self.await_data_request(); // Wait for DREQ to be HIGH again
        self.control_mode_off();
        Ok(u16::from_be_bytes(buf))
    }

    fn write_register(&mut self, reg: u8, value: u16) -> Result<(), DSPError> {
        let lsb: u8 = (value & 0xFF) as u8;
        let msb: u8 = (value >> 8) as u8;
        self.control_mode_on()?;

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
