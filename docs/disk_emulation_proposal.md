# Disk Emulation Implementation Proposal for Rust MSX Emulator

## Executive Summary

This proposal outlines a phased approach to implement disk drive emulation in the Rust MSX emulator, based on the WebMSX architecture analysis. Phase 1 focuses on DSK image support with a minimal but extensible design.

## Phase 1 Goals

- Support loading and reading DSK image files (360KB/720KB)
- Implement MSX-DOS 1 compatibility
- Create infrastructure for future enhancements
- Maintain clean separation between emulation layers

## Proposed Architecture

```
MSX Software (Z80)
    ↓
CPU Extension Mechanism (cpu.rs)
    ↓
Bus Extension Routing (bus.rs)
    ↓
Disk ROM Slot (slot/disk_rom.rs)
    ↓
Disk Driver (disk_driver.rs)
    ↓
Disk Drive (disk_drive.rs)
    ↓
DSK Image Handler (dsk_image.rs)
```

## Implementation Plan

### 1. CPU Extension Mechanism

**File**: `src/cpu_extensions.rs` (new)

```rust
#[derive(Debug, Clone)]
pub struct CpuExtensionState {
    pub ext_num: u8,
    pub ext_pc: u16,
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
    fn extension_begin(&mut self, state: &CpuExtensionState) -> Option<CpuExtensionState>;
    fn extension_finish(&mut self, state: &CpuExtensionState) -> Option<CpuExtensionState>;
}
```

**Modifications to** `src/instruction.rs`:

Add ED E0-EF instruction handling:

```rust
// In instruction decoding
0xED => {
    let next_byte = self.fetch_byte();
    match next_byte {
        0xE0..=0xEF => {
            // CPU extension
            self.execute_extension(next_byte);
        }
        // ... existing ED instructions
    }
}
```

### 2. Bus Extension Support

**Modifications to** `src/bus.rs`:

```rust
impl Bus {
    pub fn register_extension_handler(&mut self, ext_num: u8, handler: Box<dyn CpuExtensionHandler>) {
        self.extension_handlers.insert(ext_num, handler);
    }
    
    pub fn cpu_extension_begin(&mut self, state: &CpuExtensionState) -> Option<CpuExtensionState> {
        if state.ext_num < 0xF0 {
            // Route to slot containing the instruction
            let slot = self.get_slot_for_address(state.ext_pc);
            slot.cpu_extension_begin(state)
        } else {
            // Route to registered handler
            if let Some(handler) = self.extension_handlers.get_mut(&state.ext_num) {
                handler.extension_begin(state)
            } else {
                None
            }
        }
    }
}
```

### 3. Disk ROM Slot Implementation

**File**: `src/slot/disk_rom.rs` (new)

```rust
pub struct DiskRomSlot {
    rom_data: Vec<u8>,
    base_address: u16,
    disk_driver: DiskDriver,
}

impl DiskRomSlot {
    pub fn new(rom_data: Vec<u8>) -> Self {
        let mut slot = Self {
            rom_data: rom_data.clone(),
            base_address: if rom_data.len() == 0x4000 { 0x4000 } else { 0x0000 },
            disk_driver: DiskDriver::new(),
        };
        slot.patch_bios();
        slot
    }
    
    fn patch_bios(&mut self) {
        // Patch disk BIOS entry points with CPU extensions
        // DSKIO at jump table offset
        let dskio_addr = self.get_jump_table_address(0);
        self.patch_location(dskio_addr, &[0xED, 0xE4, 0xC9]); // ED E4, RET
        
        // DSKCHG at jump table offset
        let dskchg_addr = self.get_jump_table_address(3);
        self.patch_location(dskchg_addr, &[0xED, 0xE5, 0xC9]); // ED E5, RET
        
        // Continue for other disk BIOS functions...
    }
}

impl Slot for DiskRomSlot {
    fn read(&self, address: u16) -> u8 {
        if address >= self.base_address && address < self.base_address + self.rom_data.len() as u16 {
            self.rom_data[(address - self.base_address) as usize]
        } else {
            0xFF
        }
    }
    
    fn cpu_extension_begin(&mut self, state: &CpuExtensionState) -> Option<CpuExtensionState> {
        self.disk_driver.cpu_extension_begin(state)
    }
}
```

### 4. Disk Driver Implementation

**File**: `src/disk_driver.rs` (new)

