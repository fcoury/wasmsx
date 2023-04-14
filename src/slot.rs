use std::{
    fmt::{self, Debug},
    fs::File,
    io::Read,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum SlotType {
    Empty,
    Ram(RamSlot),
    Rom(RomSlot),
}

impl fmt::Display for SlotType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotType::Empty => write!(f, "Empty"),
            SlotType::Ram(slot) => write!(f, "RAM base={:#06X} size={:#06X}", slot.base, slot.size),
            SlotType::Rom(slot) => write!(
                f,
                "ROM path={:?} base={:#06X} size={:#06X}",
                slot.rom_path, slot.base, slot.size
            ),
        }
    }
}

impl SlotType {
    pub fn read(&self, address: u16) -> u8 {
        match self {
            SlotType::Empty => 0xFF,
            SlotType::Ram(slot) => slot.read(address),
            SlotType::Rom(slot) => slot.read(address),
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match self {
            SlotType::Empty => {}
            SlotType::Ram(slot) => slot.write(address, value),
            SlotType::Rom(slot) => slot.write(address, value),
        }
    }

    pub fn size(&self) -> u32 {
        match self {
            SlotType::Empty => 0,
            SlotType::Ram(slot) => slot.size,
            SlotType::Rom(slot) => slot.size,
        }
    }
}

pub trait Slot: Debug {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Clone)]
pub struct RomSlot {
    pub rom_path: Option<PathBuf>,
    pub base: u16,
    pub size: u32,
    pub data: Vec<u8>,
}

impl RomSlot {
    pub fn new(rom: &[u8], base: u16, size: u32) -> Self {
        let mut data = vec![0xFF; size as usize];
        data[0..rom.len()].copy_from_slice(rom);

        if rom.len() < size as usize {
            data[rom.len()..].copy_from_slice(&rom[0..(size as usize - rom.len())]);
        }

        RomSlot {
            base,
            size,
            data,
            rom_path: None,
        }
    }

    pub fn load(rom_path: PathBuf, base: u16, size: u32) -> anyhow::Result<Self> {
        let mut file = File::open(&rom_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut rom_slot = Self::new(&buffer, base, size);
        rom_slot.rom_path = Some(rom_path);

        Ok(rom_slot)
    }

    fn translate_address(&self, address: u16) -> u16 {
        address - self.base
    }
}

impl Slot for RomSlot {
    fn read(&self, address: u16) -> u8 {
        let address = self.translate_address(address);
        if (address as usize) >= self.data.len() {
            // tracing::warn!(
            //     "Attempt to read from out of bounds ROM address {:#06X}, returning 0xFF",
            //     address
            // );
            return 0xFF;
        }
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, _value: u8) {
        tracing::trace!("Attempt to write to ROM address {:#06X}", address);
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Clone)]
pub struct RamSlot {
    pub base: u16,
    pub size: u32,
    pub data: Vec<u8>,
}

impl RamSlot {
    pub fn new(base: u16, size: u32) -> Self {
        let data = vec![0xFF; size as usize];
        RamSlot { base, data, size }
    }

    fn translate_address(&self, address: u16) -> u16 {
        address - self.base
    }
}

impl Slot for RamSlot {
    fn read(&self, address: u16) -> u8 {
        let address = self.translate_address(address);
        if (address as usize) >= self.data.len() {
            tracing::warn!(
                "Attempt to read from out of bounds RAM address {:#06X}, returning 0xFF",
                address
            );
            return 0xFF;
        }
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, value: u8) {
        let address = self.translate_address(address);
        if (address as usize) >= self.data.len() {
            return;
        }
        self.data[address as usize] = value;
    }
}
