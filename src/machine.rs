use std::{cell::RefCell, collections::VecDeque, fmt, rc::Rc};

use z80::{Z80_io, Z80};

use crate::{
    bus::{Bus, MemorySegment},
    clock::{Clock, ClockEvent, CPU_CYCLES_PER_SCANLINE, SCANLINES_PER_FRAME},
    cpu_extensions::{CpuExtensionHandler, CpuExtensionState},
    partial_hexdump,
    slot::{RamSlot, RomSlot, SlotType},
    vdp::TMS9918,
};

pub struct Machine {
    pub bus: Rc<RefCell<Bus>>,
    pub cpu: Z80<Io>,
    pub queue: Rc<RefCell<VecDeque<Message>>>,
    pub clock: Clock,
    pub cycles: usize,
    pub frame_ready: bool,
    pub disk_drive: Option<crate::disk_drive::SharedDiskDrive>,
}

impl Machine {
    pub fn new(slots: &[SlotType]) -> Self {
        tracing::trace!("Initializing MSX with slots: {:?}", slots);
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let bus = Rc::new(RefCell::new(Bus::new(slots, queue.clone())));
        let io = Io::new(bus.clone());
        let cpu = Z80::new(io);

        let mut machine = Self {
            bus,
            cpu,
            queue,
            clock: Clock::new(),
            cycles: 0,
            frame_ready: false,
            disk_drive: None,
        };

        // Check if slot 1 has a disk ROM and set up disk system if so
        machine.check_and_setup_disk_system();

        machine
    }

    pub fn get_cycles(&self) -> usize {
        self.cycles
    }

    pub fn load_rom(&mut self, slot: u8, data: &[u8]) {
        self.bus.borrow_mut().load_rom(slot, data);
    }

    pub fn load_ram(&mut self, slot: u8) {
        self.bus.borrow_mut().load_ram(slot);
    }

    pub fn load_empty(&mut self, slot: u8) {
        self.bus.borrow_mut().load_empty(slot);
    }

    pub fn print_memory_page_info(&self) {
        self.bus.borrow().print_memory_page_info();
    }

    pub fn get_vdp(&self) -> TMS9918 {
        self.bus.borrow().vdp.clone()
    }

    pub fn mem_size(&self) -> usize {
        // FIXME self.cpu.memory.size()
        64 * 1024
    }

    pub fn ram(&self) -> Vec<u8> {
        let mut memory = Vec::new();
        for pc in 0..self.mem_size() {
            memory.push(self.bus.borrow().read_byte(pc as u16));
        }
        memory
    }