```rust
pub struct DiskDriver {
    disk_drive: Arc<Mutex<DiskDrive>>,
}

impl DiskDriver {
    pub fn new() -> Self {
        Self {
            disk_drive: Arc::new(Mutex::new(DiskDrive::new())),
        }
    }
}

impl CpuExtensionHandler for DiskDriver {
    fn extension_begin(&mut self, state: &CpuExtensionState) -> Option<CpuExtensionState> {
        let mut result = state.clone();
        
        match state.ext_num {
            0xE4 => self.dskio(&mut result),
            0xE5 => self.dskchg(&mut result),
            0xE6 => self.getdpb(&mut result),
            0xE8 => self.dskfmt(&mut result),
            _ => return None,
        }
        
        Some(result)
    }
    
    fn extension_finish(&mut self, _state: &CpuExtensionState) -> Option<CpuExtensionState> {
        // Motor off for all drives
        if let Ok(mut drive) = self.disk_drive.lock() {
            drive.all_motors_off();
        }
        None
    }
}

impl DiskDriver {
    fn dskio(&self, state: &mut CpuExtensionState) -> bool {
        let drive_num = state.a;
        let sector_count = (state.bc & 0xFF) as u8;
        let logical_sector = state.de;
        let memory_address = state.hl;
        let is_write = (state.f & 0x01) != 0;
        
        if let Ok(mut drive) = self.disk_drive.lock() {
            if is_write {
                // Phase 1: Read-only support
                state.f |= 0x01;  // Set carry flag (error)
                state.a = 0x00;   // Write protect error
                false
            } else {
                // Read sectors
                match drive.read_sectors(drive_num, logical_sector, sector_count) {
                    Ok(data) => {
                        // TODO: Write data to memory at memory_address
                        // This requires access to the memory system
                        state.f &= !0x01;  // Clear carry flag (success)
                        state.bc = 0;      // All sectors transferred
                        true
                    }
                    Err(_) => {
                        state.f |= 0x01;   // Set carry flag (error)
                        state.a = 0x02;    // Not ready error
                        false
                    }
                }
            }
        } else {
            false
        }
    }
    
    fn dskchg(&self, state: &mut CpuExtensionState) -> bool {
        let drive_num = state.a;
        
        if let Ok(mut drive) = self.disk_drive.lock() {
            match drive.disk_changed(drive_num) {
                Some(false) => {
                    // Disk not changed
                    state.f &= !0x01;  // Clear carry
                    state.bc = 0x0001; // B=0, disk not changed
                    true
                }
                Some(true) => {
                    // Disk changed
                    state.f &= !0x01;  // Clear carry
                    state.bc = 0xFF00; // B=FF, disk changed
                    true
                }
                None => {
                    // No disk
                    state.f |= 0x01;   // Set carry
                    state.a = 0x02;    // Not ready
                    false
                }
            }
        } else {
            false
        }
    }
    
    fn getdpb(&self, state: &mut CpuExtensionState) -> bool {
        let media_type = if state.bc < 0xF8 { (state.bc >> 8) as u8 } else { state.bc as u8 };
        
        // Return disk parameter block for media type
        // Phase 1: Support F8 (360KB) and F9 (720KB)
        match media_type {
            0xF8 => {
                // 360KB parameters
                state.f &= !0x01;  // Success
                true
            }
            0xF9 => {
                // 720KB parameters
                state.f &= !0x01;  // Success
                true
            }
            _ => {
                state.f |= 0x01;   // Error
                false
            }
        }
    }
    
    fn dskfmt(&self, state: &mut CpuExtensionState) -> bool {
        // Phase 1: Not implemented
        state.f |= 0x01;  // Set carry (error)
        state.a = 0x00;   // Write protect error
        false
    }
}
```

### 5. Disk Drive Implementation

**File**: `src/disk_drive.rs` (new)

