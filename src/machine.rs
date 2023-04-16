use std::{cell::RefCell, fmt, rc::Rc};

use crate::{
    bus::Bus,
    slot::{RamSlot, RomSlot, SlotType},
    vdp::DisplayMode,
    TMS9918,
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
        Self::new(&[
            SlotType::Empty,
            SlotType::Empty,
            SlotType::Empty,
            SlotType::Empty,
        ])
    }
}

impl Machine {
    pub fn new(slots: &[SlotType]) -> Self {
        let queue = Rc::new(RefCell::new(Vec::new()));
        let vdp = TMS9918::new(queue.clone());
        let bus = Bus::new(slots, vdp, queue);

        Self {
            bus,
            current_scanline: 0,
        }
    }

    pub fn screen_buffer(&self) -> Vec<u8> {
        // self.bus.borrow().screen_buffer()
        todo!()
    }

    pub fn vram(&self) -> Vec<u8> {
        // self.bus.borrow().vram()
        todo!()
    }

    pub fn load_rom(&mut self, slot: u8, data: &[u8]) {
        // self.bus.borrow_mut().load_rom(slot, data);
        todo!()
    }

    pub fn load_ram(&mut self, slot: u8) {
        // self.bus.borrow_mut().load_ram(slot);
        todo!()
    }

    pub fn load_empty(&mut self, slot: u8) {
        // self.bus.borrow_mut().load_empty(slot);
        todo!()
    }

    pub fn print_memory_page_info(&self) {
        // self.bus.borrow().print_memory_page_info();
        todo!()
    }

    pub fn mem_size(&self) -> usize {
        // FIXME self.cpu.memory.size()
        64 * 1024
    }

    pub fn ram(&self) -> Vec<u8> {
        // let mut memory = Vec::new();
        // for pc in 0..self.mem_size() {
        //     memory.push(self.bus.borrow_mut().read_byte(pc as u16));
        // }
        // memory
        todo!()
    }

    pub fn pc(&self) -> u16 {
        // self.bus.borrow().pc()
        todo!()
    }

    pub fn halted(&self) -> bool {
        // self.bus.borrow().halted()
        todo!()
    }

    pub fn step(&mut self) {
        self.bus.step();
    }

    pub fn primary_slot_config(&self) -> u8 {
        // self.bus.borrow().primary_slot_config()
        todo!()
    }

    pub fn display_mode(&self) -> DisplayMode {
        // self.bus.borrow().display_mode()
        todo!()
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