    pub fn vram(&self) -> Vec<u8> {
        self.bus.borrow().vdp.vram.to_vec()
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn halted(&self) -> bool {
        self.cpu.halted
    }

    pub fn memory_dump(&mut self, start: u16, end: u16) -> String {
        partial_hexdump(&self.ram(), start, end)
    }

    pub fn memory(&self) -> Vec<u8> {
        self.ram()
    }

    pub fn vram_dump(&self) -> String {
        let vdp = self.bus.borrow().vdp.clone();
        partial_hexdump(&vdp.vram, 0, 0x4000)
    }

    pub fn vdp(&self) -> TMS9918 {
        self.bus.borrow().vdp.clone()
    }

    pub fn step_for(&mut self, n: usize) {
        let mut cycles_executed = 0;

        while cycles_executed < n {
            // Process any pending messages first
            while let Some(message) = self.queue.borrow_mut().pop_front() {
                match message {
                    Message::EnableInterrupts => {
                        // tracing::debug!("[Machine] Asserting IRQ");
                        self.cpu.assert_irq(0);
                    }
                    Message::DisableInterrupts => {
                        // tracing::debug!("[Machine] Clearing IRQ");
                        self.cpu.clr_irq();
                    }
                    Message::CpuStep => {
                        // This shouldn't happen in the queue
                    }
                    Message::DebugPC => {
                        tracing::info!("Cycles: {} PC: {:04X}", self.cycles, self.cpu.pc);
                    }
                }
            }

            // Execute CPU instruction
            let cycles_taken = self.cpu.step();
            
            // Debug disk ROM calls
            if self.cpu.pc >= 0x7000 && self.cpu.pc < 0x8000 && self.cycles % 1000 == 0 {
                static mut LAST_PC: u16 = 0;
                unsafe {
                    if self.cpu.pc != LAST_PC {
                        LAST_PC = self.cpu.pc;
                        // Only log significant PCs
                        if self.cpu.pc == 0x744D || self.cpu.pc == 0x780B || self.cpu.pc == 0x785F {
                            tracing::trace!("Disk ROM PC: 0x{:04X}", self.cpu.pc);
                        }
                    }
                }
            }

            // Debug interrupt state changes
            // static mut LAST_IM: u8 = 0xFF;
            // static mut LAST_IFF1: bool = true;
            // unsafe {
            //     if self.cpu.interrupt_mode != LAST_IM || self.cpu.iff1 != LAST_IFF1 {
            //         tracing::info!(
            //             "[Machine] Interrupt state changed - IM: {} -> {}, IFF1: {} -> {} at PC: {:04X}",
            //             LAST_IM, self.cpu.interrupt_mode,
            //             LAST_IFF1, self.cpu.iff1,
            //             self.cpu.pc
            //         );
            //         LAST_IM = self.cpu.interrupt_mode;
            //         LAST_IFF1 = self.cpu.iff1;
            //     }
            // }

            // Clock the bus components (including PSG)
            self.bus.borrow_mut().clock(cycles_taken);

            // Update clock and handle timing events
            let events = self.clock.tick(cycles_taken);
            if !events.is_empty() {
                self.handle_clock_events(events);
            }

            self.cycles += cycles_taken as usize;
            cycles_executed += cycles_taken as usize;
        }
    }

    fn handle_clock_events(&mut self, events: Vec<ClockEvent>) {
        for event in events {
            match event {
                ClockEvent::VBlankStart => {
                    // Generate VDP interrupt
                    let mut bus = self.bus.borrow_mut();
                    // Evaluate sprites once per frame at the start of VBlank
                    bus.vdp.evaluate_all_sprite_lines();
                    bus.vdp.set_vblank(true);
                    if bus.vdp.is_interrupt_enabled() {
                        // tracing::debug!("[Machine] VBlank interrupt enabled, asserting IRQ");
                        self.cpu.assert_irq(0);
                    } else {
                        // tracing::debug!("[Machine] VBlank interrupt disabled");
                    }
                }
                ClockEvent::VBlankEnd => {
                    let mut bus = self.bus.borrow_mut();
                    bus.vdp.set_vblank(false);
                }
                ClockEvent::HBlankStart => {
                    // Could be used for mid-scanline effects
                }
                ClockEvent::HBlankEnd => {
                    // Could be used for mid-scanline effects
                }
                ClockEvent::ScanlineStart(line) => {
                    // Update VDP scanline for scanline-based rendering
                    let mut bus = self.bus.borrow_mut();
                    bus.vdp.set_current_scanline(line as u16);
                }
                ClockEvent::FrameEnd => {
                    self.frame_ready = true;
                    tracing::trace!(
                        "Frame {} completed, total cycles: {}",
                        self.clock.frame_count(),
                        self.clock.total_cycles()
                    );
                }
            }
        }
    }

    pub fn step_frame(&mut self) {
        self.frame_ready = false;
        let cycles_per_frame = (SCANLINES_PER_FRAME * CPU_CYCLES_PER_SCANLINE) as usize;
        let target_cycles = self.cycles + cycles_per_frame;

        // Run CPU for one complete frame worth of cycles
        while self.cycles < target_cycles {
            // Process any pending messages
            while let Some(message) = self.queue.borrow_mut().pop_front() {
                match message {
                    Message::EnableInterrupts => {
                        // tracing::debug!("[Machine] Asserting IRQ");
                        self.cpu.assert_irq(0);
                    }
                    Message::DisableInterrupts => {
                        // tracing::debug!("[Machine] Clearing IRQ");
                        self.cpu.clr_irq();
                    }
                    Message::CpuStep => {
                        // This shouldn't happen in the queue
                    }
                    Message::DebugPC => {
                        tracing::info!("Cycles: {} PC: {:04X}", self.cycles, self.cpu.pc);
                    }
                }
            }

            // Execute CPU instruction and get actual cycle count
            let cycles_taken = self.cpu.step();

            // Debug interrupt state changes
            // static mut LAST_IM: u8 = 0xFF;
            // static mut LAST_IFF1: bool = true;
            // unsafe {
            //     if self.cpu.interrupt_mode != LAST_IM || self.cpu.iff1 != LAST_IFF1 {
            //         tracing::info!(
            //             "[Machine] Interrupt state changed - IM: {} -> {}, IFF1: {} -> {} at PC: {:04X}",
            //             LAST_IM, self.cpu.interrupt_mode,
            //             LAST_IFF1, self.cpu.iff1,
            //             self.cpu.pc
            //         );
            //         LAST_IM = self.cpu.interrupt_mode;
            //         LAST_IFF1 = self.cpu.iff1;
            //     }
            // }

            // Clock the bus components (including PSG)
            self.bus.borrow_mut().clock(cycles_taken);

            // Update clock and handle timing events
            let events = self.clock.tick(cycles_taken);
            if !events.is_empty() {
                self.handle_clock_events(events);
            }

            self.cycles += cycles_taken as usize;
        }

        // Frame is complete
        self.frame_ready = true;
    }

    pub fn is_frame_ready(&self) -> bool {
        self.frame_ready
    }

    pub fn get_frame_progress(&self) -> f64 {
        self.clock.frame_progress()
    }

    fn check_and_setup_disk_system(&mut self) {
        use crate::disk_drive::SharedDiskDrive;
        use crate::disk_rom_manager::DiskRomManager;

        // Check if slot 1 contains a disk ROM (typically 16KB at 0x4000)
        let has_disk_rom = {
            let bus = self.bus.borrow();
            let slot1 = bus.get_slot(1);
            // Check for disk ROM signature at typical locations
            if slot1.size() >= 0x4000 {
                let byte0 = slot1.read(0x4000);
                let byte1 = slot1.read(0x4001);
                byte0 == 0x41 && byte1 == 0x42 // 'AB' header
            } else {
                false
            }
        };

        if has_disk_rom {
            tracing::info!("Disk ROM detected in slot 1, setting up disk system");

            // Patch the disk ROM if it's a RomSlot
            {
                let mut bus = self.bus.borrow_mut();
                if let SlotType::Rom(rom_slot) = bus.get_slot_mut(1) {
                    DiskRomManager::patch_disk_rom(rom_slot);
                }
            }

            // Create disk drive system
            let disk_drive = SharedDiskDrive::new();

            // Set up disk extensions
            DiskRomManager::setup_disk_system(&self.cpu.io, disk_drive.clone(), self.bus.clone());

            // Store the disk drive for later use
            self.disk_drive = Some(disk_drive);

            tracing::info!("Disk system initialized");
        }
    }

    pub fn primary_slot_config(&self) -> u8 {
        self.bus.borrow().primary_slot_config()
    }

    pub fn memory_segments(&self) -> Vec<MemorySegment> {
        self.bus.borrow().memory_segments()
    }

    /// Load a DSK image file into the specified drive (0 = A:, 1 = B:)
    pub fn load_disk_image(&mut self, drive: u8, image_data: Vec<u8>) -> Result<(), String> {
        use crate::dsk_image::DiskImage;

        if let Some(ref disk_drive) = self.disk_drive {
            // Parse the disk image
            let disk_image = DiskImage::from_bytes(image_data)
                .map_err(|e| format!("Failed to parse disk image: {}", e))?;

            // Store info before moving disk_image
            let total_sectors = disk_image.get_total_sectors();
            let size_kb = total_sectors as u32 * 512 / 1024;

            // Insert into drive
            if let Ok(mut drive_guard) = disk_drive.clone_inner().lock() {
                drive_guard
                    .insert_disk(drive, disk_image)
                    .map_err(|e| format!("Failed to insert disk: {}", e))?;

                tracing::info!(
                    "Loaded disk image into drive {}: {} KB, {} sectors",
                    if drive == 0 { "A:" } else { "B:" },
                    size_kb,
                    total_sectors
                );
                Ok(())
            } else {
                Err("Failed to lock disk drive".to_string())
            }
        } else {
            Err(
                "Disk system not initialized. Make sure a disk ROM is loaded in slot 1."
                    .to_string(),
            )
        }
    }

    /// Eject disk from the specified drive
    pub fn eject_disk(&mut self, drive: u8) -> Result<(), String> {
        if let Some(ref disk_drive) = self.disk_drive {
            if let Ok(mut drive_guard) = disk_drive.clone_inner().lock() {
                drive_guard
                    .eject_disk(drive)
                    .map_err(|e| format!("Failed to eject disk: {}", e))?;
                Ok(())
            } else {
                Err("Failed to lock disk drive".to_string())
            }
        } else {
            Err("Disk system not initialized".to_string())
        }
    }

    /// Check if disk system is available
    pub fn has_disk_system(&self) -> bool {
        self.disk_drive.is_some()
    }
    
    /// Insert a new formatted disk into the specified drive
    pub fn insert_new_disk(&mut self, drive: u8, media_type: u8) -> Result<(), String> {
        if let Some(ref disk_drive) = self.disk_drive {
            if let Ok(mut drive_guard) = disk_drive.clone_inner().lock() {
                drive_guard
                    .insert_new_disk(drive, media_type)
                    .map_err(|e| format!("Failed to insert new disk: {}", e))?;
                Ok(())
            } else {
                Err("Failed to lock disk drive".to_string())
            }
        } else {
            Err(
                "Disk system not initialized. Make sure a disk ROM is loaded in slot 1."
                    .to_string(),
            )
        }
    }
}

impl Default for Machine {
    fn default() -> Self {
        println!("Initializing MSX...");
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let bus = Rc::new(RefCell::new(Bus::new(
            &[
                SlotType::Empty,
                SlotType::Empty,
                SlotType::Empty,
                SlotType::Empty,
            ],
            queue.clone(),
        )));
        let io = Io::new(bus.clone());
        let cpu = Z80::new(io);

        Self {
            cpu,
            bus,
            queue,
            clock: Clock::new(),
            cycles: 0,
            frame_ready: false,
            disk_drive: None,
        }
    }
}

#[derive(Default)]
pub struct MachineBuilder {
    slots: Vec<SlotType>,
}

impl MachineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ram_slot(&mut self, base: u16, size: u32) -> &mut Self {
        self.slots.push(SlotType::Ram(RamSlot::new(base, size)));
        self
    }

