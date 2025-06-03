#![allow(dead_code)]

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::machine::Message;

#[derive(Clone)]
pub struct TMS9918 {
    pub queue: Rc<RefCell<VecDeque<Message>>>,

    // #[serde(with = "BigArray")]
    pub vram: [u8; 0x4000],
    pub data_pre_read: u8, // read-ahead value
    pub registers: [u8; 8],
    pub status: u8,
    pub address: u16,
    pub first_write: Option<u8>,
    // #[serde(with = "BigArray")]
    pub screen_buffer: [u8; 256 * 192],
    pub sprites: [Sprite; 8],
    pub frame: u8,
    pub line: u8,
    pub vblank: bool,
    pub display_mode: DisplayMode,
    pub f: u8,
    pub fh: u8,
    pub sprites_collided: bool,
    pub sprites_invalid: Option<u8>,
    pub sprites_max_computed: u8,

    pub blink_page_duration: u8,
    pub blink_per_line: bool,
    pub blink_even_page: bool,
    pub _blanking_change_pending: bool, // Renamed from blanking_change_pending

    pub layout_table_address: u16,
    pub _layout_table_address_mask: u16, // Renamed from layout_table_address_mask
    pub layout_table_address_mask_set_value: u16,

    pub color_table_address: u16,
    pub color_table_address_mask: u16,

    pub pattern_table_address: u16,
    pub pattern_table_address_mask: u16,
}

impl TMS9918 {
    pub fn new(queue: Rc<RefCell<VecDeque<Message>>>) -> Self {
        Self {
            queue,
            vram: [0; 0x4000],
            data_pre_read: 0,
            registers: [0; 8],
            status: 0,
            address: 0,
            first_write: None,
            screen_buffer: [0; 256 * 192],
            sprites: [Sprite {
                x: 0,
                y: 0,
                pattern: 0,
                color: 0,
                collision: false,
            }; 8],
            frame: 0,
            line: 0,
            vblank: false,
            display_mode: DisplayMode::Graphic1,

            f: 0,
            fh: 0,

            sprites_collided: false,
            sprites_invalid: None,
            sprites_max_computed: 0,

            blink_per_line: false,
            blink_even_page: false,
            blink_page_duration: 0,
            _blanking_change_pending: false,

            layout_table_address: 0,
            _layout_table_address_mask: 0,
            layout_table_address_mask_set_value: 0,

            color_table_address: 0,
            color_table_address_mask: 0,

            pattern_table_address: 0,
            pattern_table_address_mask: 0,
        }
    }

    pub fn new_with_vram(queue: Rc<RefCell<VecDeque<Message>>>, vram: Vec<u8>) -> Self {
        Self {
            queue,
            vram: vram.try_into().unwrap(),
            data_pre_read: 0,
            registers: [0; 8],
            status: 0,
            address: 0,
            first_write: None,
            screen_buffer: [0; 256 * 192],
            sprites: [Sprite {
                x: 0,
                y: 0,
                pattern: 0,
                color: 0,
                collision: false,
            }; 8],
            frame: 0,
            line: 0,
            vblank: false,
            display_mode: DisplayMode::Graphic1,

            f: 0,
            fh: 0,

            sprites_collided: false,
            sprites_invalid: None,
            sprites_max_computed: 0,

            blink_per_line: false,
            blink_even_page: false,
            blink_page_duration: 0,
            _blanking_change_pending: false,

            layout_table_address: 0,
            _layout_table_address_mask: 0,
            layout_table_address_mask_set_value: 0,

            color_table_address: 0,
            color_table_address_mask: 0,

            pattern_table_address: 0,
            pattern_table_address_mask: 0,
        }
    }

    pub fn reset(&mut self) {
        self.vram = [0; 0x4000];
        self.data_pre_read = 0;
        self.registers = [0; 8];
        self.status = 0;
        self.address = 0;
        self.first_write = None;
        self.screen_buffer = [0; 256 * 192];
        self.sprites = [Sprite {
            x: 0,
            y: 0,
            pattern: 0,
            color: 0,
            collision: false,
        }; 8];
        self.frame = 0;
        self.line = 0;
        self.vblank = false;
        self.display_mode = DisplayMode::Graphic1;
        self.f = 0;
        self.fh = 0;
        self.sprites_collided = false;
        self.sprites_invalid = None;
        self.sprites_max_computed = 0;

        self.update_blinking();
        // self.update_color_table_address(); // Called when R3/R10 is written
        self.update_layout_table_address();
    }

