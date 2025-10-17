# embassy-stm32-fsmc-display-interface

A `display-interface` implementation for STM32 FSMC peripheral using the Embassy async HAL.

This crate provides a way to use the STM32 Flexible Static Memory Controller (FSMC) to drive parallel LCD displays supporting Intel 8080 protocol. The display is interfaced as if it were external memory, allowing very fast data transfers.

## Supported Chips

Tested on STM32F407VET6. Other STM32F4 chips with FSMC peripheral (such as STM32F407, STM32F417) should also work.

## Performance

Using FSMC gives better performance than SPI or Parallel GPIO interfaces. To increase performance, raise AHB bus frequency to maximum.

For most displays, the default timing configuration works well, but may not be optimal for performance. Consult your display's datasheet for recommended timing values.

On my setup it takes 3ms to execute "fill screen" command.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
embassy-stm32-fsmc-display-interface = { version = "0.1", features = ["stm32f407ve"] }
embassy-stm32 = { version = "0.1", features = ["stm32f407ve"] }
display-interface = "0.5"
```

Make sure to enable the feature matching your STM32 chip (currently only `stm32f407ve` is supported).

Create `FsmcLcd`:

```rust
use embassy_stm32_fsmc_display_interface::{FsmcLcd, Timing};
use ili9341::{Ili9341, DisplaySize240x320, Orientation};

// Initialize the FSMC interface
let lcd_interface = FsmcLcd::new(
    p.PD7,  // CS  (Chip Select)
    p.PD4,  // RD  (Read Enable)
    p.PD5,  // WR  (Write Enable)
    p.PD13, // RS  (Register Select / Data-Command)
    (
        p.PD14, p.PD15, p.PD0, p.PD1,   // D0-D3
        p.PE7, p.PE8, p.PE9, p.PE10,    // D4-D7
        p.PE11, p.PE12, p.PE13, p.PE14, // D8-D11
        p.PE15, p.PD8, p.PD9, p.PD10,   // D12-D15
    ),
    &Timing::default(), // Read timing
    &Timing::default(), // Write timing
);

// Use with any display driver that supports display-interface
let mut display = Ili9341::new(
    lcd_interface,
    reset_pin,
    &mut Delay,
    Orientation::Landscape,
    DisplaySize240x320,
).unwrap();

// Now you can draw to the display!
display.clear(Rgb565::BLACK).unwrap();
```

## Example

See the [`example/`](example/) directory for a complete working example using an ILI9341 display with embedded-graphics.

To run the example, you'll need [probe-rs](https://probe.rs/) installed:

```bash
cargo install probe-rs --features=cli
```

Then navigate to the example directory and run:

```bash
cd example
cargo run --release
```
