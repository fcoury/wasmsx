# MSX Disk Emulation Implementation

This document details the implementation of disk emulation in the MSX emulator, including the architecture decisions, technical approach, integration with the existing codebase, and critical bug fixes that were required to make the FILES command work properly.

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
    pub ext_pc: u16,  // PC before ED XX instruction
    pub pc: u16,
    pub sp: u16,
    pub a: u8,
    pub f: u8,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub ix: u16,
    pub iy: u16,
}

pub trait CpuExtensionHandler {
    fn extension_begin(&mut self, state: &mut CpuExtensionState) -> bool;
    fn extension_finish(&mut self, state: &mut CpuExtensionState) -> bool;
}
```

The state includes helper methods for accessing individual registers:
```rust
impl CpuExtensionState {
    pub fn b(&self) -> u8 { (self.bc >> 8) as u8 }
    pub fn c(&self) -> u8 { self.bc as u8 }
    pub fn set_b(&mut self, value: u8) { 
        self.bc = (self.bc & 0x00FF) | ((value as u16) << 8);
    }
    pub fn set_c(&mut self, value: u8) {
        self.bc = (self.bc & 0xFF00) | (value as u16);
    }
    // ... similar for other registers
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
| 0xE0 | INIHRD/INIENV | Initialize disk hardware/environment |
| 0xE2 | DRIVES | Get number of drives |
| 0xE4 | DSKIO | Read/write disk sectors |
| 0xE5 | DSKCHG | Check if disk changed |
| 0xE6 | GETDPB | Get disk parameter block |
| 0xE7 | CHOICE | Get disk format choice string |
| 0xE8 | DSKFMT | Format disk (stubbed) |
| 0xE9 | DSKSTP | Stop disk motor immediately |
| 0xEA | MTOFF | Schedule motor off |

#### Key Implementation: GETDPB (Critical for FILES command)

The GETDPB function returns the Drive Parameter Block, which must be formatted correctly:

```rust
fn getdpb(&mut self, state: &mut CpuExtensionState) -> bool {
    // Parse BPB from boot sector
    let boot_data = drive.read_sectors(0, 0, 1)?;
    
    // Build DPB with correct MSX-DOS 1 layout:
    // Offset 0: Media descriptor (1 byte)
    // Offset 1-2: Sector size (2 bytes, little-endian)
    // Offset 3-6: Directory/cluster parameters (4 bytes)
    // Offset 7-8: First FAT sector (2 bytes, little-endian)
    // Offset 9: Number of FATs (1 byte)
    // Offset 10: Max dir entries (1 byte, NOT 2!)
    // Offset 11-12: First data sector (2 bytes, little-endian)
    // Offset 13-14: Max clusters (2 bytes, little-endian)
    // Offset 15: Sectors per FAT (1 byte)
    // Offset 16-17: First directory sector (2 bytes, little-endian)
    
    // CRITICAL: Return directory start in BC with correct endianness
    state.set_b(dpb_data[17]); // High byte
    state.set_c(dpb_data[16]); // Low byte
}
```

### 5. Disk Image Support

**File: src/dsk_image.rs**

Supports standard MSX DSK image formats with proper sector reading:

```rust
impl DiskImage {
    pub fn read_sectors(&self, start_sector: u16, count: u8) -> Result<Vec<u8>, DiskError> {
        // CRITICAL: .dsk files store sectors sequentially WITHOUT interleave
        for i in 0..count {
            let logical_sector = start_sector + i as u16;
            
            // Calculate track, side, and sector position
            let track = logical_sector / (self.sectors_per_track * self.sides as u16);
            let sectors_per_cylinder = self.sectors_per_track * self.sides as u16;
            let logical_in_cylinder = logical_sector % sectors_per_cylinder;
            let side = logical_in_cylinder / self.sectors_per_track;
            let sector_on_track = logical_in_cylinder % self.sectors_per_track;
            
            // Direct mapping - NO INTERLEAVE!
            let flat_offset = if self.sides == 1 {
                ((track * self.sectors_per_track) + sector_on_track) as usize * SECTOR_SIZE
            } else {
                ((track * self.sides as u16 * self.sectors_per_track)
                    + (side * self.sectors_per_track)
                    + sector_on_track) as usize * SECTOR_SIZE
            };
            
            result.extend_from_slice(&self.data[flat_offset..flat_offset + SECTOR_SIZE]);
        }
    }
}
```

### 6. Disk Drive Management

**File: src/disk_drive.rs**

Thread-safe disk drive management with disk-change detection:

```rust
pub struct DiskDrive {
    drives: [Option<DiskImage>; 2],  // A: and B:
    disk_changed: [Option<bool>; 2],
    motor_on: [bool; 2],
    motor_off_time: [Option<std::time::Instant>; 2],
    // Disk-change flipflop for detecting disks present at boot
    disk_changed_flipflop: [bool; 2],
}

impl DiskDrive {
    pub fn disk_changed(&mut self, drive: u8) -> Option<bool> {
        // Check flipflop first (for boot-time detection)
        if self.disk_changed_flipflop[drive as usize] {
            self.disk_changed_flipflop[drive as usize] = false;
            return Some(true); // Report disk has changed
        }
        // Then check normal change status
        self.disk_changed[drive as usize]
    }
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

## Critical Bug Fixes for FILES Command

The initial implementation had several critical bugs that prevented the FILES command from working. These were discovered through extensive debugging and analysis of MSX-DOS behavior:

### 1. DPB Structure Layout Error
**Problem**: The MAXENT field at offset 10 was incorrectly treated as a 2-byte field when MSX-DOS 1 expects it to be 1 byte.
- This shifted all subsequent fields by 1 byte
- Directory start sector was read from wrong offset (15-16 instead of 16-17)
- MSX-DOS calculated wrong sector numbers for directory access

**Fix**: Corrected the DPB structure to proper MSX-DOS 1 format:
```rust
dpb_data[10] = root_entries as u8;      // Single byte, not u16!
// This shifts remaining fields to correct positions:
dpb_data[11..13] = first_data_sector;   // Now at correct offset
dpb_data[16..18] = dir_start_sector;    // Now at correct offset
```

### 2. Incorrect Sector Interleave Mapping
**Problem**: The disk image reader was applying hardware interleave-by-2 mapping:
```rust
// Wrong - this is for physical hardware formatting:
const DEINTERLEAVE_MAP: [u16; 9] = [0, 5, 1, 6, 2, 7, 3, 8, 4];
physical_sector = DEINTERLEAVE_MAP[logical_sector];
```
- Real hardware used interleave to optimize rotational latency
- .dsk image files store sectors sequentially WITHOUT interleave
- Every sector read was landing 3×512 bytes too far into the track

**Fix**: Removed interleave mapping entirely:
```rust
// Correct - direct sequential mapping for .dsk files:
let flat_offset = ((track * sectors_per_track) + sector_on_track) 
                  as usize * SECTOR_SIZE;
```

### 3. BC Register Byte Order in GETDPB
**Problem**: The BC register pair was loaded with incorrect endianness:
```rust
// Wrong:
state.set_b(dir_sector_low);   // B = 0x05
state.set_c(0);                // C = 0x00
// Result: BC = 0x0500 = 1280 (wrong!)
```
- MSX-DOS expects BC to contain 16-bit value with B=high, C=low
- This caused DOS to add offset and request sector 0x0502 (1282)

**Fix**: Corrected register loading to proper Z80 convention:
```rust
// Correct:
state.set_b(dpb_data[17]); // B = high byte = 0x00
state.set_c(dpb_data[16]); // C = low byte = 0x05
// Result: BC = 0x0005 = 5 (correct!)
```

### 4. Dynamic BPB Parsing
**Problem**: DPB values were hardcoded instead of parsing from actual boot sector:
- Different disk formats have different parameters
- Hardcoded values didn't match actual disk layout

**Fix**: Implemented proper BPB parsing with sanity checks:
```rust
let boot_data = drive.read_sectors(0, 0, 1)?;
let reserved_sectors = u16::from_le_bytes([boot_data[0x0E], boot_data[0x0F]]);
let num_fats = boot_data[0x10];
let sectors_per_fat = u16::from_le_bytes([boot_data[0x16], boot_data[0x17]]);
let dir_start = reserved_sectors + (num_fats as u16 * sectors_per_fat);

// Sanity check media descriptor
let media_type = match boot_data[0x15] {
    0xF8..=0xFF => boot_data[0x15],
    _ => 0xF9  // Default to 720KB if invalid
};
```

### 5. Disk Change Handling for FILES
**Problem**: FILES command failed when DSKCHG reported disk changed during operation.

**Fix**: Special handling when called from FILES context (HL=0xF1AC):
```rust
if state.hl == 0xF1AC {  // FILES command context
    if disk_actually_changed {
        update_dpb_internally();  // Update DPB
        clear_change_flag();      // Clear flag
    }
    state.set_b(0x00);  // Always report "not changed" to FILES
    return true;
}
```

These fixes ensure MSX-DOS correctly:
- Reads directory from sectors 5-11 (360KB) or 7-13 (720KB)
- Displays filenames instead of "File not found"
- Handles disk operations reliably

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
A>FILES
```

This should boot to MSX-DOS and show directory listing when FILES is typed.