# WebMSX Disk Drive Emulation Investigation

## Overview

This document details the findings from investigating WebMSX's disk drive emulation system, particularly focusing on how the emulator loads and handles DSK images.

## Architecture Overview

WebMSX implements disk emulation through a sophisticated multi-layered architecture that bridges MSX software running on the emulated Z80 CPU with JavaScript disk image management:

```
MSX Software (Z80)
    ↓
CPU.js (Z80 emulation with extensions)
    ↓
BUS.js (Memory/IO routing & CPU extension dispatch)
    ↓
CartridgeDiskPatched.js (Disk ROM with patches)
    ↓
ImageDiskDriver.js (JavaScript BIOS implementation)
    ↓
DiskDriveSocket (Hardware interface)
    ↓
FileDiskDrive.js (Drive management & disk stacks)
    ↓
DiskImages.js (Disk image format handling)
    ↓
FileLoader.js (File loading & format detection)
```

## Key Components

### 1. FileLoader.js
**Location**: `WebMSX/src/main/room/files/FileLoader.js`

The central file loading component that:
- Handles multiple input methods (drag & drop, file chooser, URL loading)
- Detects file types based on content and extension
- Routes disk files to appropriate handlers
- Supports ZIP file extraction
- Manages loading of multiple disk images

Key methods:
- `loadDiskStackFromFiles()` - Loads disk images into drive stacks
- `checkFileHasValidImages()` - Validates disk image formats
- `tryLoadFilesAsMedia()` - Attempts to load files as various media types

### 2. FileDiskDrive.js
**Location**: `WebMSX/src/main/room/disk/FileDiskDrive.js`

Manages virtual disk drives with support for:
- Three drives: Drive A (0), Drive B (1), and Hard Drive (2)
- Disk stacks - up to 10 disks per floppy drive
- Disk operations: insert, remove, format, save
- Motor state simulation with spin-up/down delays
- Disk change detection

Key features:
- `driveStack` array maintains loaded disks for each drive
- `curDisk` tracks currently inserted disk from stack
- Implements DiskDriver interface for BIOS interaction
- Supports both floppy and hard disk operations

### 3. DiskImages.js
**Location**: `WebMSX/src/main/room/disk/DiskImages.js`

Handles disk image format operations:
- Creates new disk images with proper boot sectors
- Formats disks as FAT12/FAT16
- Writes files to disk images maintaining FAT structure
- Supports various media types and geometries

Supported formats:
- Floppy: 160KB, 180KB, 320KB, 360KB, 640KB, 720KB
- Hard disk: 16MB, 32MB, 64MB, 128MB
- Both FAT12 and FAT16 filesystems

### 4. ImageDiskDriver.js
**Location**: `WebMSX/src/main/msx/drivers/ImageDiskDriver.js`

Implements the MSX-DOS disk driver using CPU extension protocol:
- Patches disk BIOS ROM to intercept system calls
- Handles BIOS operations via CPU extensions (0xe0-0xea)
- Supports both MSX-DOS 1/2 and SymbOS disk access

Key operations:
- `DSKIO` - Read/write sectors (extensions 0xe4)
- `DSKCHG` - Detect disk changes (extension 0xe5)
- `GETDPB` - Get disk parameter block (extension 0xe6)
- `DSKFMT` - Format disk (extension 0xe8)

## Disk Loading Process

1. **File Input**
   - User provides disk file via drag/drop, file dialog, or URL
   - FileLoader receives and processes the file

2. **Format Detection**
   - Content is checked for valid disk sizes
   - ZIP files are extracted if necessary
   - Multiple disk images in single file are split

3. **Validation**
   - Floppy disks: Must match known sizes (360KB, 720KB, etc.)
   - Hard disks: Must be sector-aligned, minimum size enforced
   - Partition table or FAT headers are checked

4. **Stack Management**
   - Disks are added to drive stacks
   - Current disk is set (usually first in stack)
   - User can switch between stacked disks

5. **BIOS Integration**
   - Disk ROM cartridge is loaded and patched
   - CPU extensions replace BIOS disk routines
   - All disk I/O goes through the driver

## Disk ROM Loading

The system identifies and loads disk ROMs through:

1. **ROM Database** - Hash-based identification of known disk ROMs
2. **Format Detection** - Size-based detection (16KB-64KB)
3. **Cartridge Creation** - Appropriate cartridge class instantiated:
   - `CartridgeDiskPatched` - Standard disk ROMs
   - `CartridgeDiskPatchedDOS2TR` - DOS2 ROMs
   - `CartridgeNextorPatched` - Nextor ROMs

## Special Features

### Disk Stacks
- Multiple disks can be loaded per drive
- Quick switching without ejection
- Useful for multi-disk software

