use wasmsx::fdc::{DiskFormat, DiskImage, WD2793};

#[test]
fn test_fdc_basic_operations() {
    let mut fdc = WD2793::new();
    
    // Test initial state
    assert_eq!(fdc.read(0), 0x84); // Status: Not Ready (0x80) + Track 0 (0x04)
    assert_eq!(fdc.read(1), 0);    // Track register
    assert_eq!(fdc.read(2), 1);    // Sector register (starts at 1)
    
    // Create a disk image
    let disk_data = vec![0u8; 720 * 1024]; // 720KB disk
    let disk = DiskImage::new(disk_data, DiskFormat::DSK);
    
    // Insert disk
    fdc.insert_disk(0, disk);
    
    // Enable motor
    fdc.drive_control(0x80); // Motor on, drive 0
    
    // Status should now show ready
    assert_eq!(fdc.read(0) & 0x80, 0); // Not Ready bit should be clear
}

#[test]
fn test_fdc_seek_operations() {
    let mut fdc = WD2793::new();
    
    // Create and insert disk
    let disk_data = vec![0u8; 720 * 1024];
    let disk = DiskImage::new(disk_data, DiskFormat::DSK);
    fdc.insert_disk(0, disk);
    fdc.drive_control(0x80); // Motor on
    
    // Seek to track 10
    fdc.write(3, 10);  // Data register = 10
    fdc.write(0, 0x10); // Seek command
    
    // Check track register
    assert_eq!(fdc.read(1), 10);
    
    // Restore (seek to track 0)
    fdc.write(0, 0x00); // Restore command
    assert_eq!(fdc.read(1), 0);
    assert_eq!(fdc.read(0) & 0x04, 0x04); // Track 0 flag should be set
}

#[test]
fn test_fdc_read_sector() {
    let mut fdc = WD2793::new();
    
    // Create disk with test pattern
    let mut disk_data = vec![0u8; 720 * 1024];
    // Write test pattern to first sector
    for i in 0..512 {
        disk_data[i] = (i & 0xFF) as u8;
    }
    
    let disk = DiskImage::new(disk_data, DiskFormat::DSK);
    fdc.insert_disk(0, disk);
    fdc.drive_control(0x80); // Motor on
    
    // Read sector 1 of track 0
    fdc.write(1, 0);    // Track 0
    fdc.write(2, 1);    // Sector 1
    fdc.write(0, 0x80); // Read sector command
    
    // Check DRQ is set
    assert_eq!(fdc.read(0) & 0x02, 0x02);
    
    // Read first few bytes
    assert_eq!(fdc.read(3), 0x00);
    assert_eq!(fdc.read(3), 0x01);
    assert_eq!(fdc.read(3), 0x02);
    assert_eq!(fdc.read(3), 0x03);
}

#[test]
fn test_fdc_write_protection() {
    let mut fdc = WD2793::new();
    
    // Create write-protected disk
    let disk_data = vec![0u8; 720 * 1024];
    let mut disk = DiskImage::new(disk_data, DiskFormat::DSK);
    disk.set_write_protected(true);
    
    fdc.insert_disk(0, disk);
    fdc.drive_control(0x80); // Motor on
    
    // Try to write sector
    fdc.write(1, 0);    // Track 0
    fdc.write(2, 1);    // Sector 1
    fdc.write(0, 0xA0); // Write sector command
    
    // Check write protect flag
    assert_eq!(fdc.read(0) & 0x40, 0x40);
}