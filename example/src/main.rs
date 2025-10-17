//! ILI9341 Display Example using FSMC Interface
//!
//! This example demonstrates how to use the embassy-stm32-fsmc-display-interface
//! crate with an ILI9341 LCD display controller. It continuously fills the screen
//! with red, green, and blue colors to demonstrate the FSMC interface speed.
//!
//! # Hardware Setup
//!
//! This example is configured for STM32F407VET6 development boards with
//! an integrated TFT LCD connector. Adjust pin mappings for your board.
//!
//! ## Pin Connections
//!
//! | Function | STM32 Pin | FSMC Signal | Description           |
//! |----------|-----------|-------------|-----------------------|
//! | CS       | PD7       | FSMC_NE1    | Chip Select           |
//! | RD       | PD4       | FSMC_NOE    | Output Enable (Read)  |
//! | WR       | PD5       | FSMC_NWE    | Write Enable          |
//! | RS/DC    | PD13      | FSMC_A18    | Register Select       |
//! | RST      | PD12      | GPIO        | Reset                 |
//! | BL       | PB1       | GPIO        | Backlight Control     |
//! | D0       | PD14      | FSMC_D0     | Data bit 0            |
//! | D1       | PD15      | FSMC_D1     | Data bit 1            |
//! | D2       | PD0       | FSMC_D2     | Data bit 2            |
//! | D3       | PD1       | FSMC_D3     | Data bit 3            |
//! | D4       | PE7       | FSMC_D4     | Data bit 4            |
//! | D5       | PE8       | FSMC_D5     | Data bit 5            |
//! | D6       | PE9       | FSMC_D6     | Data bit 6            |
//! | D7       | PE10      | FSMC_D7     | Data bit 7            |
//! | D8       | PE11      | FSMC_D8     | Data bit 8            |
//! | D9       | PE12      | FSMC_D9     | Data bit 9            |
//! | D10      | PE13      | FSMC_D10    | Data bit 10           |
//! | D11      | PE14      | FSMC_D11    | Data bit 11           |
//! | D12      | PE15      | FSMC_D12    | Data bit 12           |
//! | D13      | PD8       | FSMC_D13    | Data bit 13           |
//! | D14      | PD9       | FSMC_D14    | Data bit 14           |
//! | D15      | PD10      | FSMC_D15    | Data bit 15           |

#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::{
    gpio::{Level, Output, Speed}, rcc::{AHBPrescaler, APBPrescaler, Pll, PllMul, PllPDiv, PllPreDiv, PllSource, Sysclk}, Config
};
use embassy_time::{Delay, Instant, Timer};
use embassy_stm32_fsmc_display_interface::{FsmcLcd, Timing};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
};
use ili9341::{DisplaySize240x320, Ili9341, Orientation};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Configure memory bus (AHB) to work on maximum speed (168 Mhz)
    // in order to speed up LCD communications.
    let mut config = Config::default();
    // Use internal oscillator (16 Mhz)
    config.rcc.hse = None;
    config.rcc.hsi = true;
    // Use HSI as input to PLL
    config.rcc.pll_src = PllSource::HSI;
    config.rcc.pll = Some(Pll {
        // Divide by 8, get 2 Mhz
        prediv: PllPreDiv::DIV8,
        // Then multiply by 168, get 336 Mhz
        mul: PllMul::MUL168,
        // Then divide by 2, get 168 Mhz
        divp: Some(PllPDiv::DIV2),
        divq: None,
        divr: None,
    });
    // Use PLL as system clock source
    config.rcc.sys = Sysclk::PLL1_P;
    // Don't modify bus frequency, leave 168 Mhz
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    let p = embassy_stm32::init(config);
    info!("STM32 initialized!");

    let mut timing = Timing::default();
    timing.bus_turnaround = 1;
    timing.data = 4;
    timing.address_hold = 0;
    timing.address_setup = 0;

    // Initialize FSMC LCD interface
    // The FSMC peripheral provides a parallel interface that works like external memory.
    // Commands are sent to COMMAND_ADDRESS and data to DATA_ADDRESS, which differ by
    // address line A18 (connected to RS/DC pin).
    let lcd_interface = FsmcLcd::new(
        p.PD7,  // CS  - Chip Select (FSMC_NE1)
        p.PD4,  // RD  - Read Enable (FSMC_NOE)
        p.PD5,  // WR  - Write Enable (FSMC_NWE)
        p.PD13, // RS  - Register Select / Data-Command (FSMC_A18)
        (
            p.PD14, p.PD15, p.PD0, p.PD1,   // D0-D3
            p.PE7, p.PE8, p.PE9, p.PE10,    // D4-D7
            p.PE11, p.PE12, p.PE13, p.PE14, // D8-D11
            p.PE15, p.PD8, p.PD9, p.PD10,   // D12-D15
        ),
        &timing, // Read timing
        &timing, // Write timing
    );

    // Configure reset pin for the display
    // The ILI9341 driver will handle the reset sequence
    let reset_pin = Output::new(p.PD12, Level::Low, Speed::Low);

    // Turn on display backlight
    // Some displays may require PWM for brightness control
    let _backlight = Output::new(p.PB1, Level::High, Speed::Low);
    info!("Backlight enabled");

    // Initialize ILI9341 display driver
    // This performs the initialization sequence including reset
    let mut display = Ili9341::new(
        lcd_interface,
        reset_pin,
        &mut Delay,
        Orientation::Landscape, // 320x240 orientation
        DisplaySize240x320,
    )
    .unwrap();
    info!("Display initialized!");

    info!("Starting color cycling demo to demonstrate FSMC interface speed...");

    // Define colors to cycle through
    let colors = [
        ("RED", Rgb565::RED),
        ("GREEN", Rgb565::GREEN),
        ("BLUE", Rgb565::BLUE),
    ];

    // Continuously cycle through colors, filling the entire screen
    // This demonstrates the speed of the FSMC interface
    loop {
        for (color_name, color) in colors.iter() {
            // Measure how long it takes to fill the screen
            let start = Instant::now();

            // Fill the entire screen with the current color
            display.clear(*color).unwrap();

            let duration = start.elapsed();

            // Log the color and time taken
            // 320x240 = 76,800 pixels at 16 bits per pixel = 153,600 bytes
            info!(
                "Filled screen with {} in {} ms ({} pixels, ~{} KB/s)",
                color_name,
                duration.as_millis(),
                320 * 240,
                if duration.as_millis() > 0 {
                    (153600 * 1000) / (duration.as_millis() as u64 * 1024)
                } else {
                    0
                }
            );

            // Wait 1 second before changing to the next color
            Timer::after_secs(1).await;
        }
    }
}