use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FdcStatus {
    Idle,
    Busy,
    Read,
    Write,
    Seek,
}

#[derive(Debug)]
pub struct WD2793 {
    // Registers
    status_register: u8,
    command_register: u8,
    track_register: u8,
    sector_register: u8,
    data_register: u8,
    
    // Internal state
    current_drive: u8,
    side: u8,
    motor_on: bool,
    
    // Operation state
    state: FdcStatus,
    data_buffer: Vec<u8>,
    buffer_pos: usize,
    
    // Disk images
    drives: [Option<DiskImage>; 2],
    
    // Status flags
    busy: bool,
    drq: bool,  // Data Request
    index_pulse: bool,
    track_zero: bool,
    crc_error: bool,
    seek_error: bool,
    lost_data: bool,
    write_protect: bool,
}

#[derive(Debug, Clone)]
pub struct DiskImage {
    data: Vec<u8>,
    format: DiskFormat,
    write_protected: bool,
    tracks_per_side: u8,
    sectors_per_track: u8,
    sides: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiskFormat {
    DSK,  // Standard MSX DSK format (720KB)
    DI,   // DiskImage format
    DMK,  // David M. Keil's format
}

impl WD2793 {
    pub fn new() -> Self {
        Self {
            status_register: 0,
            command_register: 0,
            track_register: 0,
            sector_register: 1,  // Sectors start at 1
            data_register: 0,
            
            current_drive: 0,
            side: 0,
            motor_on: false,
            
            state: FdcStatus::Idle,
            data_buffer: Vec::new(),
            buffer_pos: 0,
            
            drives: [None, None],
            
            busy: false,
            drq: false,
            index_pulse: false,
            track_zero: true,  // Initially at track 0
            crc_error: false,
            seek_error: false,
            lost_data: false,
            write_protect: false,
        }
    }
    
    pub fn reset(&mut self) {
        self.status_register = 0;
        self.command_register = 0;
        self.track_register = 0;
        self.sector_register = 1;
        self.data_register = 0;
        self.state = FdcStatus::Idle;
        self.busy = false;
        self.drq = false;
        self.motor_on = false;
        self.update_status();
    }
    
    pub fn read(&mut self, port: u8) -> u8 {
        match port & 0x03 {
            0 => {
                // Status register
                self.update_status();
                self.status_register
            }
            1 => self.track_register,
            2 => self.sector_register,
            3 => {
                // Data register
                if self.state == FdcStatus::Read && self.drq {
                    let data = self.data_buffer.get(self.buffer_pos).copied().unwrap_or(0);
                    self.buffer_pos += 1;
                    
                    if self.buffer_pos >= self.data_buffer.len() {
                        self.complete_read();
                    }
                    
                    data
                } else {
                    self.data_register
                }
            }
            _ => 0xFF,
        }
    }
    
    pub fn write(&mut self, port: u8, value: u8) {
        match port & 0x03 {
            0 => {
                // Command register
                self.command_register = value;
                self.execute_command(value);
            }
            1 => self.track_register = value,
            2 => self.sector_register = value,
            3 => {
                // Data register
                self.data_register = value;
                
                if self.state == FdcStatus::Write && self.drq {
                    if self.buffer_pos < self.data_buffer.len() {
                        self.data_buffer[self.buffer_pos] = value;
                        self.buffer_pos += 1;
                        
                        if self.buffer_pos >= self.data_buffer.len() {
                            self.complete_write();
                        }
                    }
                }
            }
            _ => {}
        }
    }
    
    pub fn drive_control(&mut self, value: u8) {
        // Handle 0x7FFB drive control port
        self.current_drive = value & 0x01;
        self.side = (value >> 1) & 0x01;
        self.motor_on = (value & 0x80) != 0;
    }
    
    pub fn insert_disk(&mut self, drive: usize, image: DiskImage) {
        if drive < 2 {
            self.drives[drive] = Some(image);
        }
    }
    
    pub fn eject_disk(&mut self, drive: usize) {
        if drive < 2 {
            self.drives[drive] = None;
        }
    }
    
    fn execute_command(&mut self, cmd: u8) {
        let command_type = cmd >> 4;
        
        match command_type {
            0x0 => self.restore_command(cmd),     // Restore (seek track 0)
            0x1 => self.seek_command(cmd),        // Seek
            0x2..=0x3 => self.step_command(cmd),  // Step
            0x4..=0x5 => self.step_in_command(cmd),  // Step in
            0x6..=0x7 => self.step_out_command(cmd), // Step out
            0x8..=0x9 => self.read_sector_command(cmd),  // Read sector
            0xA..=0xB => self.write_sector_command(cmd), // Write sector
            0xC => self.read_address_command(cmd),       // Read address
            0xD => self.force_interrupt_command(cmd),    // Force interrupt
            0xE => self.read_track_command(cmd),         // Read track
            0xF => self.write_track_command(cmd),        // Write track
            _ => {}
        }
    }
    
