#![no_std]

//! DisplayInterface implementation for STM32 FSMC peripheral using Embassy HAL
//!
//! This crate provides a DisplayInterface implementation that allows using the STM32
//! Flexible Memory Controller (FSMC) to communicate with LCD displays.

//!
//! # Example
//!
//! ```no_run
//! use embassy_stm32_fsmc_display_interface::{FsmcLcd, Timing};
//! # use embassy_stm32::gpio::Output;
//! # let pins = todo!();
//!
//! let lcd_interface = FsmcLcd::new(
//!     pins.PD7,  // CS
//!     pins.PD4,  // RD
//!     pins.PD5,  // WR
//!     pins.PD13, // RS (D/C)
//!     (pins.PD14, pins.PD15, pins.PD0, pins.PD1,
//!      pins.PE7, pins.PE8, pins.PE9, pins.PE10,
//!      pins.PE11, pins.PE12, pins.PE13, pins.PE14,
//!      pins.PE15, pins.PD8, pins.PD9, pins.PD10), // D0-D15
//!     &Timing::default(),
//!     &Timing::default(),
//! );
//!
//! // Now use lcd_interface with your display driver
//! // let display = ili9341::Ili9341::new(lcd_interface, reset_pin, ...);
//! ```

use display_interface::{DataFormat, DisplayError, WriteOnlyDataCommand};
use embassy_stm32::gpio::{AfType, Flex, Pin, Speed, Pull, OutputType};
use embassy_stm32::pac::fsmc::vals::{Accmod, Cpsize, Mtyp, Waitcfg, Waitpol};
use embassy_stm32::pac::fsmc::vals::Mwid;
use embassy_stm32::rcc;
use embassy_stm32::Peri;

/// STM32F407 Reference manual, 36.5.6
/// Register base address for FSMC
const REG_ADDRESS: usize = 0xA000_0000;

/// The base address of the first FSMC bank
const BASE_ADDRESS: usize = 0x6000_0000;

/// Address used to send commands to the display
const COMMAND_ADDRESS: usize = BASE_ADDRESS;

/// Address used to send data to the display
const DATA_ADDRESS: usize = make_data_address(BASE_ADDRESS);

/// Converts a command address into a data address
///
/// The data address will result in all external address signals being set high.
/// This allows the display to differentiate between command and data based on
/// address line state (typically A18/RS pin).
const fn make_data_address(base: usize) -> usize {
    // Bits 26 and 27 select the sub-bank, don't change them.
    // Bits 25 through 1 become address signals 24 through 0, set these high.
    // Bit 0 is not used with 16-bit addressing.
    base | 0x3fffffe
}

/// FSMC timing configuration
///
/// Controls the timing parameters for FSMC bus operations. These values
/// determine how fast the FSMC can communicate with the display.
pub struct Timing {
    /// Access mode for the memory bank
    pub access_mode: Accmod,
    /// Bus turnaround time in HCLK cycles (0-15)
    pub bus_turnaround: u8,
    /// Data phase duration in HCLK cycles (1-255)
    pub data: u8,
    /// Address hold phase duration in HCLK cycles (1-15)
    pub address_hold: u8,
    /// Address setup phase duration in HCLK cycles (0-15)
    pub address_setup: u8,
}

impl Timing {
    /// Maximum allowed value of the bus turnaround time
    pub const BUS_TURNAROUND_MAX: u8 = 15;

    /// Minimum allowed value of the data phase time
    pub const DATA_MIN: u8 = 1;

    /// Minimum allowed value of the address hold time
    pub const ADDRESS_HOLD_MIN: u8 = 1;

    /// Maximum allowed value of the address hold time
    pub const ADDRESS_HOLD_MAX: u8 = 15;

    /// Maximum allowed value of the address setup time
    pub const ADDRESS_SETUP_MAX: u8 = 15;

    /// Creates a new timing configuration with conservative (slow) values
    ///
    /// These values should work with most displays but may not be optimal.
    /// Adjust based on your display's datasheet for better performance.
    pub const fn new(
        access_mode: Accmod,
        bus_turnaround: u8,
        data: u8,
        address_hold: u8,
        address_setup: u8,
    ) -> Self {
        Self {
            access_mode,
            bus_turnaround,
            data,
            address_hold,
            address_setup,
        }
    }
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            access_mode: Accmod::C,
            bus_turnaround: Self::BUS_TURNAROUND_MAX,
            data: 255,
            address_hold: Self::ADDRESS_HOLD_MAX,
            address_setup: Self::ADDRESS_SETUP_MAX,
        }
    }
}

