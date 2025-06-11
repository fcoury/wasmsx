# MSX Disk Emulation Implementation

This document details the implementation of disk emulation in the MSX emulator, including the architecture decisions, technical approach, and integration with the existing codebase.

## Overview

The disk emulation system was implemented to support MSX-DOS disk operations without requiring hardware-level FDC (Floppy Disk Controller) emulation. The approach uses CPU extensions to intercept and handle disk BIOS calls directly.

## Architecture

### Core Design Principles

1. **Minimal Core Modification**: The Z80 CPU core was modified minimally to support extension handlers
2. **BIOS-Level Emulation**: Disk operations are handled at the BIOS call level rather than hardware level
3. **Automatic Detection**: The system automatically detects disk ROMs and initializes the disk subsystem
4. **Thread-Safe Design**: All disk operations are thread-safe for WASM compatibility

### Key Components

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   Z80 CPU   │────▶│ CPU Extension│────▶│ Disk Driver │
│  (ED E0-EF) │     │   Handler    │     │  (BIOS Impl)│
└─────────────┘     └──────────────┘     └─────────────┘
                            │                     │
                            ▼                     ▼
                    ┌──────────────┐     ┌─────────────┐
                    │ Extension    │     │ Disk Drive  │
                    │   State      │     │  Manager    │
                    └──────────────┘     └─────────────┘
                                                 │
                                         ┌───────┴───────┐
                                         ▼               ▼
                                    ┌─────────┐    ┌─────────┐
                                    │ Drive A │    │ Drive B │
                                    │  (DSK)  │    │  (DSK)  │
                                    └─────────┘    └─────────┘
```

## Implementation Details

### 1. Z80 CPU Extension Support

The Z80 core was extended to support CPU extensions through unused ED opcodes (E0-EF):

```rust
// In z80/z80.rs
224..=239 => {
    // ED E0-EF: CPU extensions for MSX disk BIOS and other uses
    if let Some(extra_cycles) = (*z).io.handle_extension(opcode, &mut *z) {
        cyc = cyc.wrapping_add(extra_cycles);
    }
}
```

The `Z80_io` trait was extended with:
```rust
fn handle_extension(&mut self, ext_num: u8, z80: &mut Z80<Self>) -> Option<u32> {
    None // Default implementation
}
```

### 2. CPU Extension Infrastructure

**File: src/cpu_extensions.rs**

Defines the framework for CPU extensions:

```rust
pub struct CpuExtensionState {
    pub ext_num: u8,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
}

pub trait CpuExtensionHandler {
    fn extension_begin(&mut self, state: &mut CpuExtensionState) -> bool;
    fn extension_finish(&mut self, state: &mut CpuExtensionState) -> bool;
}
```

### 3. Disk ROM Patching

**File: src/disk_rom_manager.rs**

The disk ROM manager automatically detects and patches disk ROMs:

1. Searches for MSX-DOS BIOS jump table (pattern: `C3 XX XX` repeated)
2. Replaces jump instructions with CPU extension calls:
   - `ED E0 C9` for INIENV
   - `ED E2 C9` for DRIVES
   - `ED E4 C9` for DSKIO
   - etc.

```rust
pub fn patch_disk_rom(rom_slot: &mut RomSlot) -> bool {
    // Find jump table pattern
    let jump_table = find_jump_table(&rom_data);
    
    // Patch each entry
    rom_slot.write(addr, 0xED);      // Extension prefix
    rom_slot.write(addr + 1, ext);   // Extension number
    rom_slot.write(addr + 2, 0xC9);  // RET
}
```

### 4. MSX-DOS BIOS Implementation

**File: src/disk_driver.rs**

Implements MSX-DOS BIOS functions as CPU extensions:

| Extension | Function | Description |
|-----------|----------|-------------|
| 0xE0 | INIENV | Initialize disk environment |
| 0xE2 | DRIVES | Get number of drives |
| 0xE4 | DSKIO | Read/write disk sectors |
| 0xE5 | DSKCHG | Check if disk changed |
| 0xE6 | GETDPB | Get disk parameter block |
| 0xE8 | DSKFMT | Format disk (stubbed) |
| 0xEA | MTOFF | Turn off motor |

Example implementation:
```rust
fn dskio(&mut self, state: &mut CpuExtensionState) -> bool {
    let function = state.bc & 0xFF;
    let drive = (state.af & 0xFF) as u8;
    let sector_count = state.bc >> 8;
    let start_sector = state.de;
    let buffer_addr = state.hl;
    
    match function {
        0 => self.read_sectors(state, drive, start_sector, sector_count, buffer_addr),
        1 => self.write_sectors(state, drive, start_sector, sector_count, buffer_addr),
        _ => false,
    }
}
```

### 5. Disk Image Support

**File: src/dsk_image.rs**

Supports standard MSX DSK image formats:

```rust
impl DiskImage {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, DiskError> {
        let (media_type, sectors_per_track, total_sectors, tracks, sides) = 
            match data.len() {
                368640 => (0xF8, 9, 720, 80, 1),   // 360KB
                737280 => (0xF9, 9, 1440, 80, 2),  // 720KB
                _ => return Err(DiskError::InvalidSize),
            };
        // ...
    }
}
```

### 6. Disk Drive Management

**File: src/disk_drive.rs**

Thread-safe disk drive management:

```rust
#[derive(Clone)]
pub struct SharedDiskDrive(Arc<Mutex<DiskDrive>>);

