use std::{cell::RefCell, fmt, rc::Rc};

use serde::{Deserialize, Serialize};
use tracing::error;
use wasm_bindgen::prelude::wasm_bindgen;
use z80::Z80_io;

use super::{ppi::Ppi, sound::AY38910, vdp::TMS9918};
use crate::{
    cpu::Cpu,
    slot::{RamSlot, RomSlot, SlotType},
    vdp::DisplayMode,
    Renderer,
};

#[derive(Clone, Debug)]
pub struct Bus {
    // I/O Devices
    pub cpu: Rc<RefCell<Cpu>>,
    pub vdp: Rc<RefCell<TMS9918>>,
    pub ppi: Rc<RefCell<Ppi>>,
    pub psg: Rc<RefCell<AY38910>>,

    slots: [SlotType; 4],
}

impl Bus {
    pub fn new(slots: &[SlotType]) -> Self {
        let bus = Rc::new(RefCell::new(Bus {
            slots: [
                slots.get(0).unwrap().clone(),
                slots.get(1).unwrap().clone(),
                slots.get(2).unwrap().clone(),
                slots.get(3).unwrap().clone(),
            ],
            cpu: Rc::new(RefCell::new(Cpu::new())),
            vdp: Rc::new(RefCell::new(TMS9918::new())),
            ppi: Rc::new(RefCell::new(Ppi::new())),
            psg: Rc::new(RefCell::new(AY38910::new())),
        }));

        bus.borrow_mut()
            .cpu
            .borrow_mut()
            .set_bus(Rc::downgrade(&bus));
        bus.borrow_mut()
            .vdp
            .borrow_mut()
            .set_bus(Rc::downgrade(&bus));

        Rc::try_unwrap(bus).unwrap().into_inner()
    }

    pub fn mem_size(&self) -> usize {
        0x10000
    }

    // pub fn send_message(&self, message: BusMessage) {
    //     match message {}
    // }

    pub fn reset(&mut self) {
        self.vdp.borrow_mut().reset();
        self.psg.borrow_mut().reset();
        self.ppi.borrow_mut().reset();
    }

    pub fn input(&mut self, port: u8) -> u8 {
        match port {
            0x98 | 0x99 => self.vdp.borrow_mut().read(port),
            0xA0 | 0xA1 => self.psg.borrow_mut().read(port),
            0xA8 | 0xA9 | 0xAA | 0xAB => self.ppi.borrow_mut().read(port),
            _ => {
                error!("[BUS] Invalid port {:02X} read", port);
                0xff
            }
        }
    }

    pub fn output(&mut self, port: u8, data: u8) {
        match port {
            0x98 | 0x99 => self.vdp.borrow_mut().write(port, data),
            0xA0 | 0xA1 => self.psg.borrow_mut().write(port, data),
            0xA8 | 0xA9 | 0xAA | 0xAB => self.ppi.borrow_mut().write(port, data),
            _ => {
                error!("[BUS] Invalid port {:02X} write", port);
            }
        };
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        let (slot_number, addr) = self.translate_address(addr);
        self.slots[slot_number].read(addr)
    }

    pub fn write_byte(&mut self, addr: u16, data: u8) {
        let (slot_number, addr) = self.translate_address(addr);
        self.slots[slot_number].write(addr, data);
    }

    pub fn write_word(&mut self, address: u16, value: u16) {
        let low_byte = (value & 0x00FF) as u8;
        let high_byte = ((value & 0xFF00) >> 8) as u8;
        self.write_byte(address, low_byte);
        self.write_byte(address + 1, high_byte);
    }

    pub fn read_word(&self, address: u16) -> u16 {
        let low_byte = self.read_byte(address) as u16;
        let high_byte = self.read_byte(address + 1) as u16;
        (high_byte << 8) | low_byte
    }

    pub fn set_irq(&self, irq: bool) {
        if irq {
            self.cpu.borrow_mut().set_irq();
        } else {
            self.cpu.borrow_mut().clear_irq();
        }
    }

    pub fn primary_slot_config(&self) -> u8 {
        self.ppi.borrow().primary_slot_config
    }

    pub fn translate_address(&self, address: u16) -> (usize, u16) {
        let segments = self.memory_segments();
        for segment in &segments {
            if address >= segment.start && address <= segment.end {
                return (segment.slot as usize, address);
                // let relative_address = address - segment.start + segment.base;
                // return (segment.slot as usize, relative_address);
            }
        }

        // This should not be reached; if it is, there's an issue with the memory segments.
        panic!("Address not found in any segment: {:#x}", address);
    }

    pub fn print_memory_page_info(&self) {
        for page in 0..4 {
            let start_address = page * 0x4000;
            let end_address = start_address + 0x3FFF;
            let slot_number =
                ((self.ppi.borrow().primary_slot_config >> (page * 2)) & 0x03) as usize;
            let slot_type = self.slots.get(slot_number).unwrap();

            println!(
                "Memory page {} (0x{:04X} - 0x{:04X}): primary slot {} ({})",
                page, start_address, end_address, slot_number, slot_type
            );
        }
    }