/// FSMC LCD interface for parallel displays
///
/// This struct provides a DisplayInterface implementation using the STM32 FSMC
/// peripheral. It supports 16-bit parallel communication with LCD controllers.
///
/// # Type Parameters
///
/// The type parameters represent the GPIO pins used for various FSMC signals:
/// - `CS`: Chip Select
/// - `RD`: Read Enable
/// - `RW`: Write Enable
/// - `RS`: Register Select (Data/Command, sometimes called D/C)
/// - `D0`-`D15`: 16-bit data bus
pub struct FsmcLcd<'d> {
    _cs: Flex<'d>,
    _rd: Flex<'d>,
    _rw: Flex<'d>,
    _rs: Flex<'d>,
    _data_pins: (
        Flex<'d>, Flex<'d>, Flex<'d>, Flex<'d>,
        Flex<'d>, Flex<'d>, Flex<'d>, Flex<'d>,
        Flex<'d>, Flex<'d>, Flex<'d>, Flex<'d>,
        Flex<'d>, Flex<'d>, Flex<'d>, Flex<'d>,
    ),
}

impl<'d> FsmcLcd<'d> {
    /// Creates a new FSMC LCD interface
    ///
    /// # Arguments
    ///
    /// * `cs` - Chip Select pin
    /// * `rd` - Read Enable pin
    /// * `rw` - Write Enable pin
    /// * `rs` - Register Select pin (Data/Command)
    /// * `data_pins` - Tuple of 16 data pins (D0-D15)
    /// * `read_timing` - Timing configuration for read operations
    /// * `write_timing` - Timing configuration for write operations
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use embassy_stm32_fsmc_display_interface::{FsmcLcd, Timing};
    /// # let pins = todo!();
    /// let lcd = FsmcLcd::new(
    ///     pins.PD7,  // CS
    ///     pins.PD4,  // RD
    ///     pins.PD5,  // WR
    ///     pins.PD13, // RS
    ///     (pins.PD14, pins.PD15, pins.PD0, pins.PD1,
    ///      pins.PE7, pins.PE8, pins.PE9, pins.PE10,
    ///      pins.PE11, pins.PE12, pins.PE13, pins.PE14,
    ///      pins.PE15, pins.PD8, pins.PD9, pins.PD10),
    ///     &Timing::default(),
    ///     &Timing::default(),
    /// );
    /// ```
    pub fn new(
        cs: Peri<'d, impl Pin>,
        rd: Peri<'d, impl Pin>,
        rw: Peri<'d, impl Pin>,
        rs: Peri<'d, impl Pin>,
        data_pins: (
            Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>,
            Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>,
            Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>,
            Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>, Peri<'d, impl Pin>,
        ),
        read_timing: &Timing,
        write_timing: &Timing,
    ) -> Self {
        // Enable FSMC peripheral clock
        rcc::enable_and_reset::<embassy_stm32::peripherals::FSMC>();

        let fsmc = unsafe { embassy_stm32::pac::fsmc::Fsmc::from_ptr(REG_ADDRESS as _) };

        // Configure FSMC Bank Control Register
        fsmc.bcr(0).write(|w| {
            // Disable synchronous writes
            w.set_cburstrw(false);
            // Don't split burst transactions (doesn't matter for LCD mode)
            w.set_cpsize(Cpsize::NO_BURST_SPLIT);
            // Ignore wait signal (asynchronous mode)
            w.set_asyncwait(false);
            // Enable extended mode, for different read and write timings
            w.set_extmod(true);
            // Ignore wait signal (synchronous mode)
            w.set_waiten(false);
            // Allow write operations
            w.set_wren(true);
            // Default wait timing
            w.set_waitcfg(Waitcfg::BEFORE_WAIT_STATE);
            // Default wait polarity
            w.set_waitpol(Waitpol::ACTIVE_LOW);
            // Disable burst reads
            w.set_bursten(false);
            // Enable NOR flash operations
            w.set_faccen(true);
            // 16-bit bus width
            w.set_mwid(Mwid::BITS16);
            // NOR flash mode (compatible with LCD controllers)
            w.set_mtyp(Mtyp::FLASH);
            // Address and data not multiplexed
            w.set_muxen(false);
            // Enable this memory bank
            w.set_mbken(true);
        });

        // Configure read timing
        fsmc.btr(0).write(|w| {
            w.set_accmod(read_timing.access_mode);
            w.set_busturn(read_timing.bus_turnaround);
            w.set_datast(read_timing.data);
            w.set_addhld(read_timing.address_hold);
            w.set_addset(read_timing.address_setup);
        });

        // Configure write timing
        fsmc.bwtr(0).write(|w| {
            w.set_accmod(write_timing.access_mode);
            w.set_busturn(write_timing.bus_turnaround);
            w.set_datast(write_timing.data);
            w.set_addhld(write_timing.address_hold);
            w.set_addset(write_timing.address_setup);
        });

        // Configure all pins as FSMC alternate function (AF12)
        let af_type = AfType::output_pull(OutputType::PushPull, Speed::VeryHigh, Pull::None);

        let mut cs_flex = Flex::new(cs);
        cs_flex.set_as_af_unchecked(12, af_type);
        let mut rd_flex = Flex::new(rd);
        rd_flex.set_as_af_unchecked(12, af_type);
        let mut rw_flex = Flex::new(rw);
        rw_flex.set_as_af_unchecked(12, af_type);
        let mut rs_flex = Flex::new(rs);
        rs_flex.set_as_af_unchecked(12, af_type);

        let mut d0_flex = Flex::new(data_pins.0);
        d0_flex.set_as_af_unchecked(12, af_type);
        let mut d1_flex = Flex::new(data_pins.1);
        d1_flex.set_as_af_unchecked(12, af_type);
        let mut d2_flex = Flex::new(data_pins.2);
        d2_flex.set_as_af_unchecked(12, af_type);
        let mut d3_flex = Flex::new(data_pins.3);
        d3_flex.set_as_af_unchecked(12, af_type);
        let mut d4_flex = Flex::new(data_pins.4);
        d4_flex.set_as_af_unchecked(12, af_type);
        let mut d5_flex = Flex::new(data_pins.5);
        d5_flex.set_as_af_unchecked(12, af_type);
        let mut d6_flex = Flex::new(data_pins.6);
        d6_flex.set_as_af_unchecked(12, af_type);
        let mut d7_flex = Flex::new(data_pins.7);
        d7_flex.set_as_af_unchecked(12, af_type);
        let mut d8_flex = Flex::new(data_pins.8);
        d8_flex.set_as_af_unchecked(12, af_type);
        let mut d9_flex = Flex::new(data_pins.9);
        d9_flex.set_as_af_unchecked(12, af_type);
        let mut d10_flex = Flex::new(data_pins.10);
        d10_flex.set_as_af_unchecked(12, af_type);
        let mut d11_flex = Flex::new(data_pins.11);
        d11_flex.set_as_af_unchecked(12, af_type);
        let mut d12_flex = Flex::new(data_pins.12);
        d12_flex.set_as_af_unchecked(12, af_type);
        let mut d13_flex = Flex::new(data_pins.13);
        d13_flex.set_as_af_unchecked(12, af_type);
        let mut d14_flex = Flex::new(data_pins.14);
        d14_flex.set_as_af_unchecked(12, af_type);
        let mut d15_flex = Flex::new(data_pins.15);
        d15_flex.set_as_af_unchecked(12, af_type);

        Self {
            _cs: cs_flex,
            _rd: rd_flex,
            _rw: rw_flex,
            _rs: rs_flex,
            _data_pins: (
                d0_flex, d1_flex, d2_flex, d3_flex,
                d4_flex, d5_flex, d6_flex, d7_flex,
                d8_flex, d9_flex, d10_flex, d11_flex,
                d12_flex, d13_flex, d14_flex, d15_flex,
            ),
        }
    }

