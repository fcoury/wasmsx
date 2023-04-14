use std::sync::{Arc, RwLock};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::bus::Bus;

#[derive(Derivative, Serialize, Deserialize)]
#[derivative(Clone, Debug, PartialEq)]
pub struct Memory {
    #[serde(skip)]
    #[derivative(PartialEq = "ignore")]
    pub bus: Arc<RwLock<Bus>>,
    pub data: Vec<u8>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            bus: Arc::new(RwLock::new(Bus::default())),
            data: vec![],
        }
    }
}

impl Memory {
    pub fn new(bus: Arc<RwLock<Bus>>, size: usize) -> Self {
        let data = vec![0xFF; size];

        // let mut data = vec![0xFF; size];

        // fill the addresses from FD9A through FFC9 with C9
        // (0xFD9A..=0xFFC9).for_each(|i| {
        //     data[i] = 0xC9;
        // });

        // (0x8003..=0xF37F).for_each(|i| {
        //     data[i] = 0xFF;
        // });

        Memory { bus, data }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn reset(&mut self) {
        let mut data = vec![0xFF; self.data.len()];

        // fill the addresses from FD9A through FFC9 with C9
        (0xFD9A..=0xFFC9).for_each(|i| {
            data[i] = 0xC9;
        });

        (0x8003..=0xF37F).for_each(|i| {
            data[i] = 0xFF;
        });

        self.data = data;
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            // BIOS ROM
            0x0000..=0x3FFF => self.data[address as usize],
            // Cartidge Slot 1
            0x4000..=0x7FFF => self.data[address as usize],
            // Cartidge Slot 2
            0x8000..=0xBFFF => self.data[address as usize],
            // Main RAM
            0xC000..=0xFFFF => self.data[address as usize],
        }
    }

    pub fn read_signed_byte(&self, addr: u16) -> i8 {
        let unsigned_byte = self.read_byte(addr);
        unsigned_byte as i8
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x3FFF => {
                // Writing to BIOS is typically not allowed
                self.data[address as usize] = value;
                warn!(
                    "Writing to BIOS is not allowed - ${:04X} = ${:02X}",
                    address, value
                );
            }
            0x4000..=0x7FFF => {
                // Writing to BASIC is typically not allowed
                self.data[address as usize] = value;
                warn!("Writing to BASIC is not allowed")
            }
            0x8000..=0xBFFF => {
                // panic!("Writing to cartidge, does nothing")
                match address {
                    0x9800 => {
                        // Write to VDP Data Register (0x98)
                        // Implement VRAM write logic here
                        let mut bus = self
                            .bus
                            .write()
                            .expect("Couldn't obtain a write lock on the bus.");
                        bus.output(0x98, value);
                    }
                    0x9801 => {
                        // Write to VDP Address Register (0x99)
                        // Implement VRAM address setting logic here
                        let mut bus = self
                            .bus
                            .write()
                            .expect("Couldn't obtain a write lock on the bus.");
                        bus.output(0x99, value);
                    }
                    _ => {}
                }
            }
            0xC000..=0xDFFF => self.data[address as usize] = value,
            0xE000..=0xFFFF => {
                self.data[address as usize] = value;
            }
        }
    }

    pub fn load_bios(&mut self, buffer: &[u8]) -> std::io::Result<()> {
        let load_address: u16 = 0x0000;
        for (i, byte) in buffer.iter().enumerate() {
            let address = load_address.wrapping_add(i as u16);
            self.data[address as usize] = *byte;
        }

        Ok(())
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

    #[allow(unused)]
    pub fn load_rom(&mut self, start_address: u16, data: &[u8]) {
        let start = start_address as usize;
        let end = start + data.len();
        self.data[start..end].copy_from_slice(data);
    }
}