    pub fn memory_segments(&self) -> Vec<MemorySegment> {
        let s = self.ppi.borrow().primary_slot_config;
        let mut c: Option<MemorySegment> = None;
        let mut rolling_bases = [0; 4];

        let mut segments = Vec::new();
        for n in 0..4 {
            let pos = n;
            let slot = (s >> (pos * 2)) & 0b11;
            let start = n * 16 * 1024;

            c = match c {
                Some(c) => {
                    if c.slot != slot {
                        segments.push(c);
                        Some(MemorySegment {
                            base: rolling_bases[slot as usize],
                            start,
                            end: (start as u32 + 16 * 1024 - 1) as u16,
                            slot,
                        })
                    } else {
                        Some(MemorySegment {
                            base: c.base,
                            start: c.start,
                            end: (start as u32 + 16 * 1024 - 1) as u16,
                            slot,
                        })
                    }
                }
                None => Some(MemorySegment {
                    base: rolling_bases[slot as usize],
                    start,
                    end: (start as u32 + 16 * 1024 - 1) as u16,
                    slot,
                }),
            };

            rolling_bases[slot as usize] = (start as u32 + 16 * 1024) as u16;
        }
        segments.push(c.unwrap());
        segments
    }

    pub fn load_rom(&mut self, slot: u8, rom: &[u8]) {
        // self.slots[slot as usize] = SlotType::Rom(RomSlot::new(rom, 0x0000, rom.len() as u32));
        self.slots[slot as usize] = SlotType::Rom(RomSlot::new(rom, 0x0000, 0x10000));
    }

    pub fn load_ram(&mut self, slot: u8) {
        self.slots[slot as usize] = SlotType::Ram(RamSlot::new(0x0000, 0x10000));
    }

    pub fn load_empty(&mut self, slot: u8) {
        self.slots[slot as usize] = SlotType::Empty;
    }

    pub fn screen_buffer(&self) -> Vec<u8> {
        let vdp = self.vdp.borrow();
        let mut renderer = Renderer::new(&vdp);
        renderer.draw();
        renderer.screen_buffer.to_vec()
    }

    pub fn vram(&self) -> Vec<u8> {
        self.vdp.borrow().vram.to_vec()
    }

    pub fn pc(&self) -> u16 {
        self.cpu.borrow().pc()
    }

    pub fn halted(&self) -> bool {
        self.cpu.borrow().halted()
    }

    pub fn step(&mut self) {
        self.cpu.borrow_mut().step();
    }

    pub fn display_mode(&self) -> DisplayMode {
        self.vdp.borrow().display_mode.clone()
    }
}

impl Z80_io for Bus {
    fn read_byte(&self, addr: u16) -> u8 {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, data: u8) {
        self.write_byte(addr, data);
    }
}

#[wasm_bindgen]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MemorySegment {
    base: u16,
    start: u16,
    end: u16,
    slot: u8,
}

impl fmt::Display for MemorySegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "0x{:04X} - 0x{:04X} - base: 0x{:04X} - (slot {})",
            self.start, self.end, self.base, self.slot
        )
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::slot::{RamSlot, RomSlot};

//     use super::*;

//     #[test]
//     fn test_slot_definition() {
//         let bus = Bus::new(&[
//             SlotType::Rom(RomSlot::new(&[0; 0x8000], 0x0000, 0x8000)),
//             SlotType::Empty,
//             SlotType::Empty,
//             SlotType::Ram(RamSlot::new(0x0000, 0x10000)),
//         ]);
//         let mut bus = bus.borrow_mut();

//         bus.ppi.primary_slot_config = 0b00_11_00_00;
//         let segments = bus.memory_segments();
//         assert_eq!(segments.len(), 3);
//         let segment = segments.get(0).unwrap();
//         assert_eq!(segment.slot, 0);
//         assert_eq!(segment.start, 0x0000);
//         assert_eq!(segment.end, 0x7FFF);
//         let segment = segments.get(1).unwrap();
//         assert_eq!(segment.slot, 3);
//         assert_eq!(segment.start, 0x8000);
//         assert_eq!(segment.end, 0xBFFF);
//         let segment = segments.get(2).unwrap();
//         assert_eq!(segment.slot, 0);
//         assert_eq!(segment.start, 0xC000);
//         assert_eq!(segment.end, 0xFFFF);
//         assert_eq!(segments.get(3), None);

//         bus.ppi.primary_slot_config = 0b0000000;
//         let segments = bus.memory_segments();
//         assert_eq!(segments.len(), 1);
//         let segment = segments.get(0).unwrap();
//         assert_eq!(segment.slot, 0);
//         assert_eq!(segment.start, 0x0000);
//         assert_eq!(segment.end, 0xFFFF);
//         assert_eq!(segments.get(1), None);
//         assert_eq!(segments.get(2), None);
//         assert_eq!(segments.get(3), None);

