// Disk Driver - implements MSX-DOS BIOS functions via CPU extensions

use crate::bus::Bus;
use crate::cpu_extensions::{CpuExtensionHandler, CpuExtensionState};
use crate::disk_drive::DiskDrive;
use crate::disk_error::DiskError;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const SECTOR_SIZE: usize = 512;

// MSX-DOS FCB structure offsets
const FCB_DRIVE: usize = 0; // Drive number (0=default, 1=A:, 2=B:, etc.)
const FCB_FILENAME: usize = 1; // 8-byte filename
const FCB_EXTENSION: usize = 9; // 3-byte extension
const FCB_EXTENT: usize = 12; // Current extent
const FCB_S1: usize = 13; // Reserved
const FCB_S2: usize = 14; // Reserved (used for record count)
const FCB_RC: usize = 15; // Record count in this extent
const FCB_AL: usize = 16; // Allocation map (16 bytes)
const FCB_CR: usize = 32; // Current record within extent
const FCB_R0: usize = 33; // Random record number (3 bytes)
const FCB_R1: usize = 34;
const FCB_R2: usize = 35;

pub struct DiskDriver {
    disk_drive: Arc<Mutex<DiskDrive>>,
    motor_off_counter: u32,
    bus: Rc<RefCell<Bus>>,
}

impl DiskDriver {
    pub fn new(disk_drive: Arc<Mutex<DiskDrive>>, bus: Rc<RefCell<Bus>>) -> Self {
        Self {
            disk_drive,
            motor_off_counter: 0,
            bus,
        }
    }

    fn get_default_dpb(media_type: u8) -> Result<Vec<u8>, ()> {
        match media_type {
            0xF8 => {
                // 360KB single-sided, 9 sectors/track
                Ok(vec![
                    0xF8,       // Offset 0: Media descriptor
                    0x00, 0x02, // Offset 1-2: Sector size (512) - little-endian
                    0x0F,       // Offset 3: Directory mask (16 entries per sector - 1)
                    0x04,       // Offset 4: Directory shift (2^4 = 16)
                    0x01,       // Offset 5: Cluster mask (2 sectors per cluster - 1)
                    0x01,       // Offset 6: Cluster shift (2^1 = 2)
                    0x01, 0x00, // Offset 7-8: First FAT sector (1) - little-endian
                    0x02,       // Offset 9: Number of FATs
                    0x70,       // Offset 10: Max dir entries (112) - single byte!
                    0x0C, 0x00, // Offset 11-12: First data sector (12) - little-endian
                    0x62, 0x01, // Offset 13-14: Highest cluster number (354) - little-endian
                    0x02,       // Offset 15: Sectors per FAT
                    0x05, 0x00, // Offset 16-17: First root directory sector (5) - little-endian
                ])
            }
            0xF9 => {
                // 720KB double-sided, 9 sectors/track
                Ok(vec![
                    0xF9,       // Offset 0: Media descriptor
                    0x00, 0x02, // Offset 1-2: Sector size (512) - little-endian
                    0x0F,       // Offset 3: Directory mask (16 entries per sector - 1)
                    0x04,       // Offset 4: Directory shift (2^4 = 16)
                    0x01,       // Offset 5: Cluster mask (2 sectors per cluster - 1)
                    0x01,       // Offset 6: Cluster shift (2^1 = 2)
                    0x01, 0x00, // Offset 7-8: First FAT sector (1) - little-endian
                    0x02,       // Offset 9: Number of FATs
                    0x70,       // Offset 10: Max dir entries (112) - single byte!
                    0x0E, 0x00, // Offset 11-12: First data sector (14) - little-endian
                    0xC8, 0x02, // Offset 13-14: Highest cluster number (712) - little-endian
                    0x03,       // Offset 15: Sectors per FAT
                    0x07, 0x00, // Offset 16-17: First root directory sector (7) - little-endian
                ])
            }
            _ => {
                tracing::warn!("GETDPB: Unsupported media type 0x{:02X}, defaulting to 0xF9", media_type);
                // Default to 720KB
                Ok(vec![
                    0xF9,       // Offset 0: Media descriptor
                    0x00, 0x02, // Offset 1-2: Sector size (512) - little-endian
                    0x0F,       // Offset 3: Directory mask (16 entries per sector - 1)
                    0x04,       // Offset 4: Directory shift (2^4 = 16)
                    0x01,       // Offset 5: Cluster mask (2 sectors per cluster - 1)
                    0x01,       // Offset 6: Cluster shift (2^1 = 2)
                    0x01, 0x00, // Offset 7-8: First FAT sector (1) - little-endian
                    0x02,       // Offset 9: Number of FATs
                    0x70,       // Offset 10: Max dir entries (112) - single byte!
                    0x0E, 0x00, // Offset 11-12: First data sector (14) - little-endian
                    0xC8, 0x02, // Offset 13-14: Highest cluster number (712) - little-endian
                    0x03,       // Offset 15: Sectors per FAT
                    0x07, 0x00, // Offset 16-17: First root directory sector (7) - little-endian
                ])
            }
        }
    }

