// Disk Drive emulation
// Manages virtual floppy drives A: and B:

use crate::disk_error::DiskError;
use crate::dsk_image::DiskImage;
use std::sync::{Arc, Mutex};

pub struct DiskDrive {
    drives: [Option<DiskImage>; 2],  // A: and B:
    disk_changed: [Option<bool>; 2],
    motor_on: [bool; 2],
    motor_off_time: [Option<std::time::Instant>; 2],
}

impl DiskDrive {
    pub fn new() -> Self {
        Self {
            drives: [None, None],
            disk_changed: [None, None],
            motor_on: [false, false],
            motor_off_time: [None, None],
        }
    }

    pub fn insert_disk(&mut self, drive: u8, image: DiskImage) -> Result<(), DiskError> {
        if drive >= 2 {
            return Err(DiskError::InvalidDrive);
        }
        
        self.drives[drive as usize] = Some(image);
        self.disk_changed[drive as usize] = Some(true);
        tracing::info!("Disk inserted in drive {}", if drive == 0 { "A:" } else { "B:" });
        
        Ok(())
    }

    pub fn eject_disk(&mut self, drive: u8) -> Result<(), DiskError> {
        if drive >= 2 {
            return Err(DiskError::InvalidDrive);
        }
        
        self.drives[drive as usize] = None;
        self.disk_changed[drive as usize] = None;
        self.motor_on[drive as usize] = false;
        tracing::info!("Disk ejected from drive {}", if drive == 0 { "A:" } else { "B:" });
        
        Ok(())
    }
    
    /// Insert a new formatted disk
    pub fn insert_new_disk(&mut self, drive: u8, media_type: u8) -> Result<(), DiskError> {
        if drive >= 2 {
            return Err(DiskError::InvalidDrive);
        }
        
        let disk = DiskImage::new_empty(media_type)?;
        self.insert_disk(drive, disk)?;
        
        tracing::info!(
            "Inserted new formatted {} disk in drive {}", 
            if media_type == 0xF8 { "360KB" } else { "720KB" },
            if drive == 0 { "A:" } else { "B:" }
        );
        
        Ok(())
    }

    pub fn disk_changed(&mut self, drive: u8) -> Option<bool> {
        if drive >= 2 {
            return None;
        }
        
        let changed = self.disk_changed[drive as usize];
        // Clear the changed flag after reading
        if changed == Some(true) {
            self.disk_changed[drive as usize] = Some(false);
        }
        changed
    }
    
    pub fn clear_disk_changed(&mut self, drive: u8) {
        if drive < 2 && self.has_disk(drive) {
            self.disk_changed[drive as usize] = Some(false);
        }
    }

    pub fn read_sectors(&mut self, drive: u8, start_sector: u16, count: u8) -> Result<Vec<u8>, DiskError> {
        if drive >= 2 {
            return Err(DiskError::InvalidDrive);
        }
        
        // Turn on motor
        self.motor_on[drive as usize] = true;
        self.motor_off_time[drive as usize] = None;
        
        if let Some(disk) = &self.drives[drive as usize] {
            tracing::debug!(
                "Reading {} sectors from drive {} starting at sector {}",
                count,
                if drive == 0 { "A:" } else { "B:" },
                start_sector
            );
            disk.read_sectors(start_sector, count)
        } else {
            Err(DiskError::NoDisk)
        }
    }

    pub fn write_sectors(&mut self, drive: u8, start_sector: u16, data: &[u8]) -> Result<(), DiskError> {
        if drive >= 2 {
            return Err(DiskError::InvalidDrive);
        }
        
        // Turn on motor
        self.motor_on[drive as usize] = true;
        self.motor_off_time[drive as usize] = None;
        
        if let Some(disk) = &mut self.drives[drive as usize] {
            tracing::debug!(
                "Writing {} bytes to drive {} starting at sector {}",
                data.len(),
                if drive == 0 { "A:" } else { "B:" },
                start_sector
            );
            disk.write_sectors(start_sector, data)
        } else {
            Err(DiskError::NoDisk)
        }
    }

