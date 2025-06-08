// Disk Driver - implements MSX-DOS BIOS functions via CPU extensions

use crate::bus::Bus;
use crate::cpu_extensions::{CpuExtensionHandler, CpuExtensionState};
use crate::disk_drive::DiskDrive;
use crate::disk_error::DiskError;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// Static debugging flags for tracking FILES command issue
static mut AFTER_GETDPB: bool = false;
static mut LAST_GETDPB_HL: u16 = 0;

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
    files_command_dir_sector: Option<u16>,
}

impl DiskDriver {
    pub fn new(disk_drive: Arc<Mutex<DiskDrive>>, bus: Rc<RefCell<Bus>>) -> Self {
        Self {
            disk_drive,
            motor_off_counter: 0,
            bus,
            files_command_dir_sector: None,
        }
    }

    fn dskio(&mut self, state: &mut CpuExtensionState) -> bool {
        let drive_num = state.a & 0x01;
        let sector_count = state.b();
        let original_sector = state.de;
        let mut memory_address = state.hl; // Make mutable
        let is_write = state.carry_flag();

        let mut logical_sector = original_sector;

        let is_files_bug = !is_write && (original_sector >= 1794 && original_sector <= 1810);

        if is_files_bug {
            // The FILES routine expects the directory data to be read into the default
            // Disk Transfer Area (DTA) at 0x0080, not the address in HL
            memory_address = 0x0080;
            
            // Initialize our sector tracker on first detection
            if self.files_command_dir_sector.is_none() {
                tracing::info!("FILES bug detected. Initializing workaround...");
                
                // Get the correct directory start sector based on media type
                let drive = self.disk_drive.lock().unwrap();
                if let Some((media_type, _, _, _, _)) = drive.get_disk_info(drive_num) {
                    let dir_start = match media_type {
                        0xF8 => 5,  // 360KB: directory starts at sector 5
                        0xF9 => 7,  // 720KB: directory starts at sector 7
                        _ => 5,     // Default to 360KB layout
                    };
                    tracing::info!("   -> Starting directory scan at sector {}", dir_start);
                    self.files_command_dir_sector = Some(dir_start);
                } else {
                    // No disk
                    state.set_carry_flag(true);
                    state.a = 0x02; // Not ready
                    state.set_b(sector_count);
                    return false;
                }
            }

            // Use our tracked sector number instead of the bogus one
            if let Some(sector_to_read) = self.files_command_dir_sector {
                logical_sector = sector_to_read;
                tracing::info!("FILES workaround: Overriding sector request from {} to {}", original_sector, logical_sector);

                // Increment our tracker for the next call
                self.files_command_dir_sector = Some(sector_to_read + sector_count as u16);
            }
        } else {
            // A normal DSKIO call means the FILES operation is over. Reset the tracker
            if self.files_command_dir_sector.is_some() {
                tracing::debug!("Normal DSKIO call, resetting FILES workaround state");
                self.files_command_dir_sector = None;
            }
        }

        // --- The rest of the function now uses the corrected logical_sector and memory_address ---

        tracing::info!(
        "DSKIO: drive={}, sectors={}, logical_sector={}, address=0x{:04X}, write={}, caller_PC=0x{:04X}",
        drive_num, sector_count, logical_sector, memory_address, is_write, state.pc
    );

        if is_write {
            state.set_carry_flag(true);
            state.a = 0x00; // Write protect error
            state.set_b(sector_count);
            return false;
        }

        if let Ok(mut drive) = self.disk_drive.lock() {
            match drive.read_sectors(drive_num, logical_sector, sector_count) {
                Ok(data) => {
                    self.bus.borrow_mut().write_block(memory_address, &data);

                    let media_type = drive.get_disk_info(drive_num).map_or(0xF8, |d| d.0);
                    state.set_carry_flag(false);
                    state.a = media_type;
                    state.set_b(0);
                    true
                }
                Err(err) => {
                    tracing::warn!("DSKIO read error: {:?}", err);
                    state.set_carry_flag(true);
                    state.a = match err {
                        DiskError::NoDisk => 0x02,
                        DiskError::InvalidSector => 0x08,
                        _ => 0x0C,
                    };
                    state.set_b(sector_count);
                    false
                }
            }
        } else {
            state.set_carry_flag(true);
            state.a = 0x0C;
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

        // For BASIC FILES command, always report disk not changed to avoid issues
        // The HL=0xF197 pattern seems to be used by FILES
        // let force_not_changed = state.hl == 0xF197;

        // Check disk state and get media descriptor if needed
        let (disk_state, media_desc_opt) = if let Ok(mut drive) = self.disk_drive.lock() {
            let changed = drive.disk_changed(drive_num);

            // If disk changed, read media descriptor
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
                // Disk not changed
                state.set_carry_flag(false); // Clear carry
                state.set_b(0x00); // B=0, disk not changed
                tracing::debug!("DSKCHG: Disk not changed");
                true
            }
            Some(true) => {
                // Disk changed
                state.set_carry_flag(false); // Clear carry
                state.set_b(0xFF); // B=FF, disk changed
                tracing::debug!("DSKCHG: Disk changed!");

                // For DOS1, we should automatically update the DPB when disk changed
                if let Some(media_desc) = media_desc_opt {
                    tracing::debug!(
                        "DSKCHG: Auto-updating DPB with media type 0x{:02X}",
                        media_desc
                    );

                    // For DOS1 compatibility, write DPB directly like WebMSX does
                    // This matches WebMSX line 208: if (!dos2) GETDPB(F, A, mediaDeskFromDisk, C, HL, true);

                    // Create a temporary state for GETDPB call
                    let mut dpb_state = state.clone();
                    dpb_state.set_b(media_desc); // Media type in B
                    dpb_state.set_c(media_desc); // Also in C for safety
                                                 // HL already contains the DPB address from DSKCHG call

                    self.getdpb(&mut dpb_state);

                    // Don't update main state's registers from the GETDPB call
                    // Just keep the disk changed status
                }

                true
            }
            None => {
                // No disk
                state.set_carry_flag(true); // Set carry
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

        // Get the actual media type from the disk
        let actual_media_type = if let Ok(drive_guard) = self.disk_drive.lock() {
            if let Some(info) = drive_guard.get_disk_info(0) {
                info.0 // media_type
            } else {
                requested_media_type
            }
        } else {
            requested_media_type
        };

        tracing::debug!(
            "GETDPB: requested_type=0x{:02X}, actual_type=0x{:02X}, dpb_address=0x{:04X}",
            requested_media_type,
            actual_media_type,
            dpb_address
        );

        let media_type = actual_media_type;

        // Get DPB based on media type
        let dpb_data = match media_type {
            0xF8 => {
                // 360KB single-sided, 9 sectors/track
                vec![
                    0xF8, // Media descriptor
                    0x00, 0x02, // Sector size (512)
                    0x70, // Directory mask
                    0x04, // Directory shift
                    0x01, // Cluster mask
                    0x01, // Cluster shift
                    0x01, 0x00, // First FAT sector
                    0x02, // Number of FAT copies
                    0x70, 0x00, // Directory entries (112)
                    0x05, 0x00, // First directory sector
                    0x62, 0x01, // Number of clusters (354)
                    0x02, // Sectors per FAT
                    0x07, 0x00, // First data sector
                ]
            }
            0xF9 => {
                // 720KB double-sided, 9 sectors/track
                vec![
                    0xF9, // Media descriptor
                    0x00, 0x02, // Sector size (512)
                    0x70, // Directory mask
                    0x04, // Directory shift
                    0x01, // Cluster mask
                    0x01, // Cluster shift
                    0x01, 0x00, // First FAT sector
                    0x02, // Number of FAT copies
                    0x70, 0x00, // Directory entries (112)
                    0x07, 0x00, // First directory sector
                    0xC9, 0x02, // Number of clusters (713)
                    0x03, // Sectors per FAT
                    0x0E, 0x00, // First data sector
                ]
            }
            _ => {
                tracing::warn!("GETDPB: Unsupported media type 0x{:02X}", media_type);
                state.set_carry_flag(true); // Error
                return false;
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
        let dir_sector = u16::from_le_bytes([dpb_data[12], dpb_data[13]]);
        tracing::info!(
            "DPB written: media=0x{:02X}, dir_sector={}",
            media_type,
            dir_sector
        );

        // Set flag for DSKIO debugging
        unsafe {
            AFTER_GETDPB = true;
            LAST_GETDPB_HL = dpb_address;
        }

        state.set_carry_flag(false); // Success
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

        // Clear disk changed flags for all drives
        if let Ok(mut drive) = self.disk_drive.lock() {
            drive.clear_disk_changed(0);
            drive.clear_disk_changed(1);
        }

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