    pub fn rom_slot(&mut self, data: &[u8], base: u16, size: u32) -> &mut Self {
        self.slots
            .push(SlotType::Rom(RomSlot::new(data, base, size)));
        self
    }

    pub fn empty_slot(&mut self) -> &mut Self {
        self.slots.push(SlotType::Empty);
        self
    }

    pub fn build(&self) -> Machine {
        if self.slots.len() != 4 {
            panic!(
                "MachineBuilder: Expected exactly 4 slots, but got {}",
                self.slots.len()
            );
        }

        Machine::new(&self.slots)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramEntry {
    pub address: u16,
    pub instruction: String,
    pub data: String,
    pub dump: Option<String>,
}

impl fmt::Display for ProgramEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04X}  {:<12}  {:<20} {}",
            self.address,
            self.data,
            self.instruction,
            self.dump.as_deref().unwrap_or("")
        )
    }
}

#[derive(Debug)]
pub enum Message {
    EnableInterrupts,
    DisableInterrupts,
    CpuStep,
    DebugPC,
}

pub struct Io {
    pub bus: Rc<RefCell<Bus>>,
    pub extension_handlers: RefCell<std::collections::HashMap<u8, Box<dyn CpuExtensionHandler>>>,
}

impl Io {
    pub fn new(bus: Rc<RefCell<Bus>>) -> Self {
        Self {
            bus,
            extension_handlers: RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn register_extension_handler(&self, ext_num: u8, handler: Box<dyn CpuExtensionHandler>) {
        self.extension_handlers
            .borrow_mut()
            .insert(ext_num, handler);
    }
}

impl Z80_io for Io {
    fn read_byte(&self, address: u16) -> u8 {
        self.bus.borrow().read_byte(address)
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        self.bus.borrow_mut().write_byte(address, value)
    }

    fn port_in(&self, port: u16) -> u8 {
        self.bus.borrow_mut().input(port as u8)
    }

    fn port_out(&mut self, port: u16, value: u8) {
        self.bus.borrow_mut().output(port as u8, value)
    }

    fn handle_extension(&mut self, ext_num: u8, z80: &mut Z80<Self>) -> Option<u32> {
        // First check if we have a registered handler for this extension
        let handler_exists = self.extension_handlers.borrow().contains_key(&ext_num);

        if handler_exists {
            let mut state = CpuExtensionState::from_z80(z80, ext_num);

            // Call the handler
            let handled =
                if let Some(handler) = self.extension_handlers.borrow_mut().get_mut(&ext_num) {
                    handler.extension_begin(&mut state)
                } else {
                    false
                };

            if handled {
                // Apply any state changes back to the Z80
                state.apply_to_z80(z80);

                // TODO: Handle extension_finish if needed

                // Return cycles consumed (4 for the ED XX instruction)
                return Some(4);
            }
        }

        // Extension not handled
        None
    }
}