    /// Writes a command value to the display
    ///
    /// This performs a write to the command address, which will set the
    /// register select (RS) line appropriately.
    #[inline]
    pub fn write_command(&self, value: u16) {
        unsafe {
            core::ptr::write_volatile(COMMAND_ADDRESS as *mut u16, value);
        }
    }

    /// Writes a data value to the display
    ///
    /// This performs a write to the data address, which will set the
    /// register select (RS) line appropriately.
    #[inline]
    pub fn write_data(&self, value: u16) {
        unsafe {
            core::ptr::write_volatile(DATA_ADDRESS as *mut u16, value);
        }
    }
}

// Implement DisplayInterface WriteOnlyDataCommand trait
impl<'d> WriteOnlyDataCommand for FsmcLcd<'d> {
    fn send_commands(&mut self, cmd: DataFormat<'_>) -> Result<(), DisplayError> {
        match cmd {
            DataFormat::U8(items) => {
                for item in items {
                    self.write_command(u16::from(*item));
                }
            }
            DataFormat::U16(items) => {
                for item in items {
                    self.write_command(*item);
                }
            }
            DataFormat::U16BE(items) | DataFormat::U16LE(items) => {
                for item in items {
                    self.write_command(*item);
                }
            }
            DataFormat::U8Iter(iterator) => {
                for item in iterator {
                    self.write_command(u16::from(item));
                }
            }
            DataFormat::U16BEIter(iterator) | DataFormat::U16LEIter(iterator) => {
                for item in iterator {
                    self.write_command(item);
                }
            }
            _ => return Err(DisplayError::DataFormatNotImplemented),
        }
        Ok(())
    }

    fn send_data(&mut self, buf: DataFormat<'_>) -> Result<(), DisplayError> {
        match buf {
            DataFormat::U8(items) => {
                for item in items {
                    self.write_data(u16::from(*item));
                }
            }
            DataFormat::U16(items) => {
                for item in items {
                    self.write_data(*item);
                }
            }
            DataFormat::U16BE(items) | DataFormat::U16LE(items) => {
                for item in items {
                    self.write_data(*item);
                }
            }
            DataFormat::U8Iter(iterator) => {
                for item in iterator {
                    self.write_data(u16::from(item));
                }
            }
            DataFormat::U16BEIter(iterator) | DataFormat::U16LEIter(iterator) => {
                for item in iterator {
                    self.write_data(item);
                }
            }
            _ => return Err(DisplayError::DataFormatNotImplemented),
        }
        Ok(())
    }
}