pub struct DiskDrive {
    drives: [Option<DiskImage>; 2],  // A: and B:
    disk_changed: [Option<bool>; 2],
    motor_on: [bool; 2],
    motor_off_time: [Option<std::time::Instant>; 2],
}
```

### 7. Machine Integration

**File: src/machine.rs**

The machine automatically detects and initializes disk support:

```rust
fn check_and_setup_disk_system(&mut self) {
    // Check if slot 1 contains a disk ROM
    let has_disk_rom = {
        let slot1 = bus.get_slot(1);
        if slot1.size() >= 0x4000 {
            let byte0 = slot1.read(0x4000);
            let byte1 = slot1.read(0x4001);
            byte0 == 0x41 && byte1 == 0x42  // 'AB' header
        } else {
            false
        }
    };
    
    if has_disk_rom {
        // Patch ROM and setup disk system
        DiskRomManager::patch_disk_rom(rom_slot);
        let disk_drive = SharedDiskDrive::new();
        DiskRomManager::setup_disk_system(&self.cpu.io, disk_drive.clone(), self.bus.clone());
    }
}
```

### 8. WASM Bindings

**File: src/lib.rs**

Exposes disk functionality to JavaScript:

```rust
#[wasm_bindgen(js_name=insertDisk)]
pub fn insert_disk(&mut self, drive: u8, data: &[u8], _filename: &str) -> Result<(), JsValue> {
    self.0.load_disk_image(drive, data.to_vec())
        .map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen(js_name=enableDiskSystem)]
pub fn enable_disk_system(&mut self) -> Result<(), JsValue> {
    // Disk system is automatically enabled when disk ROM detected
}
```

## Usage

### Loading a Disk ROM

```javascript
// Load BIOS and disk ROM
const machine = new Machine(biosData, diskRomData);
// Disk system is automatically initialized if disk ROM detected
```

### Inserting a Disk

```javascript
// Insert disk image into drive A (0)
machine.insertDisk(0, diskImageData, "game.dsk");
```

### Ejecting a Disk

```javascript
// Eject disk from drive A
machine.ejectDisk(0);
```

## Technical Achievements

1. **Zero Hardware Emulation**: No FDC chip emulation required
2. **Minimal CPU Changes**: Only 15 lines added to Z80 core
3. **Automatic Configuration**: Disk system self-configures when ROM detected
4. **High Compatibility**: Works with standard MSX-DOS disk images
5. **Thread-Safe**: All operations safe for multi-threaded WASM environment

## Limitations

1. Only supports standard 360KB and 720KB DSK images
2. No low-level FDC operations (direct sector access only)
3. Format operation is stubbed (returns success without formatting)
4. No support for copy-protected disks requiring FDC tricks

## Future Enhancements

1. Support for additional disk image formats (DMK, IMD)
2. Implement actual formatting functionality
3. Add support for hard disk images
4. Implement Nextor (MSX-DOS 2.x) support

## Testing

The implementation can be tested with:
1. MSX-DOS boot disks
2. Game disks that use standard disk access
3. Productivity software using MSX-DOS calls

Example test scenario:
```basic
10 CALL SYSTEM
A>DIR
```

This should boot to MSX-DOS and show directory listing.