    pub fn name_table_base_and_size(&self) -> (usize, usize) {
        match self.display_mode {
            DisplayMode::Text1 => (0x0000, 960),
            DisplayMode::Graphic1 => (0x1800, 768),
            DisplayMode::Graphic2 => (0x1800, 768),
            DisplayMode::Multicolor => (0x0800, 768),
        }
    }

    pub fn char_pattern_table(&self) -> &[u8] {
        let base_address = match self.display_mode {
            DisplayMode::Text1 => (self.registers[4] as usize & 0x07) * 0x800,
            DisplayMode::Graphic1 | DisplayMode::Graphic2 => {
                ((self.registers[4] as usize) & 0x04) << 11
            }
            DisplayMode::Multicolor => 0x0000,
        };

        let size = match self.display_mode {
            DisplayMode::Text1 => 2 * 1024,
            DisplayMode::Graphic1 => 2 * 1024,
            DisplayMode::Graphic2 => 6 * 1024,
            DisplayMode::Multicolor => 1536,
        };

        if base_address + size <= self.vram.len() {
            &self.vram[base_address..(base_address + size)]
        } else {
            error!(
                "Invalid character pattern table range: {:04X} to {:04X}",
                base_address,
                base_address + size
            );
            &self.vram[0..0]
        }
    }

    pub fn color_table(&self) -> &[u8] {
        let ct_base = match self.display_mode {
            DisplayMode::Graphic1 => ((self.registers[3] as usize) & 0x80) << 6,
            DisplayMode::Graphic2 => ((self.registers[3] as usize) & 0x80) << 6,
            _ => 0x2000,
        };

        let ct_table_size = match self.display_mode {
            DisplayMode::Graphic1 => 32,
            DisplayMode::Graphic2 => 0x1800,
            _ => 32,
        };

        if ct_base.saturating_add(ct_table_size) <= self.vram.len() {
            &self.vram[ct_base..(ct_base + ct_table_size)]
        } else {
            tracing::error!(
                "VDP::color_table OOB access: base={:04X}, size={:04X} (mode {:?}), R3={:02X}, calculated ct_base={:04X}. VRAM len={:04X}",
                self.color_table_address, ct_table_size, self.display_mode, self.registers[3], ct_base, self.vram.len()
            );
            &self.vram[0..0] // Fallback to empty slice
        }
    }

    pub fn get_horizontal_scroll_high(&self) -> usize {
        (self.registers[0] as usize & 0x07) * 8
    }

    pub fn get_vertical_scroll(&self) -> usize {
        0
    }

    fn read98(&mut self) -> u8 {
        self.first_write = None;
        let data = self.data_pre_read;
        self.data_pre_read = self.vram[self.address as usize];
        self.address_wrapping_inc();
        data
    }

    pub fn write_98(&mut self, data: u8) {
        if self.address < self.vram.len() as u16 {
            self.vram[self.address as usize] = data;
            self.data_pre_read = data;
        } else {
            error!(
                "Attempted to write to an invalid VRAM address: {:04X}",
                self.address
            );
        }
        self.address = (self.address + 1) & 0x3FFF;
        self.first_write = None;
    }

    fn read99(&mut self) -> u8 {
        let mut res = 0;
        if self.f != 0 {
            res |= 0x80;
            self.f = 0;
            self.update_irq();
        }
        if self.sprites_collided {
            res |= 0x20;
            self.sprites_collided = false;
        }
        if let Some(sprites_invalid) = self.sprites_invalid {
            res |= 0x40 | sprites_invalid;
            self.sprites_invalid = None;
        } else {
            res |= self.sprites_max_computed;
        }
        res
    }

