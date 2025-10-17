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
use embassy_stm32::gpio::Pin;
use embassy_stm32::pac::fsmc::vals::{Accmod, Cpsize, Mtyp, Waitcfg, Waitpol};
use embassy_stm32::pac::fsmc::vals::Mwid;

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
pub struct FsmcLcd<
    CS,
    RD,
    RW,
    RS,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    D10,
    D11,
    D12,
    D13,
    D14,
    D15,
> {
    cs: CS,
    rd: RD,
    rw: RW,
    rs: RS,
    data_pins: (
        D0, D1, D2, D3, D4, D5, D6, D7, D8, D9, D10, D11, D12, D13, D14, D15,
    ),
}

impl<
        CS: Pin,
        RD: Pin,
        RW: Pin,
        RS: Pin,
        D0: Pin,
        D1: Pin,
        D2: Pin,
        D3: Pin,
        D4: Pin,
        D5: Pin,
        D6: Pin,
        D7: Pin,
        D8: Pin,
        D9: Pin,
        D10: Pin,
        D11: Pin,
        D12: Pin,
        D13: Pin,
        D14: Pin,
        D15: Pin,
    >
    FsmcLcd<CS, RD, RW, RS, D0, D1, D2, D3, D4, D5, D6, D7, D8, D9, D10, D11, D12, D13, D14, D15>
{
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
        cs: CS,
        rd: RD,
        rw: RW,
        rs: RS,
        data_pins: (
            D0, D1, D2, D3, D4, D5, D6, D7, D8, D9, D10, D11, D12, D13, D14, D15,
        ),
        read_timing: &Timing,
        write_timing: &Timing,
    ) -> Self {
        use embassy_stm32::rcc::low_level::RccPeripheral as _;

        // Enable FSMC peripheral clock
        embassy_stm32::peripherals::FSMC::enable_and_reset();

        let fsmc = unsafe { embassy_stm32::pac::fsmc::Fsmc::from_ptr(REG_ADDRESS as _) };

        // Configure FSMC Bank Control Register
        fsmc.bcr(0).write(|w| {
            // Disable synchronous writes
            w.set_cburstrw(false);
            // Don't split burst transactions (doesn't matter for LCD mode)
            w.set_cpsize(Cpsize::NOBURSTSPLIT);
            // Ignore wait signal (asynchronous mode)
            w.set_asyncwait(false);
            // Enable extended mode, for different read and write timings
            w.set_extmod(true);
            // Ignore wait signal (synchronous mode)
            w.set_waiten(false);
            // Allow write operations
            w.set_wren(true);
            // Default wait timing
            w.set_waitcfg(Waitcfg::BEFOREWAITSTATE);
            // Default wait polarity
            w.set_waitpol(Waitpol::ACTIVELOW);
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
        cs.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        rd.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        rw.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        rs.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);

        data_pins.0.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.1.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.2.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.3.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.4.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.5.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.6.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.7.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.8.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.9.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.10.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.11.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.12.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.13.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.14.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);
        data_pins.15.set_as_af(12, embassy_stm32::gpio::low_level::AFType::OutputPushPull);

        Self {
            cs,
            rd,
            rw,
            rs,
            data_pins,
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

    /// Releases the FSMC peripheral and returns the pins
    ///
    /// This disables the FSMC peripheral and returns ownership of all pins.
    pub fn release(
        self,
    ) -> (
        CS,
        RD,
        RW,
        RS,
        (D0, D1, D2, D3, D4, D5, D6, D7, D8, D9, D10, D11, D12, D13, D14, D15),
    ) {
        use embassy_stm32::rcc::low_level::RccPeripheral as _;
        embassy_stm32::peripherals::FSMC::disable();

        (self.cs, self.rd, self.rw, self.rs, self.data_pins)
    }
}

// Implement DisplayInterface WriteOnlyDataCommand trait
impl<
        CS: Pin,
        RD: Pin,
        RW: Pin,
        RS: Pin,
        D0: Pin,
        D1: Pin,
        D2: Pin,
        D3: Pin,
        D4: Pin,
        D5: Pin,
        D6: Pin,
        D7: Pin,
        D8: Pin,
        D9: Pin,
        D10: Pin,
        D11: Pin,
        D12: Pin,
        D13: Pin,
        D14: Pin,
        D15: Pin,
    > WriteOnlyDataCommand
    for FsmcLcd<
        CS,
        RD,
        RW,
        RS,
        D0,
        D1,
        D2,
        D3,
        D4,
        D5,
        D6,
        D7,
        D8,
        D9,
        D10,
        D11,
        D12,
        D13,
        D14,
        D15,
    >
{
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