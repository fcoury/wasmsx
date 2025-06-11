# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an MSX computer emulator written in Rust and compiled to WebAssembly for browser execution. The emulator implements the Z80 CPU, TMS9918 VDP (Video Display Processor), keyboard matrix, and other MSX hardware components.

## Development Commands

### Building and Running

```bash
# Build WASM package
cargo make build

# Run development server (watches Rust changes and serves client)
cargo make dev

# Run tests
cargo test

# Run specific test
cargo test test_screen1_color
```

### Client Development

```bash
# Vanilla JS client (in /client)
cd client
yarn install
yarn dev      # Development server
yarn build    # Production build

# React client (in /react)
cd react
yarn install
yarn dev      # Vite dev server
yarn build    # Production build
```

## Architecture

### Core Emulation (`/src/`)
- `machine.rs` - Main emulation entry point, coordinates all components
- `bus.rs` - System bus for memory and I/O operations
- `vdp.rs` - TMS9918 video processor (screen modes, color, patterns)
- `ppi.rs` - 8255 Programmable Peripheral Interface
- `psg.rs` - AY-3-8910 Programmable Sound Generator
- `keyboard.rs` - Keyboard matrix emulation (currently A and B keys work)
- `slot.rs` - Memory slot management (ROM/RAM configuration)
- `renderer.rs` - Canvas rendering for VDP output

### Memory Architecture
- Slot-based system supporting multiple ROM/RAM configurations
- Default: ROM at 0x0000-0x10000, RAM at same range
- Configured via `MachineBuilder` pattern

### VDP Implementation
- Supports Screen 0 (40x24 text) and Screen 1 (32x24 graphics)
- Register-based configuration (see `/docs/vdp.md`)
- Pattern, color, and name tables in VRAM
- Interrupt generation support

### WASM Integration
- Built with `wasm-pack` targeting web
- JavaScript bindings via `wasm-bindgen`
- Canvas-based rendering in browser
- Keyboard input handling through web events

## Testing

Tests are in `/tests/machine_tests.rs` with fixtures in `/tests/fixtures/`. The test ROM is `/roms/hotbit.rom`.

Key test areas:
- VDP display modes and color rendering
- Machine state after specific operations
- Memory slot configuration
- Screen rendering output

## Current Development State

- CPU: Z80 emulation functional
- Video: Screen 0 and 1 modes working, vertical bar rendering fixed
- Keyboard: Matrix implementation complete, A and B keys functional
- Sound: PSG structure in place, not fully implemented
- Interrupts: VDP interrupt generation working

## Workflow Notes

- You don't need to run the server or the client as I am running them on the background, so feel free to ask me to re-run the test and report the logs every time you need it