    fn dskio(&mut self, state: &mut CpuExtensionState) -> bool {
        let drive_num = state.a & 0x01;
        let sector_count = state.b();
        let original_sector = state.de;
        let memory_address = state.hl;
        let is_write = state.carry_flag();

        let logical_sector_to_read = original_sector;

        // --- Common DSKIO logic ---
        tracing::info!(
            "DSKIO: drive={}, sectors={}, logical_sector_to_read={}, address=0x{:04X}, write={}, caller_PC=0x{:04X}",
            drive_num, sector_count, logical_sector_to_read, memory_address, is_write, state.pc
        );
        
        // Special logging for boot sector reads
        if logical_sector_to_read == 0 && !is_write {
            tracing::info!("DSKIO: Reading boot sector (sector 0) - this contains the BPB");
        }
        
        // Log all sector reads during FILES command for debugging
        if memory_address == 0xE500 || memory_address == 0xEBAC {
            tracing::info!("DSKIO: FILES command reading sector {} to address 0x{:04X}", logical_sector_to_read, memory_address);
        }

        if is_write {
            state.set_carry_flag(true);
            state.a = 0x00; // Write protect error
            state.set_b(sector_count);
            return false;
        }

        if let Ok(mut drive) = self.disk_drive.lock() {
            // Always attempt to read if a disk is present, regardless of motor state
            if drive.has_disk(drive_num) {
                match drive.read_sectors(drive_num, logical_sector_to_read, sector_count) {
                    Ok(data) => {
                        // Special handling for boot sector reads to examine BPB
                        if logical_sector_to_read == 0 && data.len() >= 32 {
                            tracing::info!("Boot sector first 32 bytes: {:02X?}", &data[0..32]);
                            if data.len() >= 0x18 {
                                // Log key BPB fields (DOS 1.x format)
                                tracing::info!("  Jump instruction: {:02X} {:02X} {:02X}", data[0], data[1], data[2]);
                                tracing::info!("  OEM name: {:?}", String::from_utf8_lossy(&data[3..11]));
                                tracing::info!("  Bytes per sector: {} (0x{:02X}{:02X})", 
                                    u16::from_le_bytes([data[0x0B], data[0x0C]]),
                                    data[0x0C], data[0x0B]);
                                tracing::info!("  Sectors per cluster: {}", data[0x0D]);
                                tracing::info!("  Reserved sectors: {} (0x{:02X}{:02X})", 
                                    u16::from_le_bytes([data[0x0E], data[0x0F]]),
                                    data[0x0F], data[0x0E]);
                                tracing::info!("  Number of FATs: {}", data[0x10]);
                                tracing::info!("  Root entries: {} (0x{:02X}{:02X})", 
                                    u16::from_le_bytes([data[0x11], data[0x12]]),
                                    data[0x12], data[0x11]);
                                tracing::info!("  Total sectors: {} (0x{:02X}{:02X})", 
                                    u16::from_le_bytes([data[0x13], data[0x14]]),
                                    data[0x14], data[0x13]);
                                tracing::info!("  Media descriptor: 0x{:02X}", data[0x15]);
                                tracing::info!("  Sectors per FAT: {} (0x{:02X}{:02X})", 
                                    u16::from_le_bytes([data[0x16], data[0x17]]),
                                    data[0x17], data[0x16]);
                                    
                                // Calculate where directory should be
                                let reserved = u16::from_le_bytes([data[0x0E], data[0x0F]]);
                                let num_fats = data[0x10];
                                let sectors_per_fat = u16::from_le_bytes([data[0x16], data[0x17]]);
                                let dir_start = reserved + (num_fats as u16 * sectors_per_fat);
                                tracing::info!("  → Directory should start at sector: {}", dir_start);
                            }
                        }
                        
                        // Also log sector 1 to see if it's FAT or directory
                        if logical_sector_to_read == 1 && data.len() >= 32 {
                            tracing::info!("Sector 1 first 32 bytes: {:02X?}", &data[0..32]);
                            // Check if it looks like FAT (starts with media descriptor) or directory entries
                            if data[0] == 0xF8 || data[0] == 0xF9 {
                                tracing::info!("  Looks like FAT data (media descriptor: {:02X})", data[0]);
                            } else if data[0] >= 0x20 && data[0] <= 0x7E {
                                tracing::info!("  Looks like directory entries (first char: '{}')", data[0] as char);
                                
                                // Log all 16 possible directory entries in this sector
                                tracing::info!("  Full sector size: {} bytes", data.len());
                                for i in 0..16 {
                                    let offset = i * 32;
                                    if offset + 32 <= data.len() {
                                        let entry_start = &data[offset..offset+32];
                                        if entry_start[0] == 0x00 {
                                            tracing::info!("  Entry {}: End of directory marker (0x00)", i);
                                            // Don't break - show what's after the end marker
                                        } else if entry_start[0] == 0xE5 {
                                            tracing::info!("  Entry {}: Deleted entry (0xE5)", i);
                                        } else if entry_start[0] >= 0x20 && entry_start[0] <= 0x7E {
                                            let filename: String = entry_start[0..11].iter()
                                                .map(|&b| if b >= 0x20 && b <= 0x7E { b as char } else { '.' })
                                                .collect();
                                            tracing::info!("  Entry {}: '{}' (first 32 bytes: {:02X?})", i, filename, entry_start);
                                        } else {
                                            tracing::info!("  Entry {}: Invalid (first 32 bytes: {:02X?})", i, entry_start);
                                        }
                                    }
                                }
                            }
                        }
                        
                        self.bus.borrow_mut().write_block(memory_address, &data);
                        
                        // Additional logging for FILES command debugging
                        if memory_address == 0xE500 || memory_address == 0xEBAC {
                            tracing::info!("FILES: Written {} bytes to 0x{:04X}", data.len(), memory_address);
                            // Log memory dump after writing to help debug FILES parsing
                            let bus = self.bus.borrow();
                            for i in 0..3 {  // First 3 entries
                                let entry_addr = memory_address + (i * 32) as u16;
                                let mut entry_data = vec![0u8; 32];
                                for j in 0..32 {
                                    entry_data[j] = bus.read_byte(entry_addr + j as u16);
                                }
                                if entry_data[0] == 0x00 {
                                    tracing::info!("  Memory Entry {}: End of directory", i);
                                    break;
                                } else if entry_data[0] == 0xE5 {
                                    tracing::info!("  Memory Entry {}: Deleted", i);
                                } else if entry_data[0] >= 0x20 && entry_data[0] <= 0x7E {
                                    let filename: String = entry_data[0..11].iter()
                                        .map(|&b| if b >= 0x20 && b <= 0x7E { b as char } else { '.' })
                                        .collect();
                                    tracing::info!("  Memory Entry {} @ 0x{:04X}: '{}' [{:02X?}]", 
                                        i, entry_addr, filename, &entry_data[0..11]);
                                }
                            }
                            drop(bus);
                        }
                        
                        let media_type = drive.get_disk_info(drive_num).map_or(0xF8, |d| d.0);
                        state.set_carry_flag(false);
                        state.a = media_type;
                        state.set_b(0); // 0 sectors not transferred
                        return true;
                    }
                    Err(err) => {
                        // If the error is NoDisk, we let it fall through to the NoDisk handling below.
                        // Otherwise, we handle other read errors here.
                        if err != DiskError::NoDisk {
                            tracing::warn!("DSKIO read error: {:?}", err);
                            state.set_carry_flag(true);
                            state.a = match err {
                                DiskError::InvalidSector => 0x08,
                                _ => 0x0C,
                            };
                            state.set_b(sector_count);
                            return false;
                        }
                        // NoDisk error means we should continue to the normal NoDisk handling
                    }
                }
            }

            // If we reach here, either there's no disk or we got a NoDisk error
            tracing::warn!("DSKIO read error: NoDisk");
            state.set_carry_flag(true);
            state.a = 0x02; // Not ready
            state.set_b(sector_count);
            false
        } else {
            state.set_carry_flag(true);
            state.a = 0x0C; // General error
            false
        }
    }

