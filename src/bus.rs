use std::{cell::RefCell, collections::VecDeque, fmt, rc::Rc};

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;
use z80::Z80_io;

use super::{ppi::Ppi, psg::AY38910, vdp::TMS9918};
use crate::{
    machine::Message,
    slot::{RamSlot, RomSlot, SlotType},
};

pub struct Bus {
    // I/O Devices
    pub vdp: TMS9918,
    pub psg: AY38910,
    pub ppi: Ppi,

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
            slots: [
                slots[0].clone(),
                slots[1].clone(),
                slots[2].clone(),
                slots[3].clone(),
            ],
        }
    }

    pub fn key_down(&mut self, key: String) {
        // Send key press to both keyboard and joystick handlers
        self.ppi.key_down(key.clone());
        self.psg.joystick_key_down(key);
    }

    pub fn key_up(&mut self, key: String) {
        // Send key release to both keyboard and joystick handlers
        self.ppi.key_up(key.clone());
        self.psg.joystick_key_up(key);
    }

    pub fn mem_size(&self) -> usize {
        0x10000
    }

    pub fn reset(&mut self) {
        self.vdp.reset();
        self.psg.reset();
        self.ppi.reset();
    }

    pub fn clock(&mut self, cycles: u32) {
        // Clock the PSG for audio generation
        self.psg.clock(cycles);
    }

    pub fn update_psg_pulse_signal(&mut self) {
        // Update PSG pulse signal based on PPI register C bits 5 and 7
        let pulse_active = (self.ppi.register_c() & 0xa0) != 0;
        self.psg.set_pulse_signal(pulse_active);
    }

    pub fn get_slot(&self, slot: usize) -> &SlotType {
        &self.slots[slot]
    }

    pub fn get_slot_mut(&mut self, slot: usize) -> &mut SlotType {
        &mut self.slots[slot]
    }

    pub fn input(&mut self, port: u8) -> u8 {
        if (0x7C..=0x7F).contains(&port) || (0xD0..=0xDF).contains(&port) {
            let ppi_a8 = self.ppi.primary_slot_config;
            tracing::warn!(
                "[FDC I/O Port Check - INPUT] Port {:02X}. PPI A8: {:02X} (P0:{:X}, P1:{:X}, P2:{:X}, P3:{:X})",
                port, ppi_a8,
                ppi_a8 & 0x03, (ppi_a8 >> 2) & 0x03,
                (ppi_a8 >> 4) & 0x03, (ppi_a8 >> 6) & 0x03
            );
        }
        match port {
            0x98 | 0x99 => self.vdp.read(port),
            0xA0 | 0xA1 | 0xA2 => self.psg.read(port),
            0xA8 => self.ppi.read(port), // Primary slot config
            0xA9 => {
                // Special handling for keyboard port (0xA9)
                // This is where we implement the multiplexing between keyboard and joystick

                // First, get the keyboard state from PPI
                let keyboard_state = self.ppi.read(port);

                // If we're reading row 8 (where space bar is located), we need to combine with joystick
                if self.ppi.keyboard_row_selected() == 8 {
                    // Get joystick state from PSG (bit 4 is fire button/space)
                    let joystick_state = self.psg.joystick_port_a;

                    // If space is pressed on joystick (bit 4 is 0), clear bit 0 in keyboard state
                    // This simulates the space key being pressed in row 8
                    if (joystick_state & (1 << 4)) == 0 {
                        tracing::info!(
                            "[BUS] Multiplexing joystick space to keyboard: KB:{:08b}, Joy:{:08b}, Result:{:08b}",
                            keyboard_state,
                            joystick_state,
                            keyboard_state & !(1 << 0)
                        );
                        return keyboard_state & !(1 << 0);
                    }
                }

                keyboard_state
            }
            0xAA | 0xAB => self.ppi.read(port), // Other PPI ports
            _ => {
                // Only log disk-related ports
                if (0x7C..=0x7F).contains(&port)
                    || port == 0xFB
                    || port == 0xD8
                    || (0xD0..=0xD7).contains(&port)
                {
                    tracing::info!("[BUS] Read from unmapped disk-related port {:02X}", port);
                } else if port != 0xA2 {
                    // Don't spam log for A2
                    tracing::trace!("[BUS] Invalid port {:02X} read", port);
                }
                0xff
            }
        }
    }

    pub fn output(&mut self, port: u8, data: u8) {
        if (0x7C..=0x7F).contains(&port)
            || (0xD0..=0xDF).contains(&port)
            || port == 0xD8
            || port == 0xFB
        {
            let ppi_a8 = self.ppi.primary_slot_config;
            tracing::warn!(
                "[FDC I/O Port Check - OUTPUT] Port {:02X} <- {:02X}. PPI A8: {:02X} (P0:{:X}, P1:{:X}, P2:{:X}, P3:{:X})",
                port, data, ppi_a8,
                ppi_a8 & 0x03, (ppi_a8 >> 2) & 0x03,
                (ppi_a8 >> 4) & 0x03, (ppi_a8 >> 6) & 0x03
            );
        }

        match port {
            0x98 | 0x99 => self.vdp.write(port, data),
            0xA0 | 0xA1 => self.psg.write(port, data),
            0xA2 => {
                // Port 0xA2 is read-only for PSG, writes are ignored
                tracing::trace!("[BUS] Ignored write to PSG read port 0xA2: {:02X}", data);
            }
            0xA8 => {
                // PPI Port A (Slot select)
                self.ppi.write(port, data);
            }
            0xA9 => {
                // PPI Port B (Keyboard)
                self.ppi.write(port, data);
            }
            0xAA | 0xAB => {
                // PPI Port C or Control
                let old_register_c = self.ppi.register_c();
                self.ppi.write(port, data);
                // Update PSG pulse signal if register C changed
                if port == 0xAA || (port == 0xAB && old_register_c != self.ppi.register_c()) {
                    self.update_psg_pulse_signal();
                }
            }
            0xFB => {
                // Standard drive control port (0x7FFB mirrored to 0xFB in 8-bit I/O space)

                tracing::info!(
                    "[FDC I/O Write] Port {:02X} (Std Drive Ctrl) <- {:02X}",
                    port,
                    data
                );
            }
            _ => {
                // Only log disk-related ports
                if (0x7C..=0x7F).contains(&port)
                    || port == 0xFB
                    || port == 0xD8
                    || (0xD0..=0xD7).contains(&port)
                {
                    tracing::info!(
                        "[BUS] Write to unmapped disk-related port {:02X} = {:02X}",
                        port,
                        data
                    );
                } else {
                    tracing::trace!("[BUS] Invalid port {:02X} write = {:02X}", port, data);
                }
            }
        };
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        let (slot_number, addr) = self.translate_address(addr);
        let value = self.slots[slot_number].read(addr);

        value
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

    pub fn write_block(&mut self, start_addr: u16, data: &[u8]) {
        // Temporary trace for MSX FILES diagnosis
        if start_addr == 0xEBAC {
            let dump_len = data.len().min(32);
            let slice = &data[..dump_len];
            let hex: Vec<String> = slice.iter().map(|b| format!("{:02X}", b)).collect();
            tracing::info!(
                "[TRACE] Bus::write_block dumping first {} bytes to 0xEBAC: {}",
                dump_len,
                hex.join(" ")
            );
        }

        let mut addr = start_addr;
        for &byte in data {
            self.write_byte(addr, byte);
            addr = addr.wrapping_add(1);
        }
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
                // The address passed to the slot should be the full address
                // Each slot will handle its own address translation based on its base
                return (segment.slot as usize, address);
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
        if let SlotType::Rom(rom_slot) = &mut self.slots[slot as usize] {
            rom_slot.load(rom);
        } else {
            self.slots[slot as usize] = SlotType::Rom(RomSlot::new(rom, 0x0000, 0x10000));
        }
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
