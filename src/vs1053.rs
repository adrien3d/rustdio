use anyhow::Result;
use core::cmp::max;
use embedded_hal::spi::{Operation, SpiDevice};
use esp_idf_hal::gpio::{InputPin, OutputPin, PinDriver};
use log::warn;
use std::{ffi::CStr, str, thread::sleep, time::Duration};

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

fn contains(str: *const u8, substr: &str) -> bool {
    // Convert the raw pointer to a CStr
    unsafe {
        if str.is_null() {
            return false;
        }

        // Create a CStr from the raw pointer
        let c_str = CStr::from_ptr(str as *const i8);

        // Convert the CStr to a Rust &str
        if let Ok(str_slice) = c_str.to_str() {
            // Check if the &str contains the substring "Fast"
            return str_slice.contains(substr);
        }
    }

    false
}

fn map(x: i64, in_min: i64, in_max: i64, out_min: i64, out_max: i64) -> i64 {
    (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}

pub struct VS1053<SPI, XCS, XDCS, DREQ> {
    spi: SPI,
    low_spi: SPI,
    xcs_pin: XCS,
    xdcs_pin: XDCS,
    dreq_pin: DREQ,
    current_volume: u8,
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
            let _ = xcs.set_high();
        } else {
            let _ = xcs.set_low();
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
            let _ = xdcs.set_high();
        } else {
            let _ = xdcs.set_low();
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
        self.set_dcs_pin(true)?;
        self.set_cs_pin(false)
    }

    fn control_mode_off(&mut self) -> Result<(), DSPError> {
        self.set_cs_pin(true)
    }

    fn _data_mode_on(&mut self) -> Result<(), DSPError> {
        self.set_cs_pin(true)?;
        self.set_dcs_pin(false)
    }

    fn _data_mode_off(&mut self) -> Result<(), DSPError> {
        self.set_dcs_pin(true)
    }

    fn _sdi_send_buffer(&mut self, _data: &u8, _length: usize) {}

    fn _sdi_send_fillers(&mut self, _length: usize) {}

    fn _wram_write(&mut self, address: u16, data: u16) -> Result<(), DSPError> {
        self.write_register(true, SCI_WRAMADDR, address)?;
        self.write_register(true, SCI_WRAM, data)
    }

    fn _wram_read(&mut self, address: u16) -> Result<u16, DSPError> {
        self.write_register(true, SCI_WRAMADDR, address)?; // Start reading from WRAM
        self.read_register(SCI_WRAM) // Read back result
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

        if self.test_comm("Slow SPI,Testing VS1053 read/write registers...\n".as_ptr()) {
            // SLOWSPI
            self.write_register(false, SCI_AUDATA, 44101)?; // 44.1kHz stereo
                                                            // The next clocksetting allows SPI clocking at 5 MHz, 4 MHz is safe then.
            self.write_register(false, SCI_CLOCKF, 6 << 12)?; // Normal clock settings multiplyer 3.0 = 12.2 MHz
                                                              // SPI Clock to 4 MHz. Now you can set high speed SPI clock.

            // FASTSPI
            self.write_register(true, SCI_MODE, _bv!(SM_SDINEW) | _bv!(SM_LINE1))?;
            let _ =
                self.test_comm("Fast SPI, Testing VS1053 read/write registers again...\n".as_ptr());
            sleep(Duration::from_millis(10));
            self.await_data_request()?;

            let efb = self._wram_read(0x1E06)?;
            let end_fill_byte = efb & 0xFF;
            log::info!("endFillByte is {:X}\n", end_fill_byte);
            //printDetails("After last clocksetting") ;
            sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    fn read_register(&mut self, address: u8) -> Result<u16, DSPError> {
        self.control_mode_on()?;
        let mut buf: [u8; 2] = [0; 2];
        self.spi
            .transaction(&mut [Operation::Write(&[0x3, address]), Operation::Read(&mut buf)])
            .map_err(|error| {
                log::warn!("Failed to make SPI transaction for read_register: {error:?}");
                DSPError::Spi
            })?;

        self.await_data_request()?; // Wait for DREQ to be HIGH again
        self.control_mode_off()?;
        Ok(u16::from_be_bytes(buf))
    }

    fn write_register(&mut self, is_high_speed: bool, reg: u8, value: u16) -> Result<(), DSPError> {
        let lsb: u8 = (value & 0xFF) as u8;
        let msb: u8 = (value >> 8) as u8;
        self.control_mode_on()?;

        if is_high_speed {
            self.spi
                .transaction(&mut [Operation::Write(&[0x2, reg, msb, lsb])])
                .map_err(|error| {
                    log::warn!("Failed to make SPI transaction for write_register: {error:?}");
                    DSPError::Spi
                })?;
        } else {
            self.low_spi
                .transaction(&mut [Operation::Write(&[0x2, reg, msb, lsb])])
                .map_err(|error| {
                    log::warn!("Failed to make SPI transaction for LS write_register: {error:?}");
                    DSPError::Spi
                })?;
        }

        self.await_data_request()?;
        self.control_mode_off()?;
        Ok(())
    }

    fn test_comm(&mut self, header: *const u8) -> bool {
        // Test the communication with the VS1053 module.  The result will be returned.
        // If DREQ is low, there is problably no VS1053 connected. Pull the line HIGH
        // in order to prevent an endless loop waiting for this signal.  The rest of the
        // software will still work, but readbacks from VS1053 will fail.
        {
            let dreq = match PinDriver::input(&mut self.dreq_pin) {
                Ok(pin) => pin,
                Err(err) => {
                    warn!("Get DREQ pin for test_comm failed because: {:?}", err);
                    None
                }
                .expect("DREQ test_comm failed"),
            };
            if !dreq.is_high() {
                log::warn!("VS1053 not properly installed!\n");
                //     pinMode(dreq_pin, INPUT_PULLUP); // DREQ is now input with pull-up
                return false;
            }
        }
        // // Further TESTING.  Check if SCI bus can write and read without errors.
        // // We will use the volume setting for this.
        // // Will give warnings on serial output if DEBUG is active.
        // // A maximum of 20 errors will be reported.

        let (mut r1, mut r2);
        let mut cnt = 0;
        let mut delta: usize = 300; // 3 for fast SPI

        if contains(header, "Fast") {
            delta = 3; // Fast SPI, more loops
        }
        log::info!("header:{:?}", header);

        for i in (0..0xFFFF).step_by(delta) {
            if cnt >= 20 {
                break;
            }
            let _ = self.write_register(true, SCI_VOL, i); // Write data to SCI_VOL
            r1 = self.read_register(SCI_VOL).expect("First SCI_VOL test_comm read"); // Read back for the first time
            r2 = self.read_register(SCI_VOL).expect("Second SCI_VOL test_comm read"); // Read back a second time
            if r1 != r2 || i != r1 || i != r2 {
                // Check for 2 equal reads
                log::info!("VS1053 error retry SB:{:04X} R1:{:04X} R2:{:04X}\n", i, r1, r2);
                cnt += 1;
                sleep(Duration::from_millis(10));
            }
            // yield(); // Allow ESP firmware to do some bookkeeping
        }
        return cnt == 0; // Return the result
    }

    fn set_volume(&mut self, vol: u8) -> Result<(), DSPError> {
        // Set volume.  Both left and right.
        // Input value is 0..100.  100 is the loudest.
        let (mut value_l, mut value_r); // Values to send to SCI_VOL
    
        self.current_volume = vol;                         // Save for later use
        value_l = vol;
        value_r = vol;
    
        if self.current_balance < 0 {
            value_r = max(0, vol.saturating_add(self.current_balance as u8));
        } else if self.current_balance > 0 {
            value_l = max(0, vol.saturating_sub(self.current_balance as u8));
        }
    
        value_l = map(value_l.into(), 0, 100, 0xFE, 0x00) as u8; // 0..100% to left channel
        value_r = map(value_r.into(), 0, 100, 0xFE, 0x00) as u8; // 0..100% to right channel
    
        self.write_register(true, SCI_VOL, ((value_l as u16) << 8) | value_r as u16) // Volume left and right
    }
    
    fn set_balance(&mut self, balance: i8) {
        if balance > 100 {
            self.current_balance = 100;
        } else if balance < -100 {
            self.current_balance = -100;
        } else {
            self.current_balance = balance;
        }
    }
    
    // fn setTone(uint8_t *rtone) { // Set bass/treble (4 nibbles)
    //     // Set tone characteristics.  See documentation for the 4 nibbles.
    //     uint16_t value = 0; // Value to send to SCI_BASS
    //     int i;              // Loop control
    
    //     for (i = 0; i < 4; i++) {
    //         value = (value << 4) | rtone[i]; // Shift next nibble in
    //     }
    //     writeRegister(SCI_BASS, value); // Volume left and right
    // }
    
    fn get_volume(&mut self) -> u8 { // Get the current volume setting.
        return self.current_volume;
    }
    
    fn get_balance(&mut self) -> i8 { // Get the current balance setting.
        return self.current_balance;
    }
    
    // fn startSong() {
    //     sdi_send_fillers(10);
    // }
    
    // fn playChunk(uint8_t *data, size_t len) {
    //     sdi_send_buffer(data, len);
    // }
    
    // fn stopSong() {
    //     uint16_t modereg; // Read from mode register
    //     int i;            // Loop control
    
    //     sdi_send_fillers(2052);
    //     delay(10);
    //     writeRegister(SCI_MODE, _BV(SM_SDINEW) | _BV(SM_CANCEL));
    //     for (i = 0; i < 200; i++) {
    //         sdi_send_fillers(32);
    //         modereg = read_register(SCI_MODE); // Read status
    //         if ((modereg & _BV(SM_CANCEL)) == 0) {
    //             sdi_send_fillers(2052);
    //             LOG("Song stopped correctly after %d msec\n", i * 10);
    //             return;
    //         }
    //         delay(10);
    //     }
    //     printDetails("Song stopped incorrectly!");
    // }
    
    // fn softReset() {
    //     LOG("Performing soft-reset\n");
    //     writeRegister(SCI_MODE, _BV(SM_SDINEW) | _BV(SM_RESET));
    //     delay(10);
    //     await_data_request();
    // }
    
    // /**
    //  * VLSI datasheet: "SM_STREAM activates VS1053b’s stream mode. In this mode, data should be sent with as
    //  * even intervals as possible and preferable in blocks of less than 512 bytes, and VS1053b makes
    //  * every attempt to keep its input buffer half full by changing its playback speed up to 5%. For best
    //  * quality sound, the average speed error should be within 0.5%, the bitrate should not exceed
    //  * 160 kbit/s and VBR should not be used. For details, see Application Notes for VS10XX. This
    //  * mode only works with MP3 and WAV files."
    // */
    
    // fn streamModeOn() {
    //     LOG("Performing streamModeOn\n");
    //     writeRegister(SCI_MODE, _BV(SM_SDINEW) | _BV(SM_STREAM));
    //     delay(10);
    //     await_data_request();
    // }
    
    // fn streamModeOff() {
    //     LOG("Performing streamModeOff\n");
    //     writeRegister(SCI_MODE, _BV(SM_SDINEW));
    //     delay(10);
    //     await_data_request();
    // }
    
    // fn printDetails(const char *header) {
    //     uint16_t regbuf[16];
    //     uint8_t i;
    //     (void)regbuf;
    
    //     LOG("%s", header);
    //     LOG("REG   Contents\n");
    //     LOG("---   -----\n");
    //     for (i = 0; i <= SCI_num_registers; i++) {
    //         regbuf[i] = read_register(i);
    //     }
    //     for (i = 0; i <= SCI_num_registers; i++) {
    //         delay(5);
    //         LOG("%3X - %5X\n", i, regbuf[i]);
    //     }
    // }
    
    // /**
    //  * An optional switch.
    //  * Most VS1053 modules will start up in MIDI mode. The result is that there is no audio when playing MP3.
    //  * You can modify the board, but there is a more elegant way without soldering.
    //  * No side effects for boards which do not need this switch. It means you can call it just in case.
    //  *
    //  * Read more here: http://www.bajdi.com/lcsoft-vs1053-mp3-module/#comment-33773
    //  */
    // fn switchToMp3Mode() {
    //     wram_write(ADDR_REG_GPIO_DDR_RW, 3); // GPIO DDR = 3
    //     wram_write(ADDR_REG_GPIO_ODATA_RW, 0); // GPIO ODATA = 0
    //     delay(100);
    //     LOG("Switched to mp3 mode\n");
    //     softReset();
    // }
    
    // fn disableI2sOut() {
    //     wram_write(ADDR_REG_I2S_CONFIG_RW, 0x0000);
    
    //     // configure GPIO0 4-7 (I2S) as input (default)
    //     // leave other GPIOs unchanged
    //     uint16_t cur_ddr = wram_read(ADDR_REG_GPIO_DDR_RW);
    //     wram_write(ADDR_REG_GPIO_DDR_RW, cur_ddr & ~0x00f0);
    // }
    
    // fn enableI2sOut(VS1053_I2S_RATE i2sRate) {
    //     // configure GPIO0 4-7 (I2S) as output
    //     // leave other GPIOs unchanged
    //     uint16_t cur_ddr = wram_read(ADDR_REG_GPIO_DDR_RW);
    //     wram_write(ADDR_REG_GPIO_DDR_RW, cur_ddr | 0x00f0);
    
    //     uint16_t i2s_config = 0x000c; // Enable MCLK(3); I2S(2)
    //     switch (i2sRate) {
    //         case VS1053_I2S_RATE_192_KHZ:
    //             i2s_config |= 0x0002;
    //             break;
    //         case VS1053_I2S_RATE_96_KHZ:
    //             i2s_config |= 0x0001;
    //             break;
    //         default:
    //         case VS1053_I2S_RATE_48_KHZ:
    //             // 0x0000
    //             break;
    //     }
    
    //     wram_write(ADDR_REG_I2S_CONFIG_RW, i2s_config );
    // }
    
    // /**
    //  * A lightweight method to check if VS1053 is correctly wired up (power supply and connection to SPI interface).
    //  *
    //  * @return true if the chip is wired up correctly
    //  */
    // bool VS1053::isChipConnected() {
    //     uint16_t status = read_register(SCI_STATUS);
    
    //     return !(status == 0 || status == 0xFFFF);
    // }
    
    // /**
    //  * get the Version Number for the VLSI chip
    //  * VLSI datasheet: 0 for VS1001, 1 for VS1011, 2 for VS1002, 3 for VS1003, 4 for VS1053 and VS8053,
    //  * 5 for VS1033, 7 for VS1103, and 6 for VS1063. 
    //  */
    // uint16_t VS1053::getChipVersion() {
    //     uint16_t status = read_register(SCI_STATUS);
           
    //     return ( (status & 0x00F0) >> 4);
    // }
    
    // /**
    //  * Provides current decoded time in full seconds (from SCI_DECODE_TIME register value)
    //  *
    //  * When decoding correct data, current decoded time is shown in SCI_DECODE_TIME
    //  * register in full seconds. The user may change the value of this register.
    //  * In that case the new value should be written twice to make absolutely certain
    //  * that the change is not overwritten by the firmware. A write to SCI_DECODE_TIME
    //  * also resets the byteRate calculation.
    //  *
    //  * SCI_DECODE_TIME is reset at every hardware and software reset. It is no longer
    //  * cleared when decoding of a file ends to allow the decode time to proceed
    //  * automatically with looped files and with seamless playback of multiple files.
    //  * With fast playback (see the playSpeed extra parameter) the decode time also
    //  * counts faster. Some codecs (WMA and Ogg Vorbis) can also indicate the absolute
    //  * play position, see the positionMsec extra parameter in section 10.11.
    //  *
    //  * @see VS1053b Datasheet (1.31) / 9.6.5 SCI_DECODE_TIME (RW)
    //  *
    //  * @return current decoded time in full seconds
    //  */
    // uint16_t VS1053::getDecodedTime() {
    //     return read_register(SCI_DECODE_TIME);
    // }
    
    // /**
    //  * Clears decoded time (sets SCI_DECODE_TIME register to 0x00)
    //  *
    //  * The user may change the value of this register. In that case the new value
    //  * should be written twice to make absolutely certain that the change is not
    //  * overwritten by the firmware. A write to SCI_DECODE_TIME also resets the
    //  * byteRate calculation.
    //  */
    // fn clearDecodedTime() {
    //     writeRegister(SCI_DECODE_TIME, 0x00);
    //     writeRegister(SCI_DECODE_TIME, 0x00);
    // }
    
    // /**
    //  * Fine tune the data rate
    //  */
    // fn adjustRate(long ppm2) {
    //     writeRegister(SCI_WRAMADDR, 0x1e07);
    //     writeRegister(SCI_WRAM, ppm2);
    //     writeRegister(SCI_WRAM, ppm2 >> 16);
    //     // oldClock4KHz = 0 forces  adjustment calculation when rate checked.
    //     writeRegister(SCI_WRAMADDR, 0x5b1c);
    //     writeRegister(SCI_WRAM, 0);
    //     // Write to AUDATA or CLOCKF checks rate and recalculates adjustment.
    //     writeRegister(SCI_AUDATA, read_register(SCI_AUDATA));
    // }
    
    // /**
    //  * Load a patch or plugin
    //  *
    //  * Patches can be found on the VLSI Website http://www.vlsi.fi/en/support/software/vs10xxpatches.html
    //  *  
    //  * Please note that loadUserCode only works for compressed plugins (file ending .plg). 
    //  * To include them, rename them to file ending .h 
    //  * Please also note that, in order to avoid multiple definitions, if you are using more than one patch, 
    //  * it is necessary to rename the name of the array plugin[] and the name of PLUGIN_SIZE to names of your choice.
    //  * example: after renaming plugin[] to plugin_myname[] and PLUGIN_SIZE to PLUGIN_MYNAME_SIZE 
    //  * the method is called by player.loadUserCode(plugin_myname, PLUGIN_MYNAME_SIZE)
    //  * It is also possible to just rename the array plugin[] to a name of your choice
    //  * example: after renaming plugin[] to plugin_myname[]  
    //  * the method is called by player.loadUserCode(plugin_myname, sizeof(plugin_myname)/sizeof(plugin_myname[0]))
    //  */
    // fn loadUserCode(const unsigned short* plugin, unsigned short plugin_size) {
    //     int i = 0;
    //     while (i<plugin_size) {
    //         unsigned short addr, n, val;
    //         addr = plugin[i++];
    //         n = plugin[i++];
    //         if (n & 0x8000U) { /* RLE run, replicate n samples */
    //             n &= 0x7FFF;
    //             val = plugin[i++];
    //             while (n--) {
    //                 writeRegister(addr, val);
    //             }
    //         } else {           /* Copy run, copy n samples */
    //             while (n--) {
    //                 val = plugin[i++];
    //                 writeRegister(addr, val);
    //             }
    //         }
    //     }
    // }
    
    // /**
    //  * Load the latest generic firmware patch
    //  */
    // fn loadDefaultVs1053Patches() {
    //    loadUserCode(PATCHES,PATCHES_SIZE);
    // };
}

#[derive(Copy, Clone, Debug)]
pub enum DSPError {
    Spi,
    UnableToSetCSPin,
    UnableToSetDCSPin,
    UnableToGetDREQPin,
    DataRequestTimeout,
}