    fn restore_command(&mut self, _cmd: u8) {
        self.busy = true;
        self.state = FdcStatus::Seek;
        self.track_register = 0;
        self.track_zero = true;
        
        // Simulate seek time
        self.busy = false;
        self.state = FdcStatus::Idle;
        self.update_status();
    }
    
    fn seek_command(&mut self, _cmd: u8) {
        self.busy = true;
        self.state = FdcStatus::Seek;
        
        let target_track = self.data_register;
        
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            if target_track < disk.tracks_per_side {
                self.track_register = target_track;
                self.track_zero = target_track == 0;
                self.seek_error = false;
            } else {
                self.seek_error = true;
            }
        } else {
            self.seek_error = true;
        }
        
        self.busy = false;
        self.state = FdcStatus::Idle;
        self.update_status();
    }
    
    fn step_command(&mut self, cmd: u8) {
        let update_track = (cmd & 0x10) != 0;
        let direction = if (cmd & 0x20) != 0 { -1i8 } else { 1i8 };
        
        self.busy = true;
        self.state = FdcStatus::Seek;
        
        let new_track = (self.track_register as i8 + direction).max(0) as u8;
        
        if update_track {
            self.track_register = new_track;
        }
        
        self.track_zero = new_track == 0;
        self.busy = false;
        self.state = FdcStatus::Idle;
        self.update_status();
    }
    
    fn step_in_command(&mut self, cmd: u8) {
        self.step_command(cmd | 0x00);  // Step in (towards track 79)
    }
    
    fn step_out_command(&mut self, cmd: u8) {
        self.step_command(cmd | 0x20);  // Step out (towards track 0)
    }
    
    fn read_sector_command(&mut self, _cmd: u8) {
        self.busy = true;
        self.state = FdcStatus::Read;
        self.crc_error = false;
        self.lost_data = false;
        
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            let sector_size = 512;
            let track = self.track_register;
            let sector = self.sector_register;
            let side = self.side;
            
            if sector > 0 && sector <= disk.sectors_per_track {
                let offset = Self::calculate_offset(disk, track, side, sector - 1);
                
                if offset + sector_size <= disk.data.len() {
                    self.data_buffer = disk.data[offset..offset + sector_size].to_vec();
                    self.buffer_pos = 0;
                    self.drq = true;
                } else {
                    self.crc_error = true;
                }
            } else {
                self.crc_error = true;
            }
        } else {
            self.crc_error = true;
        }
        
        if self.crc_error {
            self.busy = false;
            self.state = FdcStatus::Idle;
        }
        
