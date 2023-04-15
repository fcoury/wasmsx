use std::{cell::RefCell, fmt, rc::Weak};

use z80::Z80_io;

use crate::{
    bus::Bus,
    slot::{RamSlot, RomSlot, SlotType},
    vdp::DisplayMode,
};

// #[derive(Derivative, Serialize, Deserialize)]
// #[derivative(Clone, Debug, PartialEq, Eq)]

pub struct Machine {
    pub bus: Bus,
    pub current_scanline: u16,
}

impl Default for Machine {
    fn default() -> Self {
        println!("Initializing MSX...");
        let bus = Bus::new(&[
            SlotType::Empty,
            SlotType::Empty,
            SlotType::Empty,
            SlotType::Empty,
        ]);

        Self {
            bus,
            current_scanline: 0,
        }
    }
}

impl Machine {
    pub fn new(slots: &[SlotType]) -> Self {
        let bus = Bus::new(slots);

        Self {
            bus,
            current_scanline: 0,
        }
    }

    pub fn screen_buffer(&self) -> Vec<u8> {
        self.bus.screen_buffer()
    }

    pub fn vram(&self) -> Vec<u8> {
        self.bus.vram()
    }

    pub fn load_rom(&mut self, slot: u8, data: &[u8]) {
        self.bus.load_rom(slot, data);
    }

    pub fn load_ram(&mut self, slot: u8) {
        self.bus.load_ram(slot);
    }

    pub fn load_empty(&mut self, slot: u8) {
        self.bus.load_empty(slot);
    }

    pub fn print_memory_page_info(&self) {
        self.bus.print_memory_page_info();
    }

    pub fn mem_size(&self) -> usize {
        // FIXME self.cpu.memory.size()
        64 * 1024
    }

    pub fn ram(&self) -> Vec<u8> {
        let mut memory = Vec::new();
        for pc in 0..self.mem_size() {
            memory.push(self.bus.read_byte(pc as u16));
        }
        memory
    }

    pub fn pc(&self) -> u16 {
        self.bus.pc()
    }

    pub fn halted(&self) -> bool {
        self.bus.halted()
    }

    pub fn step(&mut self) {
        self.bus.step();
        self.current_scanline = (self.current_scanline + 1) % 192;
    }

    pub fn primary_slot_config(&self) -> u8 {
        self.bus.primary_slot_config()
    }

    pub fn display_mode(&self) -> DisplayMode {
        self.bus.display_mode()
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
    pub bus: Weak<RefCell<Bus>>,
}

impl Io {
    pub fn new(bus: Weak<RefCell<Bus>>) -> Self {
        Io { bus }
    }
}

impl Z80_io for Io {
    fn read_byte(&self, address: u16) -> u8 {
        if let Some(bus) = self.bus.upgrade() {
            if address == 0x1452 || address == 0x0d12 || address == 0x0c3c {
                tracing::info!("[KEYBOARD] Reading from {:04X}", address);
            }
            bus.borrow().read_byte(address)
        } else {
            panic!("Bus is not available")
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        if let Some(bus) = self.bus.upgrade() {
            if address == 0x1452 || address == 0x0d12 || address == 0x0c3c {
                tracing::info!("[KEYBOARD] Writing to {:04X}", address);
            }
            bus.borrow_mut().write_byte(address, value)
        } else {
            panic!("Bus is not available")
        }
    }

    fn port_in(&self, port: u16) -> u8 {
        if let Some(bus) = self.bus.upgrade() {
            bus.borrow_mut().input(port as u8)
        } else {
            panic!("Bus is not available")
        }
    }

    fn port_out(&mut self, port: u16, value: u8) {
        if let Some(bus) = self.bus.upgrade() {
            bus.borrow_mut().output(port as u8, value)
        } else {
            panic!("Bus is not available")
        }
    }
}
