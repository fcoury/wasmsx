use std::{cell::RefCell, collections::VecDeque, fmt, rc::Rc};

use z80::{Z80_io, Z80};

use crate::{
    bus::{Bus, MemorySegment},
    clock::{Clock, ClockEvent, CPU_CYCLES_PER_SCANLINE, SCANLINES_PER_FRAME},
    fdc::DiskImage,
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
}

impl Machine {
    pub fn new(slots: &[SlotType]) -> Self {
        tracing::info!("Initializing MSX with slots: {:?}", slots);
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let bus = Rc::new(RefCell::new(Bus::new(slots, queue.clone())));
        let io = Io::new(bus.clone());
        let cpu = Z80::new(io);

        Self {
            bus,
            cpu,
            queue,
            clock: Clock::new(),
            cycles: 0,
            frame_ready: false,
        }
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
                        self.cpu.assert_irq(0);
                    }
                    Message::DisableInterrupts => {
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
                    bus.vdp.set_vblank(true);
                    if bus.vdp.is_interrupt_enabled() {
                        self.cpu.assert_irq(0);
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
                    tracing::trace!("Frame {} completed, total cycles: {}", 
                        self.clock.frame_count(), self.clock.total_cycles());
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
                        self.cpu.assert_irq(0);
                    }
                    Message::DisableInterrupts => {
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

    pub fn primary_slot_config(&self) -> u8 {
        self.bus.borrow().primary_slot_config()
    }

    pub fn memory_segments(&self) -> Vec<MemorySegment> {
        self.bus.borrow().memory_segments()
    }

    pub fn enable_disk_system(&mut self) {
        tracing::info!("[Machine] Enabling disk system");
        self.bus.borrow_mut().enable_fdc();
    }

    pub fn insert_disk(&mut self, drive: usize, image: DiskImage) {
        if let Some(fdc) = &mut self.bus.borrow_mut().fdc {
            fdc.insert_disk(drive, image);
        }
    }

    pub fn eject_disk(&mut self, drive: usize) {
        if let Some(fdc) = &mut self.bus.borrow_mut().fdc {
            fdc.eject_disk(drive);
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
        }
    }
}

#[derive(Default)]
pub struct MachineBuilder {
    slots: Vec<SlotType>,
    enable_disk: bool,
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

    pub fn with_disk_support(&mut self) -> &mut Self {
        self.enable_disk = true;
        self
    }

    pub fn build(&self) -> Machine {
        if self.slots.len() != 4 {
            panic!(
                "MachineBuilder: Expected exactly 4 slots, but got {}",
                self.slots.len()
            );
        }

        let mut machine = Machine::new(&self.slots);
        if self.enable_disk {
            machine.enable_disk_system();
        }
        machine
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
}

impl Io {
    pub fn new(bus: Rc<RefCell<Bus>>) -> Self {
        Self { bus }
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
}
