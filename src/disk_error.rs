// Disk emulation error types

use std::fmt;

#[derive(Debug)]
pub enum DiskError {
    InvalidDrive,
    NoDisk,
    InvalidSector,
    ReadError,
    WriteError,
    WriteProtected,
    InvalidSize(String),
    FormatError(String),
}

impl fmt::Display for DiskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskError::InvalidDrive => write!(f, "Invalid drive number"),
            DiskError::NoDisk => write!(f, "No disk in drive"),
            DiskError::InvalidSector => write!(f, "Invalid sector number"),
            DiskError::ReadError => write!(f, "Disk read error"),
            DiskError::WriteError => write!(f, "Disk write error"),
            DiskError::WriteProtected => write!(f, "Disk is write protected"),
            DiskError::InvalidSize(msg) => write!(f, "Invalid disk size: {}", msg),
            DiskError::FormatError(msg) => write!(f, "Format error: {}", msg),
        }
    }
}

impl std::error::Error for DiskError {}

impl From<std::io::Error> for DiskError {
    fn from(_: std::io::Error) -> Self {
        DiskError::ReadError
    }
}