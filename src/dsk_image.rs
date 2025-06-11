// DSK Image handler for MSX disk images
// Supports standard 360KB and 720KB formats

use crate::disk_error::DiskError;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const SECTOR_SIZE: usize = 512;

#[derive(Debug, Clone)]
pub struct DiskImage {
    data: Vec<u8>,
    media_type: u8,
    sectors_per_track: u16,
    total_sectors: u16,
    tracks: u16,
    sides: u8,
}

impl DiskImage {
    pub fn new_empty(media_type: u8) -> Result<Self, DiskError> {
        let (total_sectors, sectors_per_track, tracks, sides) = match media_type {
            0xF8 => (720, 9, 80, 1),  // 360KB: 80 tracks, 9 sectors/track, 1 side
            0xF9 => (1440, 9, 80, 2), // 720KB: 80 tracks, 9 sectors/track, 2 sides
            _ => {
                return Err(DiskError::FormatError(format!(
                    "Unsupported media type: 0x{:02X}",
                    media_type
                )))
            }
        };

        let mut data = vec![0; total_sectors as usize * SECTOR_SIZE];

        // Format the disk properly
        Self::format_disk_data(&mut data, media_type, total_sectors)?;

        Ok(Self {
            data,
            media_type,
            sectors_per_track,
            total_sectors,
            tracks,
            sides,
        })
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, DiskError> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        Self::from_bytes(data)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self, DiskError> {
        let (media_type, sectors_per_track, total_sectors, tracks, sides) = match data.len() {
            368640 => (0xF8, 9, 720, 80, 1),  // 360KB
            737280 => (0xF9, 9, 1440, 80, 2), // 720KB
            _ => {
                return Err(DiskError::InvalidSize(format!(
                    "Invalid disk image size: {} bytes (expected 368640 or 737280)",
                    data.len()
                )))
            }
        };

        // Validate disk by checking boot sector if present
        if data.len() >= SECTOR_SIZE {
            // Check for valid boot sector marker at offset 0x1FE-0x1FF
            if data.len() >= 0x200 && (data[0x1FE] != 0x55 || data[0x1FF] != 0xAA) {
                tracing::warn!("Disk image does not have valid boot sector signature");
            }

            // Check media descriptor at offset 0x15 (should match media_type)
            if data[0x15] != media_type {
                tracing::warn!(
                    "Media descriptor mismatch: expected 0x{:02X}, found 0x{:02X}",
                    media_type,
                    data[0x15]
                );
            }

            // Check directory sectors (5-8 for 360KB, 7-10 for 720KB)
            let dir_start = if media_type == 0xF8 { 5 } else { 7 };
            let dir_sectors = 3; // 112 entries * 32 bytes = 3584 bytes = 7 sectors, but usually only 3-4 used

            let mut has_valid_entry = false;
            for sector in dir_start..dir_start + dir_sectors {
                let sector_offset = sector as usize * SECTOR_SIZE;
                if sector_offset + SECTOR_SIZE <= data.len() {
                    // Check each 32-byte directory entry in the sector
                    for entry in 0..16 {
                        // 512 / 32 = 16 entries per sector
                        let entry_offset = sector_offset + entry * 32;
                        if entry_offset + 32 <= data.len() {
                            let first_byte = data[entry_offset];
                            // Valid entries start with 0x00-0x7F or 0xE5 (deleted)
                            // 0x00 means end of directory
                            if first_byte != 0x00 && first_byte != 0xE5 && first_byte != 0xFF {
                                has_valid_entry = true;
                                let filename: String = data[entry_offset..entry_offset + 11]
                                    .iter()
                                    .map(|&b| {
                                        if b >= 0x20 && b <= 0x7E {
                                            b as char
                                        } else {
                                            '.'
                                        }
                                    })
                                    .collect();
                                tracing::debug!("Found directory entry: '{}'", filename.trim());
                            }
                        }
                    }
                }
            }

            if !has_valid_entry {
                tracing::warn!("No valid directory entries found in disk image");
            }
        }

        Ok(Self {
            data,
            media_type,
            sectors_per_track,
            total_sectors,
            tracks,
            sides,
        })
    }

    pub fn read_sector(&self, sector: u16) -> Result<&[u8], DiskError> {
        if sector >= self.total_sectors {
            return Err(DiskError::InvalidSector);
        }

        let start = sector as usize * SECTOR_SIZE;
        let end = start + SECTOR_SIZE;

        Ok(&self.data[start..end])
    }

    pub fn read_sectors(&self, start_sector: u16, count: u8) -> Result<Vec<u8>, DiskError> {
        if start_sector as u32 + count as u32 > self.total_sectors as u32 {
            return Err(DiskError::InvalidSector);
        }

        let mut result = Vec::with_capacity(count as usize * SECTOR_SIZE);

        for i in 0..count {
            let logical_sector = start_sector + i as u16;

            // For single-sided disks (360KB), track calculation is straightforward
            // For double-sided disks (720KB), we need to account for both sides
            let track = if self.sides == 1 {
                logical_sector / self.sectors_per_track
            } else {
                logical_sector / (self.sectors_per_track * self.sides as u16)
            };

            let sectors_per_cylinder = self.sectors_per_track * self.sides as u16;
            let logical_in_cylinder = logical_sector % sectors_per_cylinder;
            let side = logical_in_cylinder / self.sectors_per_track;
            let sector_on_track = logical_in_cylinder % self.sectors_per_track;

            // Calculate the flat offset in the .dsk file
            // .dsk files store sectors sequentially without interleave
            let flat_offset = if self.sides == 1 {
                ((track * self.sectors_per_track) + sector_on_track) as usize * SECTOR_SIZE
            } else {
                ((track * self.sides as u16 * self.sectors_per_track)
                    + (side * self.sectors_per_track)
                    + sector_on_track) as usize
                    * SECTOR_SIZE
            };

            let end_byte = flat_offset + SECTOR_SIZE;
            if end_byte > self.data.len() {
                return Err(DiskError::ReadError);
            }

            result.extend_from_slice(&self.data[flat_offset..end_byte]);

            // Enhanced trace logging
            if logical_sector <= 11 {
                // Only log for boot/FAT/dir sectors
                tracing::info!(
                    "Sector Read: Logical: {}, Track: {}, Side: {}, Sector on Track: {}, Flat Offset: 0x{:X}",
                    logical_sector, track, side, sector_on_track, flat_offset
                );
                
                // Also log what we're actually reading for debugging
                let preview_len = 32.min(SECTOR_SIZE);
                tracing::info!(
                    "  Reading from offset 0x{:X}, first {} bytes: {:02X?}",
                    flat_offset,
                    preview_len,
                    &self.data[flat_offset..flat_offset + preview_len]
                );
            }
        }

        // Log first few bytes when reading important sectors
        if start_sector == 0 || (start_sector >= 5 && start_sector <= 8) {
            let sector_name = if start_sector == 0 {
                "Boot"
            } else {
                "Directory"
            };
            tracing::info!(
                "{} sector {} (count {}): first 16 bytes = {:02X?}",
                sector_name,
                start_sector,
                count,
                &result[..16.min(result.len())]
            );
        }

        Ok(result)
    }

    pub fn write_sector(&mut self, sector: u16, data: &[u8]) -> Result<(), DiskError> {
        if sector >= self.total_sectors {
            return Err(DiskError::InvalidSector);
        }

        if data.len() != SECTOR_SIZE {
            return Err(DiskError::WriteError);
        }

        let start = sector as usize * SECTOR_SIZE;
        self.data[start..start + SECTOR_SIZE].copy_from_slice(data);

        Ok(())
    }

    pub fn write_sectors(&mut self, start_sector: u16, data: &[u8]) -> Result<(), DiskError> {
        let sector_count = data.len() / SECTOR_SIZE;

        if data.len() % SECTOR_SIZE != 0 {
            return Err(DiskError::WriteError);
        }

        if start_sector as usize + sector_count > self.total_sectors as usize {
            return Err(DiskError::InvalidSector);
        }

        // Write each sector individually
        for i in 0..sector_count {
            let logical_sector = start_sector + i as u16;

            // Calculate track, side, and sector on track
            let track = if self.sides == 1 {
                logical_sector / self.sectors_per_track
            } else {
                logical_sector / (self.sectors_per_track * self.sides as u16)
            };

            let sectors_per_cylinder = self.sectors_per_track * self.sides as u16;
            let logical_in_cylinder = logical_sector % sectors_per_cylinder;
            let side = logical_in_cylinder / self.sectors_per_track;
            let sector_on_track = logical_in_cylinder % self.sectors_per_track;

            // Calculate the flat offset in the .dsk file
            // .dsk files store sectors sequentially without interleave
            let flat_offset = if self.sides == 1 {
                ((track * self.sectors_per_track) + sector_on_track) as usize * SECTOR_SIZE
            } else {
                ((track * self.sides as u16 * self.sectors_per_track)
                    + (side * self.sectors_per_track)
                    + sector_on_track) as usize
                    * SECTOR_SIZE
            };

            let data_offset = i * SECTOR_SIZE;

            self.data[flat_offset..flat_offset + SECTOR_SIZE]
                .copy_from_slice(&data[data_offset..data_offset + SECTOR_SIZE]);
        }

        Ok(())
    }

    pub fn get_media_type(&self) -> u8 {
        self.media_type
    }

    pub fn get_total_sectors(&self) -> u16 {
        self.total_sectors
    }

    pub fn get_sectors_per_track(&self) -> u16 {
        self.sectors_per_track
    }

    pub fn get_tracks(&self) -> u16 {
        self.tracks
    }

    pub fn get_sides(&self) -> u8 {
        self.sides
    }

    /// Format disk data with proper FAT12 structure
    fn format_disk_data(
        data: &mut [u8],
        media_type: u8,
        total_sectors: u16,
    ) -> Result<(), DiskError> {
        // Boot sector parameters for MSX-DOS - must match DPB!
        let bytes_per_sector: u16 = 512;
        let sectors_per_cluster: u8 = 2;
        let reserved_sectors: u16 = 1;
        let num_fats: u8 = 2;
        let root_entries: u16 = 112;
        let (sectors_per_fat, first_dir_sector): (u16, u16) = match media_type {
            0xF8 => (2, 5), // 360KB: FAT size=2, dir starts at sector 5
            0xF9 => (3, 7), // 720KB: FAT size=3, dir starts at sector 7
            _ => return Err(DiskError::FormatError("Unsupported media type".to_string())),
        };

        // Write boot sector
        // JMP instruction
        data[0] = 0xEB;
        data[1] = 0xFE;
        data[2] = 0x90;

        // OEM name "MSX     "
        data[3..11].copy_from_slice(b"MSX     ");

        // BPB (BIOS Parameter Block)
        data[11..13].copy_from_slice(&bytes_per_sector.to_le_bytes());
        data[13] = sectors_per_cluster;
        data[14..16].copy_from_slice(&reserved_sectors.to_le_bytes());
        data[16] = num_fats;
        data[17..19].copy_from_slice(&root_entries.to_le_bytes());
        data[19..21].copy_from_slice(&total_sectors.to_le_bytes());
        data[21] = media_type;
        data[22..24].copy_from_slice(&sectors_per_fat.to_le_bytes());

        // Boot signature
        data[510] = 0x55;
        data[511] = 0xAA;

        // Initialize FAT
        let fat_start = reserved_sectors as usize * bytes_per_sector as usize;
        data[fat_start] = media_type;
        data[fat_start + 1] = 0xFF;
        data[fat_start + 2] = 0xFF;

        // Copy FAT to second FAT
        let fat2_start = fat_start + (sectors_per_fat as usize * bytes_per_sector as usize);
        for i in 0..(sectors_per_fat as usize * bytes_per_sector as usize) {
            data[fat2_start + i] = data[fat_start + i];
        }

        // Initialize data area with 0xFF (important!)
        // Calculate based on actual MSX-DOS layout, not theoretical layout
        let data_start_sector = match media_type {
            0xF8 => 7,  // 360KB: data starts at sector 7
            0xF9 => 14, // 720KB: data starts at sector 14
            _ => 7,
        };
        let data_start = data_start_sector * bytes_per_sector as usize;

        // Fill data area with 0xFF
        for i in data_start..data.len() {
            data[i] = 0xFF;
        }

        tracing::debug!(
            "Formatted disk: FAT at {}, dir at sector {}, data at {}, filled {} bytes with 0xFF",
            fat_start,
            first_dir_sector,
            data_start,
            data.len() - data_start
        );

        Ok(())
    }

    // Convert logical sector to physical CHS (Cylinder-Head-Sector)
    pub fn logical_to_chs(&self, logical_sector: u16) -> (u8, u8, u8) {
        let track = logical_sector / (self.sectors_per_track * self.sides as u16);
        let temp = logical_sector % (self.sectors_per_track * self.sides as u16);
        let head = temp / self.sectors_per_track;
        let sector = (temp % self.sectors_per_track) + 1; // Sectors are 1-based

        (track as u8, head as u8, sector as u8)
    }

    // Convert physical CHS to logical sector
    pub fn chs_to_logical(&self, cylinder: u8, head: u8, sector: u8) -> u16 {
        let logical = (cylinder as u16 * self.sides as u16 * self.sectors_per_track)
            + (head as u16 * self.sectors_per_track)
            + (sector as u16 - 1); // Convert from 1-based to 0-based
        logical
    }
}
