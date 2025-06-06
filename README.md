# WasmSX

A WebAssembly-powered MSX computer emulator written in Rust that runs in the browser.

## Features

- **TMS9918 VDP** - Video Display Processor
- **AY-3-8910 PSG** - Programmable Sound Generator for authentic MSX audio
- **Keyboard matrix** - Full keyboard input support
- **Joystick emulation** - Game controller support via PSG
- **Cycle-accurate timing** - Precise emulation timing
- **Multiple ROM support** - Load different MSX ROMs and disk ROMs

## Quick Start

### Prerequisites

- Rust and Cargo
- Node.js and Yarn
- `cargo-make` (`cargo install cargo-make`)
- `wasm-pack` (`cargo install wasm-pack`)

### Building and Running

```bash
# Clone the repository
git clone https://github.com/fcoury/wasmsx.git
cd wasmsx

# Build the WASM package
cargo make build

# Run the development server (watches for changes)
cargo make dev
```

Open http://localhost:3000 in your browser to use the emulator.

## Architecture

The emulator is structured as:

- **Core emulation** (`/src/`) - Rust implementation of MSX hardware

  - `machine.rs` - Main emulation coordinator
  - `bus.rs` - System bus for memory and I/O
  - `vdp.rs` - TMS9918 video processor
  - `psg.rs` - AY-3-8910 sound generator
  - `fdc.rs` - WD2793 floppy disk controller
  - `keyboard.rs` - Keyboard matrix emulation
  - `ppi.rs` - 8255 Programmable Peripheral Interface

- **Web interface** (`/client/`) - JavaScript/TypeScript frontend

  - Canvas-based display rendering
  - Web Audio API for sound
  - Keyboard and mouse input handling
  - Disk image loading support

- **React demo** (`/react/`) - Alternative React-based frontend

## Usage

### Loading ROMs

The emulator comes with C-BIOS (an open-source MSX BIOS) included. You can also load your own ROM files through the web interface.

### Keyboard

The keyboard matrix emulation maps your physical keyboard to the MSX keyboard layout. Special keys:

- F11: Toggle fullscreen
- F12: Reset the machine

### Disk Support

Click the disk icon in the UI to load `.dsk` disk image files. The emulator supports standard MSX disk formats.

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_screen1_color
```

### Building for Production

```bash
# Build optimized WASM
cargo make build --release

# Build client
cd client && yarn build
```

## Technical Details

- **Memory:** Slot-based system supporting multiple ROM/RAM configurations
- **Video:** TMS9918 implementation with pattern/color/name tables
- **Audio:** Cycle-accurate PSG emulation with proper resampling
- **Timing:** Precise Z80 cycle counting for accurate emulation

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Z80 CPU emulation based on the z80 crate
- C-BIOS team for the open-source MSX BIOS
- MSX community for documentation and support