```rust
use std::fs::File;
use std::io::Read;

pub struct DiskDrive {
    drives: [Option<DiskImage>; 2],  // A: and B:
    disk_changed: [Option<bool>; 2],
    motor_on: [bool; 2],
}

impl DiskDrive {
    pub fn new() -> Self {
        Self {
            drives: [None, None],
            disk_changed: [None, None],
            motor_on: [false, false],
        }
    }
    
    pub fn insert_disk(&mut self, drive: u8, image: DiskImage) {
        if drive < 2 {
            self.drives[drive as usize] = Some(image);
            self.disk_changed[drive as usize] = Some(true);
        }
    }
    
    pub fn disk_changed(&mut self, drive: u8) -> Option<bool> {
        if drive >= 2 {
            return None;
        }
        
        let changed = self.disk_changed[drive as usize];
        if changed == Some(true) {
            self.disk_changed[drive as usize] = Some(false);
        }
        changed
    }
    
    pub fn read_sectors(&mut self, drive: u8, start_sector: u16, count: u8) -> Result<Vec<u8>, DiskError> {
        if drive >= 2 {
            return Err(DiskError::InvalidDrive);
        }
        
        self.motor_on[drive as usize] = true;
        
        if let Some(disk) = &self.drives[drive as usize] {
            disk.read_sectors(start_sector, count)
        } else {
            Err(DiskError::NoDisk)
        }
    }
    
    pub fn all_motors_off(&mut self) {
        self.motor_on[0] = false;
        self.motor_on[1] = false;
    }
}

#[derive(Debug)]
pub enum DiskError {
    InvalidDrive,
    NoDisk,
    InvalidSector,
    ReadError,
}
```

### 6. DSK Image Handler

**File**: `src/dsk_image.rs` (new)

```rust
pub struct DiskImage {
    data: Vec<u8>,
    media_type: u8,
    sectors_per_track: u16,
    total_sectors: u16,
}

impl DiskImage {
    pub fn load_from_file(path: &str) -> Result<Self, std::io::Error> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        
        // Validate disk size
        let media_type = match data.len() {
            368640 => 0xF8,  // 360KB
            737280 => 0xF9,  // 720KB
            _ => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid disk image size"
            )),
        };
        
        Ok(Self {
            data,
            media_type,
            sectors_per_track: 9,
            total_sectors: (data.len() / 512) as u16,
        })
    }
    
    pub fn read_sectors(&self, start_sector: u16, count: u8) -> Result<Vec<u8>, DiskError> {
        let start_byte = start_sector as usize * 512;
        let end_byte = start_byte + (count as usize * 512);
        
        if end_byte > self.data.len() {
            return Err(DiskError::InvalidSector);
        }
        
        Ok(self.data[start_byte..end_byte].to_vec())
    }
    
    pub fn get_media_type(&self) -> u8 {
        self.media_type
    }
}
```

### 7. Integration with Machine

**Modifications to** `src/machine.rs`:

```rust
impl Machine {
    pub fn insert_disk_rom(&mut self, rom_data: Vec<u8>, slot: u8) {
        let disk_slot = DiskRomSlot::new(rom_data);
        self.bus.insert_slot(Box::new(disk_slot), slot);
    }
    
    pub fn insert_disk(&mut self, drive: u8, image_path: &str) -> Result<(), std::io::Error> {
        let image = DiskImage::load_from_file(image_path)?;
        // TODO: Access disk drive through the disk ROM slot
        Ok(())
    }
}
```

## Memory Access Considerations

One challenge is that disk operations need to transfer data directly to/from MSX memory. Solutions:

1. **Pass Memory Reference**: Extension handlers receive a reference to the memory system
2. **Memory Callback**: Extension handlers use a callback to read/write memory
3. **Deferred Operations**: Extension returns memory operations to be executed by CPU

Recommended approach for Phase 1: **Memory Callback**

```rust
pub trait MemoryAccess {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
}

impl CpuExtensionHandler for DiskDriver {
    fn extension_begin(&mut self, state: &CpuExtensionState, mem: &mut dyn MemoryAccess) 
        -> Option<CpuExtensionState> {
        // Can now read/write memory during disk operations
    }
}
```

## Testing Strategy

1. **Unit Tests**:
   - DSK image loading and validation
   - Sector read operations
   - CPU extension mechanism

2. **Integration Tests**:
   - Load disk ROM and verify patching
   - Execute disk operations via extensions
   - Verify correct data transfer

3. **Compatibility Tests**:
   - Boot MSX-DOS from disk image
   - Run disk-based software
   - Verify timing behavior

## Phase 1 Limitations

- Read-only disk support
- No write operations
- No formatting support
- Fixed to 360KB/720KB formats
- No hard disk support
- No disk change simulation with motor delays

## Future Phases

**Phase 2**: Write support and formatting
**Phase 3**: Hard disk support
**Phase 4**: Advanced features (compression, multiple disks per file)

## Conclusion

This proposal provides a solid foundation for disk emulation that:
- Follows WebMSX's proven architecture
- Maintains clean separation of concerns
- Allows incremental feature addition
- Integrates cleanly with existing Rust codebase

The CPU extension mechanism is key to maintaining accuracy while keeping the implementation manageable and performant.