    pub fn update_irq(&self) {
        if self.f != 0 && self.registers[1] & 0x20 != 0
            || self.fh != 0 && self.registers[0] & 0x10 != 0
        {
            tracing::trace!("IRQ ON");
            self.queue.borrow_mut().push_back(Message::EnableInterrupts)
        } else {
            tracing::trace!("IRQ OFF: {:?}", self.queue.borrow());
            self.queue
                .borrow_mut()
                .push_back(Message::DisableInterrupts);
            tracing::trace!("IRQ OFF: {:?}", self.queue.borrow());
        }
    }

    fn set_display_mode(&mut self) {
        let r0 = self.registers[0];
        let r1 = self.registers[1];
        let m1: u8 = (r1 >> 4) & 0b0001;
        let m2: u8 = (r1 >> 3) & 0b0001;
        let m3: u8 = (r0 >> 1) & 0b0001;
        let mx_bits: u8 = (m1 << 2) | (m2 << 1) | m3;

        self.display_mode = match mx_bits {
            0b000 => DisplayMode::Graphic1,
            0b001 => DisplayMode::Graphic2,
            0b010 => DisplayMode::Multicolor,
            0b100 => DisplayMode::Text1,
            _ => {
                tracing::warn!(
                    "[VDP] Unsupported display mode: 0x{:02X} ({:04b})",
                    mx_bits,
                    mx_bits
                );
                DisplayMode::Text1
            }
        };
    }

