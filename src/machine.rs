use std::{cell::RefCell, fmt};

use z80::{Z80_io, Z80};

use crate::{
    bus::{Bus, MemorySegment},
    slot::{RamSlot, RomSlot, SlotType},
    utils::hexdump,
    vdp::TMS9918,
};

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

pub struct Io {
    pub bus: RefCell<Bus>,
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

// #[derive(Derivative, Serialize, Deserialize)]
// #[derivative(Clone, Debug, PartialEq, Eq)]

pub struct Machine {
    pub cpu: Z80<Io>,
    pub current_scanline: u16,
}

impl Default for Machine {
    fn default() -> Self {
        println!("Initializing MSX...");
        let bus = RefCell::new(Bus::default());
        let io = Io { bus };
        let cpu = Z80::new(io);

        Self {
            cpu,
            current_scanline: 0,
        }
    }
}

impl Machine {
    pub fn new(slots: &[SlotType]) -> Self {
        let cpu = Z80::new(Io {
            bus: RefCell::new(Bus::new(slots)),
        });

        Self {
            cpu,
            current_scanline: 0,
        }
    }

    pub fn load_rom(&mut self, slot: u8, data: &[u8]) {
        self.cpu.io.bus.borrow_mut().load_rom(slot, data);
    }

    pub fn load_ram(&mut self, slot: u8) {
        self.cpu.io.bus.borrow_mut().load_ram(slot);
    }

    pub fn load_empty(&mut self, slot: u8) {
        self.cpu.io.bus.borrow_mut().load_empty(slot);
    }

    pub fn print_memory_page_info(&self) {
        self.cpu.io.bus.borrow().print_memory_page_info();
    }

    pub fn get_vdp(&self) -> TMS9918 {
        self.cpu.io.bus.borrow().vdp.clone()
    }

    pub fn mem_size(&self) -> usize {
        // FIXME self.cpu.memory.size()
        64 * 1024
    }

    pub fn ram(&self) -> Vec<u8> {
        let mut memory = Vec::new();
        for pc in 0..self.mem_size() {
            memory.push(self.cpu.io.read_byte(pc as u16));
        }
        memory
    }

    pub fn vram(&self) -> Vec<u8> {
        self.cpu.io.bus.borrow().vdp.vram.to_vec()
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn halted(&self) -> bool {
        self.cpu.halted
    }

    pub fn get_memory(&self, address: u16) -> u8 {
        self.cpu.io.read_byte(address)
    }

    pub fn memory_dump(&mut self, start: u16, end: u16) -> String {
        hexdump(&self.ram(), start, end)
    }

    pub fn memory(&self) -> Vec<u8> {
        self.ram()
    }

    pub fn vram_dump(&self) -> String {
        let vdp = self.cpu.io.bus.borrow().vdp.clone();
        hexdump(&vdp.vram, 0, 0x4000)
    }

    pub fn vdp(&self) -> TMS9918 {
        self.cpu.io.bus.borrow().vdp.clone()
    }

    pub fn step(&mut self) {
        self.cpu.step();
        self.current_scanline = (self.current_scanline + 1) % 192;
    }

    pub fn primary_slot_config(&self) -> u8 {
        self.cpu.io.bus.borrow().primary_slot_config()
    }

    pub fn memory_segments(&self) -> Vec<MemorySegment> {
        self.cpu.io.bus.borrow().memory_segments()
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
        Machine::new(&self.slots)
    }
}
