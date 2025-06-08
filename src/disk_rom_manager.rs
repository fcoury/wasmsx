// Disk ROM Manager - handles disk ROM patching and disk system setup

use crate::disk_driver::DiskDriver;
use crate::disk_drive::SharedDiskDrive;
use crate::slot::RomSlot;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::Bus;
use std::collections::HashMap;

pub struct DiskRomManager;

struct DiskRomOffsets {
    driver_start: usize,
    inihrd: usize,
    drives: usize,
    choice_str_addr: Option<usize>,
}

// Choice string is already in the disk ROM at the standard location

impl DiskRomManager {
    /// Patch a disk ROM to use CPU extensions instead of native code
    pub fn patch_disk_rom(rom_slot: &mut RomSlot) -> bool {
        // Use standard MSX-DOS disk ROM offsets
        // WebMSX patches at the ACTUAL routine addresses, not the caller addresses
        let offsets = DiskRomOffsets {
            driver_start: 0x0010,  // Standard jump table location
            inihrd: 0x3786,        // INIHRD routine location (0x7786 - 0x4000)
            drives: 0x37B6,        // DRIVES routine location (0x77B6 - 0x4000)
            choice_str_addr: Some(0x3893), // Choice string location (0x7893 - 0x4000)
        };
        
        Self::patch_disk_bios(&mut rom_slot.data, 0, offsets);
        tracing::info!("Disk ROM patched successfully with standard offsets");
        true
    }
    
    
    fn patch_disk_bios(bytes: &mut [u8], patch_base: usize, offsets: DiskRomOffsets) {
        let mut choice_addresses = HashMap::new();
        
        // The CHOICE string is already at the predefined location (0x3893)
        // We don't need to write it as it's part of the standard disk ROM
        
        // Disk Driver init routines not present on Jump Table. Patched at the ROUTINE location
        // INIHRD routine (EXT 0)
        if offsets.inihrd + 2 < bytes.len() {
            bytes[patch_base + offsets.inihrd + 0] = 0xED;
            bytes[patch_base + offsets.inihrd + 1] = 0xE0;
            bytes[patch_base + offsets.inihrd + 2] = 0xC9;  // RET
            tracing::info!("Patched INIHRD at routine location 0x{:04X}", offsets.inihrd);
        }
        
        // DRIVES routine (EXT 2)
        if offsets.drives + 2 < bytes.len() {
            bytes[patch_base + offsets.drives + 0] = 0xED;
            bytes[patch_base + offsets.drives + 1] = 0xE2;
            bytes[patch_base + offsets.drives + 2] = 0xC9;  // RET
            tracing::info!("Patched DRIVES at routine location 0x{:04X}", offsets.drives);
        }
        
        // DOS Kernel Jump Table for Disk Driver routines. Patched at the ROUTINE location
        let jump_table_patches = [
            (0, 0xE4, "DSKIO"),   // Extension E4
            (3, 0xE5, "DSKCHG"),  // Extension E5
            (6, 0xE6, "GETDPB"),  // Extension E6
            (9, 0xE7, "CHOICE"),  // Extension E7
            (12, 0xE8, "DSKFMT"), // Extension E8
            (15, 0xEA, "MTOFF"),  // Extension EA
        ];
        
        tracing::info!("CHOICE string address: {:?}", offsets.choice_str_addr);
        
        for (offset_index, ext_num, name) in jump_table_patches {
            let jp_offset = patch_base + offsets.driver_start + offset_index;
            
            // Read the jump destination from the JP instruction in the jump table
            if jp_offset + 2 < bytes.len() && bytes[jp_offset] == 0xC3 {  // Verify it's a JP instruction
                let dest_addr = bytes[jp_offset + 1] as usize | ((bytes[jp_offset + 2] as usize) << 8);
                // The dest_addr is an absolute address in the MSX memory map
                // We need to convert it to a ROM offset (assuming ROM is at 0x4000)
                if dest_addr >= 0x4000 && dest_addr < 0x8000 {
                    let patch_addr = patch_base + (dest_addr - 0x4000);
                    
                    if patch_addr + 2 < bytes.len() {
                        bytes[patch_addr + 0] = 0xED;
                        bytes[patch_addr + 1] = ext_num;
                        bytes[patch_addr + 2] = 0xC9;  // RET
                        
                        tracing::info!("Patched {} at routine address 0x{:04X} (ROM offset 0x{:04X}, jump from 0x{:04X}) with extension 0x{:02X}", 
                            name, dest_addr, patch_addr, jp_offset, ext_num);
                        
                        // Special handling for CHOICE - store the mapping of routine address to string address
                        if name == "CHOICE" && offsets.choice_str_addr.is_some() {
                            let str_addr = offsets.choice_str_addr.unwrap() + 0x4000;  // Convert to absolute address
                            choice_addresses.insert(dest_addr, str_addr);
                            tracing::info!("CHOICE: Mapping routine address 0x{:04X} to string address 0x{:04X}", dest_addr, str_addr);
                            // We need to pass this information to the disk driver somehow
                            // For now, we'll store it in a static variable (not ideal but works)
                            unsafe {
                                CHOICE_STRING_ADDRESSES.insert(dest_addr, str_addr);
                            }
                        }
                    }
                } else {
                    tracing::warn!("Jump destination 0x{:04X} for {} is outside ROM range", dest_addr, name);
                }
            }
        }
        
        // Note: We don't patch INIENV as it's not part of the standard WebMSX approach
    }
    
    /// Create and register disk driver with the CPU extension system
    pub fn setup_disk_system(
        io: &crate::machine::Io,
        disk_drive: SharedDiskDrive,
        bus: Rc<RefCell<Bus>>
    ) {
        let disk_driver = Arc::new(Mutex::new(DiskDriver::new(disk_drive.clone_inner(), bus)));
        
        // Register handlers for disk extensions
        // Include E0 (INIHRD/INIENV) to properly initialize the disk system
        let extensions = [0xE0, 0xE2, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA];
        
        for ext_num in extensions {
            let driver_clone = Arc::clone(&disk_driver);
            io.register_extension_handler(ext_num, Box::new(DiskDriverWrapper(driver_clone)));
        }
        
        tracing::info!("Disk system initialized with extensions");
    }
}

// Global storage for CHOICE string addresses (not ideal but simple)
// Maps CHOICE routine address -> string address in ROM
pub static mut CHOICE_STRING_ADDRESSES: once_cell::sync::Lazy<HashMap<usize, usize>> = 
    once_cell::sync::Lazy::new(|| HashMap::new());

// Wrapper to make Arc<Mutex<DiskDriver>> implement CpuExtensionHandler
struct DiskDriverWrapper(Arc<Mutex<DiskDriver>>);

impl crate::cpu_extensions::CpuExtensionHandler for DiskDriverWrapper {
    fn extension_begin(&mut self, state: &mut crate::cpu_extensions::CpuExtensionState) -> bool {
        if let Ok(mut driver) = self.0.lock() {
            driver.extension_begin(state)
        } else {
            false
        }
    }
    
    fn extension_finish(&mut self, state: &mut crate::cpu_extensions::CpuExtensionState) -> bool {
        if let Ok(mut driver) = self.0.lock() {
            driver.extension_finish(state)
        } else {
            false
        }
    }
}