### Files as Disk
- Create disk images from loose files
- Automatically builds FAT structure
- Maintains file dates and attributes

### Boot Disk Creation
- Automatically create bootable DOS disks
- Includes system files for MSX-DOS 1/2
- Optional Nextor support

### Save State Support
- Complete disk state preservation
- Includes disk content and motor states
- Compressed storage format

## Hardware Emulation Core

### CPU Extension Mechanism

WebMSX uses a clever CPU extension mechanism to implement disk operations:

1. **Extension Instructions**: The Z80 CPU recognizes special ED xx instructions (0xED 0xE0-0xFF)
2. **Interception**: When CPU encounters these instructions, it calls `bus.cpuExtensionBegin()`
3. **Routing**: BUS routes extensions 0xE0-0xEF to the slot containing the instruction
4. **JavaScript Execution**: The disk driver's JavaScript methods execute the operation
5. **State Return**: Results are returned to CPU registers (A for status, F for flags, etc.)

### Disk Operation Flow

Here's how a disk read operation flows through the system:

1. **MSX Software** calls disk BIOS routine (e.g., DSKIO at address in jump table)
2. **Patched ROM** contains `ED E4` instruction instead of original Z80 code
3. **CPU** recognizes extension and extracts CPU state
4. **BUS** routes to CartridgeDiskPatched's `cpuExtensionBegin()`
5. **ImageDiskDriver** receives the call with CPU registers:
   - A: Drive number
   - B: Number of sectors
   - C: Media descriptor
   - DE: Logical sector number
   - HL: Memory address for data
6. **Driver** calls FileDiskDrive's `readSectorsToSlot()`
7. **FileDiskDrive** reads from disk image buffer
8. **Data Transfer** occurs directly to emulated memory
9. **Results** returned: F register (carry for error), A (error code), B (sectors transferred)

### Key Hardware Components

#### CPU.js (Z80/R800 Emulation)
- Implements complete Z80 instruction set
- Adds extension mechanism for ED xx opcodes
- Manages CPU state extraction/reinsertion
- Handles extra iterations for slow operations

#### BUS.js (System Bus)
- Routes memory and I/O accesses
- Manages primary and secondary slot configurations
- Dispatches CPU extensions to appropriate handlers
- For extensions 0xE0-0xEF: routes to slot at instruction address
- For extensions 0xF0-0xFF: routes to registered handlers

#### DiskDriveSocket
- Interface between hardware emulation and disk drives
- Tracks connected disk interfaces (floppy/hard disk)
- Manages DOS version detection
- Handles auto power-on for disk operations

## Technical Details

### CPU Extension Protocol
- **State Extraction**: CPU state (registers, PC, SP) passed to extension
- **Synchronous Execution**: JavaScript code runs during CPU instruction
- **Extra Iterations**: Long operations can request additional CPU cycles
- **State Modification**: Extension can modify any CPU register

### Disk BIOS Patching
The disk ROM is patched at specific locations:
- **INIHRD** (0xE0): Initialize hard disk
- **DRIVES** (0xE2): Get number of drives
- **DSKIO** (0xE4): Read/write sectors
- **DSKCHG** (0xE5): Check disk change
- **GETDPB** (0xE6): Get disk parameter block
- **CHOICE** (0xE7): Format choice string
- **DSKFMT** (0xE8): Format disk
- **MTOFF** (0xEA): Motor off

### Sector Operations
- Fixed 512 bytes per sector
- Direct memory transfer for efficiency
- Boundary checking prevents overruns
- Slot-aware memory access respecting MSX banking

### Motor Simulation
- Spin-up delay: 100,000 CPU iterations
- Spin-down delay: 2300ms for floppies, 50ms for hard disk
- LED state indication through UI
- Realistic timing for software compatibility

### Format Support
- Media descriptor bytes: 0xF8-0xFF for floppies
- Cluster sizes: 512-16384 bytes
- Both MBR and direct FAT layouts
- FAT12 for floppies, FAT12/16 for hard disks

## Implementation Notes

The disk emulation achieves high accuracy through:
- **Cycle-accurate timing**: Motor delays match real hardware
- **Proper error handling**: All MSX-DOS error codes supported
- **Complete BIOS compatibility**: All disk BIOS calls implemented
- **Multi-OS support**: MSX-DOS 1/2, Nextor, and SymbOS
- **Efficient design**: JavaScript handles high-level operations while maintaining timing accuracy

The CPU extension mechanism is particularly elegant, allowing complex disk operations to be implemented in JavaScript while appearing as native Z80 instructions to the MSX software. This approach provides both performance and accuracy, as the actual sector data manipulation happens in JavaScript while the MSX software experiences proper timing and behavior.