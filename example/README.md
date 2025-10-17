# ILI9341 Display Example

This example demonstrates how to use the `embassy-stm32-fsmc-display-interface` crate with an ILI9341 LCD display controller on an STM32F407VET6 development board. It continuously fills the screen with red, green, and blue colors to demonstrate the speed of the FSMC interface.

## Hardware Requirements

- STM32F407VET6 development board (or similar)
- ILI9341 TFT LCD display with parallel interface
- ST-Link or compatible debugger

## Pin Connections

This example is configured for STM32F407VET6 boards with an integrated TFT LCD connector. Adjust pin mappings in `src/main.rs` if your hardware differs.

| Function | STM32 Pin | FSMC Signal | Description           |
|----------|-----------|-------------|-----------------------|
| CS       | PD7       | FSMC_NE1    | Chip Select           |
| RD       | PD4       | FSMC_NOE    | Output Enable (Read)  |
| WR       | PD5       | FSMC_NWE    | Write Enable          |
| RS/DC    | PD13      | FSMC_A18    | Register Select       |
| RST      | PD12      | GPIO        | Reset                 |
| BL       | PB1       | GPIO        | Backlight Control     |
| D0-D15   | Various   | FSMC_D0-D15 | 16-bit data bus       |

See the full pin mapping table in `src/main.rs`.

## Building

From this directory, run:

```bash
cargo build --release
```

## Running

Make sure your development board is connected via ST-Link, then run:

```bash
cargo run --release
```

The example will:
1. Initialize the FSMC peripheral
2. Initialize the ILI9341 display
3. Continuously cycle through colors (red, green, blue)
4. Fill the entire screen with each color
5. Log the time taken to fill the screen (demonstrating interface speed)
6. Wait 1 second between color changes

You should see the screen cycling through solid colors, and in the debug output you'll see timing information showing how fast the FSMC interface can transfer data (typically several MB/s for a 320x240 display at 16 bits per pixel = ~153KB per screen fill).

## Customizing

### For Different STM32 Chips

Edit `Cargo.toml` and change the `embassy-stm32` feature:

```toml
embassy-stm32 = { workspace = true, features = ["defmt", "stm32f429zi", "unstable-pac", "memory-x", "time-driver-any", "exti"] }
```

Also update the library dependency feature:

```toml
embassy-stm32-fsmc-display-interface = { path = "../lib", features = ["stm32f429zi"] }
```

### For Different Debuggers

Edit `.cargo/config.toml` and change the runner:

```toml
runner = "probe-rs run --chip STM32F407VETx"
```

### Timing Adjustment

If your display has timing requirements, you can adjust the FSMC timing parameters:

```rust
let read_timing = Timing {
    address_setup: 15,
    data_setup: 15,
    bus_turnaround: 0,
};

let write_timing = Timing {
    address_setup: 10,
    data_setup: 10,
    bus_turnaround: 0,
};

let lcd_interface = FsmcLcd::new(
    // ... pins ...
    &read_timing,
    &write_timing,
);
```

## Troubleshooting

### Display not working

1. Verify all pin connections match your hardware
2. Check power supply (3.3V or 5V depending on display)
3. Try increasing timing values in `Timing` struct
4. Verify backlight is enabled

### Build errors

1. Make sure you have the correct Rust toolchain installed (see `rust-toolchain.toml`)
2. Verify all dependencies are available
3. Check that the correct STM32 chip feature is enabled

### Flash/Run errors

1. Verify ST-Link connection
2. Check that the correct chip is specified in `.cargo/config.toml`
3. Try `probe-rs list` to see available chips