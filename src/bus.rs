use std::{cell::RefCell, collections::VecDeque, fmt, rc::Rc};

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;
use z80::Z80_io;

use super::{fdc::WD2793, ppi::Ppi, psg::AY38910, vdp::TMS9918};
use crate::{
    machine::Message,
    slot::{RamSlot, RomSlot, SlotType},
};

pub struct Bus {
    // I/O Devices
    pub vdp: TMS9918,
    pub psg: AY38910,
    pub ppi: Ppi,
    pub fdc: Option<WD2793>,

    slots: [SlotType; 4],
}

impl Bus {
    pub fn new(slots: &[SlotType], queue: Rc<RefCell<VecDeque<Message>>>) -> Self {
        if slots.len() != 4 {
            panic!("Bus requires exactly 4 slots, got {}", slots.len());
        }
        
        Self {
            vdp: TMS9918::new(queue),
            psg: AY38910::new(),
            ppi: Ppi::new(),
            fdc: None,  // FDC is optional, can be enabled later
            slots: [
                slots[0].clone(),
                slots[1].clone(),
                slots[2].clone(),
                slots[3].clone(),
            ],
        }
    }

    pub fn key_down(&mut self, key: String) {
        self.ppi.key_down(key);
    }

    pub fn key_up(&mut self, key: String) {
        self.ppi.key_up(key);
    }

    pub fn mem_size(&self) -> usize {
        0x10000
    }

    pub fn reset(&mut self) {
        self.vdp.reset();
        self.psg.reset();
        self.ppi.reset();
        if let Some(fdc) = &mut self.fdc {
            fdc.reset();
        }
    }
    
    pub fn enable_fdc(&mut self) {
        self.fdc = Some(WD2793::new());
    }

    pub fn input(&mut self, port: u8) -> u8 {
        match port {
            0x98 | 0x99 => self.vdp.read(port),
            0xA0 | 0xA1 => self.psg.read(port),
            0xA8 | 0xA9 | 0xAA | 0xAB => self.ppi.read(port),
            0x7C..=0x7F => {
                if let Some(fdc) = &mut self.fdc {
                    fdc.read(port)
                } else {
                    0xFF
                }
            }
            _ => {
                tracing::trace!("[BUS] Invalid port {:02X} read", port);
                0xff
            }
        }
    }

    pub fn output(&mut self, port: u8, data: u8) {
        match port {
            0x98 | 0x99 => self.vdp.write(port, data),
            0xA0 | 0xA1 => self.psg.write(port, data),
            0xA8 | 0xA9 | 0xAA | 0xAB => self.ppi.write(port, data),
            0x7C..=0x7F => {
                if let Some(fdc) = &mut self.fdc {
                    fdc.write(port, data);
                }
            }
            0xFB => {
                // Drive control port (0x7FFB mirrored to 0xFB in 8-bit I/O space)
                if let Some(fdc) = &mut self.fdc {
                    fdc.drive_control(data);
                }
            }
            _ => {
                tracing::trace!("[BUS] Invalid port {:02X} write", port);
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

    pub fn primary_slot_config(&self) -> u8 {
        self.ppi.primary_slot_config
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
            let slot_number = ((self.ppi.primary_slot_config >> (page * 2)) & 0x03) as usize;
            let slot_type = self.slots.get(slot_number).unwrap();

            println!(
                "Memory page {} (0x{:04X} - 0x{:04X}): primary slot {} ({})",
                page, start_address, end_address, slot_number, slot_type
            );
        }
    }

    pub fn set_irq(on: bool) {
        if on {}
    }

    pub fn memory_segments(&self) -> Vec<MemorySegment> {
        let s = self.ppi.primary_slot_config;
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
//         let mut bus = Bus::new(&[
//             SlotType::Rom(RomSlot::new(&[0; 0x8000], 0x0000, 0x8000)),
//             SlotType::Empty,
//             SlotType::Empty,
//             SlotType::Ram(RamSlot::new(0x0000, 0x10000)),
//         ]);

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
//         let mut bus = Bus::new(&[
//             SlotType::Rom(RomSlot::new(&[0; 0x8000], 0x0000, 0x8000)),
//             SlotType::Empty,
//             SlotType::Empty,
//             SlotType::Ram(RamSlot::new(0x0000, 0xFFFF)),
//         ]);

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