    fn write_register(&mut self, reg: u8, value: u8) {
        let modified = self.registers[reg as usize] ^ value;
        self.registers[reg as usize] = value;

        match reg {
            0 => {
                if modified & 0x10 != 0 {
                    self.update_irq();
                }
                if modified & 0x0e != 0 {
                    self.set_display_mode();
                }
            }
            1 => {
                if modified & 0x20 != 0 {
                    self.update_irq();
                }
                if modified & 0x40 != 0 {
                    self._blanking_change_pending = true;
                }
                if modified & 0x18 != 0 {
                    self.set_display_mode();
                }
                if modified & 0x04 != 0 {
                    self.update_blinking();
                }
                if modified & 0x03 != 0 {
                    info!(
                        "[VDP] 1 - 0x03 - Update sprites config | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
            }
            2 => {
                if modified & 0x7f != 0 {
                    info!(
                        "[VDP] 2 - 0x7f - Update layout table address | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    self.update_layout_table_address();
                }
            }
            10 => {
                if modified & 0x07 != 0 {
                    info!(
                        "[VDP] 10 - 0x07 - Update color table address (via R10) | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    self.update_color_table_address(ColorTablePart::High(value & 7));
                    self.queue.borrow_mut().push_front(Message::DebugPC);
                }
            }
            3 => {
                info!(
                    "[VDP] 3 - Update color table base address (via R3) | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                self.update_color_table_address(ColorTablePart::Low(value));
                self.queue.borrow_mut().push_back(Message::DebugPC);
            }
            4 => {
                if modified & 0x3f != 0 {
                    info!(
                        "[VDP] 4 - 0x3f - Update pattern table address | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    self.update_pattern_table_address(value);
                }
            }
            5 => {
                info!(
                    "[VDP] 5 - Update sprite attribute table address | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
            }
            11 => {
                info!(
                    "[VDP] 11 - Update sprite attribute table address | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
            }
            6 => {
                if modified & 0x3f != 0 {
                    info!("[VDP] 6 - 0x3f - Update sprite pattern table address | Reg: {} | Value: 0x{:02X}",
                        reg, value);
                }
            }
            7 => {
                let fg = value & 0xF0;
                let bg = value & 0x0F;
                info!("[VDP] 7 - Update backdrop color | FG: {} | BG: {}", fg, bg);
            }
            8 => {
                if modified & 0x20 != 0 {
                    info!(
                        "[VDP] 8 - 0x20 - Update transparency | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x02 != 0 {
                    info!(
                        "[VDP] 8 - 0x02 - Update sprites config | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
            }
            9 => {
                if modified & 0x80 != 0 {
                    info!(
                        "[VDP] 9 - 0x80 - Update signal metrics | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x08 != 0 {
                    info!(
                        "[VDP] 9 - 0x08 - Update render metrics | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x04 != 0 {
                    info!(
                        "[VDP] 9 - 0x04 - Update layout address mask | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x02 != 0 {
                    info!(
                        "[VDP] 9 - 0x02 - Update video standard | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
            }
            13 => {
                info!(
                    "[VDP] 13 - Update blinking | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
            }
            14 => {
                if modified & 0x07 == 0 {
                    // This condition seems inverted in WebMSX code, usually it's `if (modified & MASK)`
                    info!(
                        "[VDP] 14 - 0x07 - Update VRAM pointer | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    self.address = ((value & 0x07) as u16) << 14 | (self.address & 0x3FFF); // This was likely 0x3FFF, not 0x03FF
                    info!("[VDP] Setting VRAM pointer: {:04X}", self.address);
                }
            }
            16 => {
                info!(
                    "[VDP] 16 - Reset palette first write | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
            }
            18 => {
                if modified & 0x0f != 0 {
                    info!(
                        "[VDP] 18 - 0x0f - Horizontal adjust | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0xf0 != 0 {
                    info!(
                        "[VDP] 18 - 0xf0 - Vertical adjust | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
            }
            19 => {
                info!(
                    "[VDP] 19 - Set horizontal int line | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
            }
            23 => {
                info!(
                    "[VDP] 23 - Set horizontal int line | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
            }
            _ => {}
        }
    }

    fn update_blinking(&mut self) {
        self.blink_per_line = self.registers[1] & 0x04 != 0;
        self.blink_even_page = false;
        self.blink_page_duration = 0;
    }

    fn mode_data(&self) -> &ModeData {
        self.display_mode.mode_data()
    }

    fn update_color_table_address(&mut self, part: ColorTablePart) {
        match part {
            ColorTablePart::Low(val_r3) => {
                tracing::debug!(
                    "[VDP] Updating color table base from R3={:02X} for mode {:?}",
                    val_r3,
                    self.display_mode
                );
                match self.display_mode {
                    DisplayMode::Graphic1 => {
                        self.color_table_address =
                            if (val_r3 & 0x80) != 0 { 0x2000 } else { 0x0000 };
                    }
                    DisplayMode::Graphic2 => {
                        self.color_table_address = ((val_r3 as u16) << 6) & 0x3FC0;
                    }
                    DisplayMode::Text1 | DisplayMode::Multicolor => {
                        self.color_table_address = 0x0000;
                    }
                }
            }
            ColorTablePart::High(_val_r10) => {
                tracing::warn!("[VDP] Attempt to set Color Table base via R10 - TMS9918A ignores this for base address.");
            }
        }
        tracing::info!(
            "[VDP] Internal color_table_address set to {:04X}",
            self.color_table_address
        );
        self.color_table_address_mask =
            (self.color_table_address as i16 | COLOR_TABLE_ADDRESS_MASK_BASE) as u16;
    }

    fn update_layout_table_address(&mut self) {
        let add = self.registers[2] as i16 & 0x7f << 10;
        self.layout_table_address = (add & -1024) as u16; // -1024 is 0xFFFFFC00, effectively (add & 0xFC00) when masked
        self.layout_table_address_mask_set_value = (add | LAYOUT_TABLE_ADDRESS_MASK_BASE) as u16;
    }

    fn update_pattern_table_address(&mut self, val: u8) {
        let add: u16 = ((val as u16) << 11) & 0x1fff; // This mask is 0x3FFF for TMS9918A, but & 0x1FFF is what WebMSX uses for G1/MC
        self.pattern_table_address = (add as i16 & self.mode_data().pattern_t_base) as u16;
        self.pattern_table_address_mask = (add as i16 | PATTERN_TABLE_ADDRESS_MASK_BASE) as u16;
    }

    fn write_99(&mut self, val: u8) {
        let Some(data_first_write) = self.first_write else {
            self.first_write = Some(val);
            self.address = (self.address & !0xFF) | val as u16;
            return;
        };

        if val & 0x80 != 0 {
            let reg = val & 0x07;
            self.write_register(reg, data_first_write);
            self.address =
                ((self.address & 0x00FF) | ((data_first_write as u16 & 0x03F) << 8)) & 0x3FFF;
            info!("");
        } else {
            self.address = (((val & 0x3f) as u16) << 8) | (data_first_write as u16) & 0x3FFF;
            if (val & 0x40) == 0 {
                self.data_pre_read = self.vram[self.address as usize];
                self.address = (self.address + 1) & 0x3FFF;
            }
        }
        self.first_write = None;
    }

    pub fn address_wrapping_inc(&mut self) {
        self.address = (self.address + 1) & 0x3FFF;
    }

    pub fn read(&mut self, port: u8) -> u8 {
        match port {
            0x98 => self.read98(),
            0x99 => self.read99(),
            _ => {
                error!("Invalid port: {:02X}", port);
                0xFF
            }
        }
    }

    pub fn write(&mut self, port: u8, data: u8) {
        match port {
            0x98 => self.write_98(data),
            0x99 => self.write_99(data),
            _ => {
                error!("Invalid port: {:02X}", port);
            }
        }
    }

    pub fn pulse(&mut self) {
        if self.f == 0 {
            self.f = 1;
            self.update_irq();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisplayMode {
    Text1,
    Graphic1,
    Graphic2,
    Multicolor,
}

impl DisplayMode {
    fn mode_data(&self) -> &ModeData {
        match self {
            DisplayMode::Text1 => &MODE_DATA_TEXT1,
            DisplayMode::Graphic1 => &MODE_DATA_GRAPHIC1,
            DisplayMode::Graphic2 => &MODE_DATA_GRAPHIC2,
            DisplayMode::Multicolor => &MODE_DATA_MULTICOLOR,
        }
    }
}

struct ModeData {
    color_t_base: i16,
    pattern_t_base: i16,
    sprite_attr_t_base: i16,
    sprite_mode: u8,
    text_cols: u8,
}

impl ModeData {
    pub fn new(
        color_t_base: i16,
        pattern_t_base: i16,
        sprite_attr_t_base: i16,
        sprite_mode: u8,
        text_cols: u8,
    ) -> Self {
        Self {
            color_t_base,
            pattern_t_base,
            sprite_attr_t_base,
            sprite_mode,
            text_cols,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sprite {
    pub x: u8,
    pub y: u8,
    pub pattern: u32,
    pub color: u8,
    pub collision: bool,
}

enum ColorTablePart {
    High(u8),
    Low(u8),
}

const MODE_DATA_TEXT1: ModeData = ModeData {
    color_t_base: 0x0000,
    pattern_t_base: 0x0000, // WebMSX uses -1 << 11, but for Text1 patterns are more fixed/simple
    sprite_attr_t_base: 0x0000,
    sprite_mode: 0,
    text_cols: 40,
};

const MODE_DATA_GRAPHIC1: ModeData = ModeData {
    color_t_base: -1 << 6, // All bits relevant for address calculation, but only bit 7 of R3 matters for base
    pattern_t_base: -1 << 11,
    sprite_attr_t_base: -1 << 7,
    sprite_mode: 1,
    text_cols: 32,
};

const MODE_DATA_GRAPHIC2: ModeData = ModeData {
    color_t_base: -1 << 13, // R3.0-6 must be 1, R3.7 for base
    pattern_t_base: -1 << 13,
    sprite_attr_t_base: -1 << 7, // WebMSX uses -1 << 7, R5/R11 relevant
    sprite_mode: 1,
    text_cols: 0, // Not applicable text columns like T1
};

const MODE_DATA_MULTICOLOR: ModeData = ModeData {
    color_t_base: 0x0000, // Color info is in pattern table for MC
    pattern_t_base: -1 << 11,
    sprite_attr_t_base: -1 << 7,
    sprite_mode: 1,
    text_cols: 0, // Not applicable
};

const COLOR_TABLE_ADDRESS_MASK_BASE: i16 = !(-1 << 6);
const LAYOUT_TABLE_ADDRESS_MASK_BASE: i16 = !(-1 << 10);
const PATTERN_TABLE_ADDRESS_MASK_BASE: i16 = !(-1 << 11);
