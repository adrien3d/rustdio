use anyhow::Result;
use embedded_hal::spi::{Operation, SpiDevice};
use esp_idf_hal::gpio::{InputPin, OutputPin, PinDriver};
use log::warn;
use std::{thread::sleep, time::Duration};

const VS1053_CHUNK_SIZE: u8 = 32;
// SCI Register
const SCI_MODE: u8 = 0x0;
const SCI_STATUS: u8 = 0x1;
const SCI_BASS: u8 = 0x2;
const SCI_CLOCKF: u8 = 0x3;
const SCI_DECODE_TIME: u8 = 0x4; // current decoded time in full seconds
const SCI_AUDATA: u8 = 0x5;
const SCI_WRAM: u8 = 0x6;
const SCI_WRAMADDR: u8 = 0x7;
const SCI_AIADDR: u8 = 0xA;
const SCI_VOL: u8 = 0xB;
const SCI_AICTRL0: u8 = 0xC;
const SCI_AICTRL1: u8 = 0xD;
const SCI_NUM_REGISTERS: u8 = 0xF;
// SCI_MODE bits
const SM_SDINEW: u8 = 11; // Bitnumber in SCI_MODE always on
const SM_RESET: u8 = 2; // Bitnumber in SCI_MODE soft reset
const SM_CANCEL: u8 = 3; // Bitnumber in SCI_MODE cancel song
const SM_TESTS: u8 = 5; // Bitnumber in SCI_MODE for tests
const SM_LINE1: u8 = 14; // Bitnumber in SCI_MODE for Line input
const SM_STREAM: u8 = 6; // Bitnumber in SCI_MODE for Streaming Mode

const ADDR_REG_GPIO_DDR_RW: u16 = 0xc017;
const ADDR_REG_GPIO_VAL_R: u16 = 0xc018;
const ADDR_REG_GPIO_ODATA_RW: u16 = 0xc019;
const ADDR_REG_I2S_CONFIG_RW: u16 = 0xc040;

macro_rules! _bv {
    ($bit:expr) => {
        1 << $bit
    };
}

pub struct VS1053<SPI, XCS, XDCS, DREQ> {
    spi: SPI,
    low_spi: SPI,
    xcs_pin: XCS,
    xdcs_pin: XDCS,
    dreq_pin: DREQ,
    current_volume: i8,
    current_balance: i8,
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
            current_volume: 50,
            current_balance: 0,
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

        if (self.testComm("Slow SPI,Testing VS1053 read/write registers...\n".as_ptr())) {
            // SLOWSPI
            self.write_register(SCI_AUDATA, 44101); // 44.1kHz stereo
                                                    // The next clocksetting allows SPI clocking at 5 MHz, 4 MHz is safe then.
            self.write_register(SCI_CLOCKF, 6 << 12); // Normal clock settings multiplyer 3.0 = 12.2 MHz
                                                      // SPI Clock to 4 MHz. Now you can set high speed SPI clock.

            // FASTSPI
            self.write_register(SCI_MODE, _bv!(SM_SDINEW) | _bv!(SM_LINE1));
            //TODO: testComm("Fast SPI, Testing VS1053 read/write registers again...\n");
            sleep(Duration::from_millis(10));
            self.await_data_request();

            let efb = self._wram_read(0x1E06)?;
            let end_fill_byte = efb & 0xFF;
            log::info!("endFillByte is {:X}\n", end_fill_byte);
            //printDetails("After last clocksetting") ;
            sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    fn read_register(&mut self, address: u8) -> Result<u16, DSPError> {
        let result: u16;

        self.control_mode_on();
        let mut buf: [u8; 2] = [0; 2];
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

    fn testComm(&mut self, header: *const u8) -> bool {
        // Test the communication with the VS1053 module.  The result wille be returned.
        // If DREQ is low, there is problably no VS1053 connected.  Pull the line HIGH
        // in order to prevent an endless loop waiting for this signal.  The rest of the
        // software will still work, but readbacks from VS1053 will fail.
        let i: i32; // Loop control
        let (r1, r2, cnt): (u16, u16, u16) = (0, 0, 0);
        let delta: u16 = 300; // 3 for fast SPI
    
        // if (!digitalRead(dreq_pin)) {
        //     LOG("VS1053 not properly installed!\n");
        //     // Allow testing without the VS1053 module
        //     pinMode(dreq_pin, INPUT_PULLUP); // DREQ is now input with pull-up
        //     return false;                    // Return bad result
        // }
        // // Further TESTING.  Check if SCI bus can write and read without errors.
        // // We will use the volume setting for this.
        // // Will give warnings on serial output if DEBUG is active.
        // // A maximum of 20 errors will be reported.
        // if (strstr(header, "Fast")) {
        //     delta = 3; // Fast SPI, more loops
        // }
    
        // LOG("%s", header);  // Show a header
    
        // for (i = 0; (i < 0xFFFF) && (cnt < 20); i += delta) {
        //     writeRegister(SCI_VOL, i);         // Write data to SCI_VOL
        //     r1 = read_register(SCI_VOL);        // Read back for the first time
        //     r2 = read_register(SCI_VOL);        // Read back a second time
        //     if (r1 != r2 || i != r1 || i != r2) // Check for 2 equal reads
        //     {
        //         LOG("VS1053 error retry SB:%04X R1:%04X R2:%04X\n", i, r1, r2);
        //         cnt++;
        //         delay(10);
        //     }
        //     yield(); // Allow ESP firmware to do some bookkeeping
        // }
        return cnt == 0; // Return the result
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