    fn dskchg(&mut self, state: &mut CpuExtensionState) -> bool {
        let drive_num = state.a & 0x01;

        tracing::debug!(
            "DSKCHG: drive={}, HL=0x{:04X}, B=0x{:02X}, C=0x{:02X}",
            drive_num,
            state.hl,
            state.b(),
            state.c()
        );

        // Specific workaround for the 'FILES' command.
        // The BASIC interpreter's FILES command calls DSKCHG with HL pointing to the DPB (0xF1AC).
        // If we report "disk changed" here, it gets confused and fails with "File not found".
        // To solve this, we check for disk change internally. If it has changed, we update
        // the DPB ourselves, but still report "not changed" to the caller to keep it happy.
        if state.hl == 0xF1AC {
            tracing::debug!("DSKCHG: FILES command context detected (HL=0xF1AC).");

            let changed = if let Ok(mut drive) = self.disk_drive.lock() {
                drive.disk_changed(drive_num)
            } else {
                Some(false) // Assume not changed if drive lock fails
            };

            if changed == Some(true) {
                tracing::debug!(
                    "DSKCHG+FILES: Disk has changed, updating DPB internally before proceeding."
                );

                let mut media_desc_opt = None;
                if let Ok(drive) = self.disk_drive.lock() {
                    if let Some((media_type, _, _, _, _)) = drive.get_disk_info(drive_num) {
                        media_desc_opt = Some(media_type);
                    }
                }

                if let Some(media_desc) = media_desc_opt {
                    tracing::debug!(
                        "DSKCHG+FILES: Found media descriptor 0x{:02X}, updating DPB.",
                        media_desc
                    );
                    let mut dpb_state = state.clone();
                    dpb_state.set_b(media_desc);
                    dpb_state.set_c(media_desc);
                    self.getdpb(&mut dpb_state);
                }

                if let Ok(mut drive) = self.disk_drive.lock() {
                    drive.clear_disk_changed(drive_num);
                }
            }

            tracing::debug!("DSKCHG+FILES: Forcing 'not changed' status to caller.");
            state.set_carry_flag(false);
            state.set_b(0x00); // B=0, disk not changed
            return true;
        }

        // Original logic for all other DSKCHG calls
        let (disk_state, media_desc_opt) = if let Ok(mut drive) = self.disk_drive.lock() {
            let changed = drive.disk_changed(drive_num);
            let media_desc = if changed == Some(true) {
                if let Ok(data) = drive.read_sectors(drive_num, 1, 1) {
                    if !data.is_empty() {
                        Some(data[0])
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            (changed, media_desc)
        } else {
            (None, None)
        };

        match disk_state {
            Some(false) => {
                state.set_carry_flag(false);
                state.set_b(0x00); // B=0, disk not changed
                tracing::debug!("DSKCHG: Disk not changed");
                true
            }
            Some(true) => {
                state.set_carry_flag(false);
                state.set_b(0xFF); // B=FF, disk changed
                tracing::debug!("DSKCHG: Disk changed!");
                if let Some(media_desc) = media_desc_opt {
                    tracing::debug!(
                        "DSKCHG: Auto-updating DPB with media type 0x{:02X}",
                        media_desc
                    );
                    let mut dpb_state = state.clone();
                    dpb_state.set_b(media_desc);
                    dpb_state.set_c(media_desc);
                    self.getdpb(&mut dpb_state);
                }
                true
            }
            None => {
                state.set_carry_flag(true);
                state.a = 0x02; // Not ready
                tracing::debug!("DSKCHG: No disk");
                false
            }
        }
    }

    fn getdpb(&mut self, state: &mut CpuExtensionState) -> bool {
        // Media type is in BC (if < 0xF8) or C (if >= 0xF8)
        let requested_media_type = if state.bc < 0xF8 {
            state.b()
        } else {
            state.c()
        };
        let dpb_address = state.hl;
        let drive_num = 0; // For now, assume drive A:

        // Read boot sector to parse BPB
        let boot_sector_data = if let Ok(mut drive_guard) = self.disk_drive.lock() {
            if drive_guard.has_disk(drive_num) {
                match drive_guard.read_sectors(drive_num, 0, 1) {
                    Ok(data) => Some(data),
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        // Parse BPB from boot sector if available
        let (dpb_data, media_type) = if let Some(boot_data) = boot_sector_data {
            if boot_data.len() >= 0x18 {
                // Parse actual BPB fields
                let bytes_per_sector = u16::from_le_bytes([boot_data[0x0B], boot_data[0x0C]]);
                let sectors_per_cluster = boot_data[0x0D];
                let reserved_sectors = u16::from_le_bytes([boot_data[0x0E], boot_data[0x0F]]);
                let num_fats = boot_data[0x10];
                let root_entries = u16::from_le_bytes([boot_data[0x11], boot_data[0x12]]);
                let total_sectors = u16::from_le_bytes([boot_data[0x13], boot_data[0x14]]);
                let media_descriptor = boot_data[0x15];
                let sectors_per_fat = u16::from_le_bytes([boot_data[0x16], boot_data[0x17]]);

                // Sanity check media descriptor
                let media_type = match media_descriptor {
                    0xF8 | 0xF9 | 0xFA | 0xFB | 0xFC | 0xFD | 0xFE | 0xFF => media_descriptor,
                    _ => {
                        tracing::warn!("Invalid media descriptor 0x{:02X}, defaulting to 0xF9", media_descriptor);
                        0xF9 // Default to 720KB
                    }
                };

                // Calculate directory start sector
                let dir_start_sector = reserved_sectors + (num_fats as u16 * sectors_per_fat);
                
                // Calculate first data sector
                let dir_sectors = ((root_entries * 32) + (bytes_per_sector - 1)) / bytes_per_sector;
                let first_data_sector = dir_start_sector + dir_sectors;
                
                // Calculate total data sectors and clusters
                let data_sectors = total_sectors - first_data_sector;
                let total_clusters = data_sectors / sectors_per_cluster as u16;

                tracing::info!(
                    "GETDPB: Parsed BPB - media=0x{:02X}, dir_start={}, data_start={}, clusters={}",
                    media_type, dir_start_sector, first_data_sector, total_clusters
                );

                // Build DPB from parsed BPB data - MSX-DOS 1 format
                (vec![
                    media_type,                                      // Offset 0: Media descriptor
                    (bytes_per_sector & 0xFF) as u8,                // Offset 1: Sector size (low)
                    ((bytes_per_sector >> 8) & 0xFF) as u8,         // Offset 2: Sector size (high)
                    0x0F,                                            // Offset 3: Directory mask (16 entries per sector - 1)
                    0x04,                                            // Offset 4: Directory shift (2^4 = 16)
                    sectors_per_cluster - 1,                         // Offset 5: Cluster mask
                    (sectors_per_cluster as f32).log2() as u8,      // Offset 6: Cluster shift
                    (reserved_sectors & 0xFF) as u8,                // Offset 7: First FAT sector (low)
                    ((reserved_sectors >> 8) & 0xFF) as u8,         // Offset 8: First FAT sector (high)
                    num_fats,                                        // Offset 9: Number of FATs
                    root_entries as u8,                              // Offset 10: Max dir entries (only low byte used)
                    (first_data_sector & 0xFF) as u8,               // Offset 11: First data sector (low)
                    ((first_data_sector >> 8) & 0xFF) as u8,        // Offset 12: First data sector (high)
                    (total_clusters & 0xFF) as u8,                  // Offset 13: Highest cluster number (low)
                    ((total_clusters >> 8) & 0xFF) as u8,           // Offset 14: Highest cluster number (high)
                    sectors_per_fat as u8,                           // Offset 15: Sectors per FAT
                    (dir_start_sector & 0xFF) as u8,                // Offset 16: First root directory sector (low)
                    ((dir_start_sector >> 8) & 0xFF) as u8,         // Offset 17: First root directory sector (high)
                ], media_type)
            } else {
                // Boot sector too small or invalid, use defaults based on media type
                tracing::warn!("Invalid boot sector, using defaults");
                match Self::get_default_dpb(requested_media_type) {
                    Ok(dpb) => (dpb, requested_media_type),
                    Err(_) => {
                        state.set_carry_flag(true);
                        return false;
                    }
                }
            }
        } else {
            // No disk or read error, use defaults
            tracing::warn!("Cannot read boot sector, using defaults");
            match Self::get_default_dpb(requested_media_type) {
                Ok(dpb) => (dpb, requested_media_type),
                Err(_) => {
                    state.set_carry_flag(true);
                    return false;
                }
            }
        };

        // Write DPB to memory
        let mut addr = dpb_address.wrapping_add(1); // DPB starts at HL+1!
        let mut bus = self.bus.borrow_mut();
        tracing::debug!("Writing DPB to address 0x{:04X} (HL+1):", addr);
        for (i, byte) in dpb_data.iter().enumerate() {
            bus.write_byte(addr, *byte);
            if i < 20 {
                // Log first 20 bytes
                tracing::debug!("  DPB[{:02}] @ 0x{:04X} = 0x{:02X}", i, addr, byte);
            }
            addr = addr.wrapping_add(1);
        }
        drop(bus);

        // Also log what we expect the directory sector to be
        let dir_sector = u16::from_le_bytes([dpb_data[16], dpb_data[17]]);
        tracing::info!(
            "DPB written: media=0x{:02X}, dir_sector={}",
            media_type,
            dir_sector
        );

        // Set output registers according to MSX-DOS specification
        // A = Media descriptor
        // BC = 16-bit logical sector of first root directory sector (B=high, C=low)
        // DE = Number of free clusters (0xFFFF for MSX-DOS 1)
        // HL = Points to DPB (unchanged from input)
        // Carry flag = reset for success

        let num_free_clusters = 0xFFFF; // MSX-DOS 1 doesn't report free clusters, use 0xFFFF

        state.a = media_type;
        
        // BC must contain the directory start sector with proper endianness
        // B = high byte, C = low byte
        state.set_b(dpb_data[17]); // High byte of directory start sector
        state.set_c(dpb_data[16]); // Low byte of directory start sector
        
        state.de = num_free_clusters;
        // state.hl is already dpb_address (input parameter) and should be preserved.
        
        // IMPORTANT: Ensure carry flag is clear for success
        // Some MSX software depends on exact flag state
        state.set_carry_flag(false);

        tracing::info!(
            "GETDPB: Success. A=0x{:02X} (media), BC=0x{:04X} (dir_sector), DE=0x{:04X} (free_clusters)",
            state.a, state.bc, state.de
        );
        true
    }

    fn dskfmt(&mut self, state: &mut CpuExtensionState) -> bool {
        // Phase 1: Not implemented (read-only support)
        tracing::debug!("DSKFMT: Not implemented in read-only mode");
        state.set_carry_flag(true); // Set carry (error)
        state.a = 0x00; // Write protect error
        false
    }

    fn drives(&mut self, state: &mut CpuExtensionState) -> bool {
        // Return number of drives
        // L = number of drives (1 or 2)
        let drive_count = if self.disk_drive.lock().unwrap().has_disk(1) {
            2
        } else {
            1
        };
        state.hl = (state.hl & 0xFF00) | drive_count as u16;
        tracing::debug!("DRIVES: Returning {} drive(s)", drive_count);
        true
    }

    // INIENV is now handled by extension 0xE0 (same as INIHRD)

    fn mtoff(&mut self, _state: &mut CpuExtensionState) -> bool {
        // Motor off - schedule motor off for all drives
        tracing::debug!("MTOFF: Scheduling motor off");
        self.motor_off_counter = 2300; // ~2.3 seconds at 1000Hz
        true
    }

    fn choice(&mut self, state: &mut CpuExtensionState) -> bool {
        // CHOICE - Return choice string address for disk format selection
        tracing::debug!("CHOICE: Called from PC=0x{:04X}", state.pc);

        // Look up the choice string address based on where CHOICE was called from
        // The PC should point to the address where the CHOICE routine was patched
        let choice_str_addr = unsafe {
            crate::disk_rom_manager::CHOICE_STRING_ADDRESSES
                .get(&(state.pc as usize))
                .copied()
                .unwrap_or(0)
        };

        state.hl = choice_str_addr as u16;

        tracing::debug!("CHOICE: Returning string address 0x{:04X}", state.hl);

        true
    }

    fn inihrd(&mut self, state: &mut CpuExtensionState) -> bool {
        // INIHRD - Initialize hardware
        tracing::info!(
            "INIHRD: Initializing disk hardware at PC=0x{:04X}",
            state.pc
        );

        // Initialize hardware (nothing special needed for our emulation)
        // Clear carry flag to indicate success
        state.set_carry_flag(false);

        // We no longer clear the disk changed flag here.
        // It should only be cleared when the change is acknowledged by DSKCHG.

        // Return with HL pointing to work area (some DOS versions expect this)
        state.hl = 0xC000; // Safe work area in RAM

        true
    }

    fn inihma(&mut self, state: &mut CpuExtensionState) -> bool {
        // INIHMA - Initialize heap management
        tracing::info!("INIHMA: Initializing heap management");

        // Set up heap management area
        // HL should point to the heap area
        // For now, just return with a safe default
        state.hl = 0xF380; // Point to safe work area

        true
    }

    fn dskstp(&mut self, _state: &mut CpuExtensionState) -> bool {
        // DSKSTP - Stop disk motor
        tracing::debug!("DSKSTP: Stopping disk motor");

        // Stop all motors immediately
        if let Ok(mut drive) = self.disk_drive.lock() {
            drive.all_motors_off();
        }

        true
    }
}

impl CpuExtensionHandler for DiskDriver {
    fn extension_begin(&mut self, state: &mut CpuExtensionState) -> bool {
        // Log all extension calls to help debug FILES
        if state.ext_num != 0xE4 { // Don't log DSKIO (too verbose)
            tracing::info!("Extension 0x{:02X} called from PC=0x{:04X}, HL=0x{:04X}", 
                state.ext_num, state.pc, state.hl);
        }
        
        let success = match state.ext_num {
            0xE0 => {
                // Extension E0 is used for both INIHRD and INIENV
                // We'll handle both the same way for now
                self.inihrd(state)
            }
            0xE2 => self.drives(state), // DRIVES
            0xE4 => self.dskio(state),  // DSKIO
            0xE5 => self.dskchg(state), // DSKCHG
            0xE6 => self.getdpb(state), // GETDPB
            0xE7 => self.choice(state), // CHOICE
            0xE8 => self.dskfmt(state), // DSKFMT
            0xE9 => self.dskstp(state), // DSKSTP
            0xEA => self.mtoff(state),  // MTOFF
            _ => {
                tracing::warn!("Unknown disk extension: 0x{:02X}", state.ext_num);
                // Set carry flag to indicate error for unknown extensions
                state.set_carry_flag(true);
                false
            }
        };

        success
    }

    fn extension_finish(&mut self, _state: &mut CpuExtensionState) -> bool {
        // Handle motor off counter
        if self.motor_off_counter > 0 {
            self.motor_off_counter -= 1;
            if self.motor_off_counter == 0 {
                // Turn off all motors
                if let Ok(mut drive) = self.disk_drive.lock() {
                    drive.all_motors_off();
                    tracing::debug!("Motors turned off");
                }
            }
        }
        false
    }
}