    pub fn motor_off(&mut self, drive: u8) {
        if drive < 2 {
            self.motor_on[drive as usize] = false;
            self.motor_off_time[drive as usize] = Some(std::time::Instant::now());
        }
    }

    pub fn all_motors_off(&mut self) {
        self.motor_off(0);
        self.motor_off(1);
    }

    pub fn is_motor_on(&self, drive: u8) -> bool {
        if drive < 2 {
            self.motor_on[drive as usize]
        } else {
            false
        }
    }

    pub fn has_disk(&self, drive: u8) -> bool {
        if drive < 2 {
            self.drives[drive as usize].is_some()
        } else {
            false
        }
    }

    pub fn get_disk_info(&self, drive: u8) -> Option<(u8, u16, u16, u16, u8)> {
        if drive < 2 {
            self.drives[drive as usize].as_ref().map(|disk| {
                (
                    disk.get_media_type(),
                    disk.get_total_sectors(),
                    disk.get_sectors_per_track(),
                    disk.get_tracks(),
                    disk.get_sides(),
                )
            })
        } else {
            None
        }
    }

    // Get Disk Parameter Block info for MSX-DOS
    pub fn get_dpb(&self, drive: u8) -> Option<DiskParameterBlock> {
        if drive >= 2 {
            return None;
        }

        self.drives[drive as usize].as_ref().map(|disk| {
            match disk.get_media_type() {
                0xF8 => {
                    // 360KB disk parameters
                    DiskParameterBlock {
                        media_type: 0xF8,
                        sector_size: 512,
                        dir_mask: 0x70,         // Directory mask
                        dir_shift: 4,           // Directory shift
                        cluster_mask: 0x01,     // Cluster mask  
                        cluster_shift: 1,       // Cluster shift (2 sectors per cluster)
                        fat_start: 1,           // First FAT sector
                        fat_copies: 2,          // Number of FAT copies
                        dir_entries: 112,       // Root directory entries
                        data_start: 7,          // First data sector
                        clusters: 354,          // Total clusters
                        fat_size: 2,            // Sectors per FAT
                        dir_start: 5,           // First directory sector
                    }
                }
                0xF9 => {
                    // 720KB disk parameters
                    DiskParameterBlock {
                        media_type: 0xF9,
                        sector_size: 512,
                        dir_mask: 0x70,         // Directory mask
                        dir_shift: 4,           // Directory shift
                        cluster_mask: 0x01,     // Cluster mask
                        cluster_shift: 1,       // Cluster shift (2 sectors per cluster)
                        fat_start: 1,           // First FAT sector
                        fat_copies: 2,          // Number of FAT copies
                        dir_entries: 112,       // Root directory entries
                        data_start: 10,         // First data sector
                        clusters: 713,          // Total clusters
                        fat_size: 3,            // Sectors per FAT
                        dir_start: 7,           // First directory sector
                    }
                }
                _ => {
                    // Default to 720KB parameters
                    DiskParameterBlock {
                        media_type: disk.get_media_type(),
                        sector_size: 512,
                        dir_mask: 0x70,
                        dir_shift: 4,
                        cluster_mask: 0x01,
                        cluster_shift: 1,
                        fat_start: 1,
                        fat_copies: 2,
                        dir_entries: 112,
                        data_start: 10,
                        clusters: 713,
                        fat_size: 3,
                        dir_start: 7,
                    }
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct DiskParameterBlock {
    pub media_type: u8,
    pub sector_size: u16,
    pub dir_mask: u8,
    pub dir_shift: u8,
    pub cluster_mask: u8,
    pub cluster_shift: u8,
    pub fat_start: u16,
    pub fat_copies: u8,
    pub dir_entries: u16,
    pub data_start: u16,
    pub clusters: u16,
    pub fat_size: u8,
    pub dir_start: u16,
}

// Thread-safe wrapper for DiskDrive
#[derive(Clone)]
pub struct SharedDiskDrive(Arc<Mutex<DiskDrive>>);

impl SharedDiskDrive {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(DiskDrive::new())))
    }

    pub fn clone_inner(&self) -> Arc<Mutex<DiskDrive>> {
        Arc::clone(&self.0)
    }
}