        self.update_status();
    }
    
    fn write_sector_command(&mut self, _cmd: u8) {
        self.busy = true;
        self.state = FdcStatus::Write;
        self.crc_error = false;
        self.lost_data = false;
        
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            if disk.write_protected {
                self.write_protect = true;
                self.busy = false;
                self.state = FdcStatus::Idle;
            } else {
                let sector_size = 512;
                self.data_buffer = vec![0; sector_size];
                self.buffer_pos = 0;
                self.drq = true;
            }
        } else {
            self.crc_error = true;
            self.busy = false;
            self.state = FdcStatus::Idle;
        }
        
        self.update_status();
    }
    
    fn read_address_command(&mut self, _cmd: u8) {
        // Read ID field
        self.busy = true;
        
        if let Some(_disk) = &self.drives[self.current_drive as usize] {
            // Return track, side, sector, sector size
            self.data_buffer = vec![
                self.track_register,
                self.side,
                self.sector_register,
                0x02,  // Sector size code (512 bytes)
                0x00,  // CRC1
                0x00,  // CRC2
            ];
            self.buffer_pos = 0;
            self.drq = true;
            self.state = FdcStatus::Read;
        } else {
            self.crc_error = true;
            self.busy = false;
            self.state = FdcStatus::Idle;
        }
        
        self.update_status();
    }
    
    fn force_interrupt_command(&mut self, _cmd: u8) {
        self.busy = false;
        self.drq = false;
        self.state = FdcStatus::Idle;
        self.update_status();
    }
    
    fn read_track_command(&mut self, _cmd: u8) {
        // Read entire track
        self.busy = true;
        self.state = FdcStatus::Read;
        
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            let track_size = 512 * disk.sectors_per_track as usize;
            let track = self.track_register;
            let side = self.side;
            let offset = Self::calculate_offset(disk, track, side, 0);
            
            if offset + track_size <= disk.data.len() {
                self.data_buffer = disk.data[offset..offset + track_size].to_vec();
                self.buffer_pos = 0;
                self.drq = true;
            } else {
                self.crc_error = true;
                self.busy = false;
                self.state = FdcStatus::Idle;
            }
        } else {
            self.crc_error = true;
            self.busy = false;
            self.state = FdcStatus::Idle;
        }
        
        self.update_status();
    }
    
    fn write_track_command(&mut self, _cmd: u8) {
        // Format track
        self.busy = true;
        self.state = FdcStatus::Write;
        
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            if disk.write_protected {
                self.write_protect = true;
                self.busy = false;
                self.state = FdcStatus::Idle;
            } else {
                let track_size = 512 * disk.sectors_per_track as usize;
                self.data_buffer = vec![0; track_size];
                self.buffer_pos = 0;
                self.drq = true;
            }
        } else {
            self.crc_error = true;
            self.busy = false;
            self.state = FdcStatus::Idle;
        }
        
        self.update_status();
    }
    
    fn complete_read(&mut self) {
        self.drq = false;
        self.busy = false;
        self.state = FdcStatus::Idle;
        
        // Auto-increment sector
        self.sector_register += 1;
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            if self.sector_register > disk.sectors_per_track {
                self.sector_register = 1;
            }
        }
        
        self.update_status();
    }
    
    fn complete_write(&mut self) {
        let track = self.track_register;
        let sector = self.sector_register;
        let side = self.side;
        
        if let Some(disk) = &mut self.drives[self.current_drive as usize] {
            let offset = Self::calculate_offset(disk, track, side, sector - 1);
            
            // Write buffer to disk image
            let end = (offset + self.data_buffer.len()).min(disk.data.len());
            disk.data[offset..end].copy_from_slice(&self.data_buffer[..end - offset]);
        }
        
        self.drq = false;
        self.busy = false;
        self.state = FdcStatus::Idle;
        
        // Auto-increment sector
        self.sector_register += 1;
        if let Some(disk) = &self.drives[self.current_drive as usize] {
            if self.sector_register > disk.sectors_per_track {
                self.sector_register = 1;
            }
        }
        
        self.update_status();
    }
    
    fn calculate_offset(disk: &DiskImage, track: u8, side: u8, sector: u8) -> usize {
        let sectors_per_track = disk.sectors_per_track as usize;
        let track_offset = track as usize * disk.sides as usize * sectors_per_track;
        let side_offset = side as usize * sectors_per_track;
        let sector_offset = sector as usize;
        
        (track_offset + side_offset + sector_offset) * 512
    }
    
    fn update_status(&mut self) {
        self.status_register = 0;
        
        if self.busy {
            self.status_register |= 0x01;  // Busy
        }
        
        if self.drq {
            self.status_register |= 0x02;  // Data Request
        }
        
        if self.index_pulse {
            self.status_register |= 0x04;  // Index
        }
        
        if self.track_zero {
            self.status_register |= 0x04;  // Track 0 (shares bit with index on Type I)
        }
        
        if self.crc_error {
            self.status_register |= 0x08;  // CRC Error
        }
        
        if self.seek_error {
            self.status_register |= 0x10;  // Seek Error
        }
        
        if self.lost_data {
            self.status_register |= 0x04;  // Lost Data (Type II/III)
        }
        
        if self.write_protect {
            self.status_register |= 0x40;  // Write Protect
        }
        
        if !self.motor_on {
            self.status_register |= 0x80;  // Not Ready
        } else if self.drives[self.current_drive as usize].is_none() {
            self.status_register |= 0x80;  // Not Ready (no disk)
        }
    }
}

impl DiskImage {
    pub fn new(data: Vec<u8>, format: DiskFormat) -> Self {
        let (tracks_per_side, sectors_per_track, sides) = match format {
            DiskFormat::DSK => (80, 9, 2),   // 720KB
            DiskFormat::DI => (80, 9, 2),    // 720KB
            DiskFormat::DMK => (80, 9, 2),   // Variable, defaulting to 720KB
        };
        
        Self {
            data,
            format,
            write_protected: false,
            tracks_per_side,
            sectors_per_track,
            sides,
        }
    }
    
    pub fn format(&self) -> DiskFormat {
        self.format
    }
    
    pub fn from_file(data: Vec<u8>, filename: &str) -> Self {
        let format = if filename.ends_with(".dsk") {
            DiskFormat::DSK
        } else if filename.ends_with(".di") {
            DiskFormat::DI
        } else if filename.ends_with(".dmk") {
            DiskFormat::DMK
        } else {
            DiskFormat::DSK  // Default
        };
        
        Self::new(data, format)
    }
    
    pub fn set_write_protected(&mut self, protected: bool) {
        self.write_protected = protected;
    }
}

impl fmt::Display for WD2793 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FDC: Status={:02X} Track={} Sector={} Drive={} Side={} Motor={}",
            self.status_register,
            self.track_register,
            self.sector_register,
            self.current_drive,
            self.side,
            if self.motor_on { "ON" } else { "OFF" }
        )
    }
}