//         bus.ppi.primary_slot_config = 0b11_11_00_00;
//         let segments = bus.memory_segments();
//         assert_eq!(segments.len(), 2);
//         let segment = segments.get(0).unwrap();
//         assert_eq!(segment.slot, 0);
//         assert_eq!(segment.start, 0x0000);
//         assert_eq!(segment.end, 0x7FFF);
//         let segment = segments.get(1).unwrap();
//         assert_eq!(segment.slot, 3);
//         assert_eq!(segment.start, 0x8000);
//         assert_eq!(segment.end, 0xFFFF);
//         assert_eq!(segments.get(2), None);
//         assert_eq!(segments.get(3), None);

//         bus.ppi.primary_slot_config = 0b00_00_11_01;
//         let segments = bus.memory_segments();
//         assert_eq!(segments.len(), 3);
//         let segment = segments.get(0).unwrap();
//         assert_eq!(segment.slot, 1);
//         assert_eq!(segment.start, 0x0000);
//         assert_eq!(segment.end, 0x3FFF);
//         let segment = segments.get(1).unwrap();
//         assert_eq!(segment.slot, 3);
//         assert_eq!(segment.start, 0x4000);
//         assert_eq!(segment.end, 0x7FFF);
//         let segment = segments.get(2).unwrap();
//         assert_eq!(segment.slot, 0);
//         assert_eq!(segment.start, 0x8000);
//         assert_eq!(segment.end, 0xFFFF);
//         assert_eq!(segments.get(3), None);

//         bus.ppi.primary_slot_config = 0b01_11_10_11;
//         let segments = bus.memory_segments();
//         assert_eq!(segments.len(), 4);
//         let segment = segments.get(0).unwrap();
//         assert_eq!(segment.slot, 3);
//         assert_eq!(segment.start, 0x0000);
//         assert_eq!(segment.end, 0x3FFF);
//         let segment = segments.get(1).unwrap();
//         assert_eq!(segment.slot, 2);
//         assert_eq!(segment.start, 0x4000);
//         assert_eq!(segment.end, 0x7FFF);
//         let segment = segments.get(2).unwrap();
//         assert_eq!(segment.slot, 3);
//         assert_eq!(segment.start, 0x8000);
//         assert_eq!(segment.end, 0xBFFF);
//         let segment = segments.get(3).unwrap();
//         assert_eq!(segment.slot, 1);
//         assert_eq!(segment.start, 0xC000);
//         assert_eq!(segment.end, 0xFFFF);
//     }

//     #[test]
//     fn test_address_translation() {
//         let bus = Bus::new(&[
//             SlotType::Rom(RomSlot::new(&[0; 0x8000], 0x0000, 0x8000)),
//             SlotType::Empty,
//             SlotType::Empty,
//             SlotType::Ram(RamSlot::new(0x0000, 0xFFFF)),
//         ]);
//         let mut bus = bus.borrow_mut();

//         bus.ppi.primary_slot_config = 0b00_11_00_00;
//         assert_eq!(bus.translate_address(0x0000), (0, 0x0000));
//         assert_eq!(bus.translate_address(0x4000), (0, 0x4000));
//         assert_eq!(bus.translate_address(0x8000), (3, 0x0000));
//         assert_eq!(bus.translate_address(0xC000), (0, 0x8000));

//         bus.ppi.primary_slot_config = 0b00_00_00_00;
//         assert_eq!(bus.translate_address(0x0FFF), (0, 0x0FFF));
//         assert_eq!(bus.translate_address(0x4FFF), (0, 0x4FFF));
//         assert_eq!(bus.translate_address(0x8FFF), (0, 0x8FFF));
//         assert_eq!(bus.translate_address(0xFFFF), (0, 0xFFFF));

//         bus.ppi.primary_slot_config = 0b11_11_00_00;
//         assert_eq!(bus.translate_address(0x0FFF), (0, 0x0FFF));
//         assert_eq!(bus.translate_address(0x4FFF), (0, 0x4FFF));
//         assert_eq!(bus.translate_address(0x8FFF), (3, 0x0FFF));
//         assert_eq!(bus.translate_address(0xCFFF), (3, 0x4FFF));

//         bus.ppi.primary_slot_config = 0b11_11_01_00;
//         assert_eq!(bus.translate_address(0x0FFF), (0, 0x0FFF));
//         assert_eq!(bus.translate_address(0x4FFF), (1, 0x0FFF));
//         assert_eq!(bus.translate_address(0x8FFF), (3, 0x0FFF));
//         assert_eq!(bus.translate_address(0xFFFF), (3, 0x7FFF));

//         bus.ppi.primary_slot_config = 0b11_11_01_10;
//         assert_eq!(bus.translate_address(0x0FFF), (2, 0x0FFF));
//         assert_eq!(bus.translate_address(0x4FFF), (1, 0x0FFF));
//         assert_eq!(bus.translate_address(0x8FFF), (3, 0x0FFF));
//         assert_eq!(bus.translate_address(0xFFFF), (3, 0x7FFF));
//     }
// }
