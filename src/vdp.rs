#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use tracing::{error, info};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Sprite {
    pub x: u8,
    pub y: u8,
    pub pattern: u32,
    pub color: u8,
    pub collision: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DisplayMode {
    Text1,      // screen 0 - 40x80 text
    Graphic1,   // screen 1 - 32x64 multicolor
    Graphic2,   // screen 2 - 256x192 4-color
    Multicolor, // screen 3 - 256x192 16-color
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TMS9918 {
    #[serde(with = "BigArray")]
    pub vram: [u8; 0x4000],
    pub data_pre_read: u8, // read-ahead value
    pub registers: [u8; 8],
    pub status: u8,
    pub address: u16,
    pub first_write: Option<u8>,
    #[serde(with = "BigArray")]
    pub screen_buffer: [u8; 256 * 192],
    pub sprites: [Sprite; 8],
    pub frame: u8,
    pub line: u8,
    pub vblank: bool,
    pub display_mode: DisplayMode,
}

impl Default for TMS9918 {
    fn default() -> Self {
        Self {
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
            display_mode: DisplayMode::Text1,
        }
    }
}

impl TMS9918 {
    pub fn new() -> Self {
        Self::default()
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
    }

    pub fn name_table_base_and_size(&self) -> (usize, usize) {
        // Calculate the base address of the name table using register R#2
        // let nt_base = (self.registers[2] as usize & 0x0F) * 0x0400;
        // let nt_table_size = 1024;
        // &self.vram[nt_base..(nt_base + nt_table_size)]

        // returns the name table based on the MSX Red Book definition
        match self.display_mode {
            DisplayMode::Text1 => (0x0000, 960),
            DisplayMode::Graphic1 => (0x1800, 768),
            DisplayMode::Graphic2 => (0x1800, 768),
            DisplayMode::Multicolor => (0x0800, 768),
        }
    }

    // pub fn name_table(&self) -> &[u8] {
    //     // Calculate the base address of the name table using register R#2
    //     // let nt_base = (self.registers[2] as usize & 0x0F) * 0x0400;
    //     // let nt_table_size = 1024;
    //     // &self.vram[nt_base..(nt_base + nt_table_size)]

    //     // returns the name table based on the MSX Red Book definition
    //     let (base_address, end_address) = match self.display_mode {
    //         DisplayMode::Text1 => (0x0000, 0x03BF),
    //         DisplayMode::Graphic1 => (0x1800, 0x1AFF),
    //         DisplayMode::Graphic2 => (0x1800, 0x1AFF),
    //         DisplayMode::Multicolor => (0x0800, 0x0AFF),
    //     };

    //     &self.vram[base_address..=end_address]
    // }

    // Character Pattern Table Base Address = register 2 * 0x400
    pub fn char_pattern_table(&self) -> &[u8] {
        let base_address = match self.display_mode {
            DisplayMode::Text1 => 0x0800,
            DisplayMode::Graphic1 => 0x0000,
            DisplayMode::Graphic2 => 0x0000,
            DisplayMode::Multicolor => 0x0000,
        };

        let size = match self.display_mode {
            DisplayMode::Text1 => 2 * 1024,
            DisplayMode::Graphic1 => 2 * 1024,
            DisplayMode::Graphic2 => 6 * 1024,
            DisplayMode::Multicolor => 1536,
        };

        &self.vram[base_address..(base_address + size)]
    }

    pub fn color_table(&self) -> &[u8] {
        // Calculate the base address of the color table using register R#3
        let ct_base = (self.registers[3] as usize & 0x7F) * 0x040;
        info!("[VDP] calculated color table base_address: {:04X}", ct_base);
        let ct_base = 0x2000;
        info!("[VDP] color table base_address: {:04X}", ct_base);
        let ct_table_size = 6 * 1027; // 6k
                                      // tracing::info!("color table base_address: {:04X}", ct_base);
        &self.vram[ct_base..(ct_base + ct_table_size)]
    }

    pub fn get_horizontal_scroll_high(&self) -> usize {
        // Calculate the horizontal scroll value using register R#0
        (self.registers[0] as usize & 0x07) * 8
    }

    pub fn vram_read_np(&self, address: usize) -> usize {
        self.vram[address & 0x3FFF] as usize
    }

    pub fn get_vertical_scroll(&self) -> usize {
        // Replace with the correct logic to get the vertical scroll value
        0
    }

    // WebMSX input98
    fn read_vram(&mut self) -> u8 {
        // reset the latch
        self.first_write = None;

        // uses the read-ahead value
        let data = self.data_pre_read;

        // pre-read the next value
        self.data_pre_read = self.vram[self.address as usize];

        // increment the address
        self.address_wrapping_inc();

        // return the read-ahead value
        data
    }

    fn write_98(&mut self, data: u8) {
        // info!(
        //     "[VDP] Write 0x{:04x}: {:02x} [{}]",
        //     self.address,
        //     data,
        //     if data.is_ascii_graphic() {
        //         data as char
        //     } else {
        //         '.'
        //     }
        // );

        self.vram[self.address as usize] = data;
        self.data_pre_read = data;
        self.address = (self.address + 1) & 0x3FFF;
        self.first_write = None;
    }

    // fn read_register(&mut self) -> u8 {
    //     let data = self.status;
    //     // TODO: m_StatusReg = m_FifthSprite;
    //     // TODO: check_interrupt();
    //     self.latch = false;
    //     data
    // }

    fn read_register(&mut self) -> u8 {
        self.first_write = None;
        let res = self.status;
        // TODO: disable interrupt
        self.status &= 0x7F;
        res
    }

    fn update_mode(&mut self) {
        // Get the Mx bits from registers R#0 and R#0 - M3 is in R#1, M1 and M2 are in R#1
        // let mx_bits = ((self.registers[0] & 0x0E) >> 1) | ((self.registers[1] & 0x18) << 2);

        let r0 = self.registers[0];
        let r1 = self.registers[1];
        let m1: u8 = (r1 >> 4) & 0b0001;
        let m2: u8 = (r1 >> 3) & 0b0001;
        let m3: u8 = (r0 >> 1) & 0b0001;

        tracing::info!("[VDP] M1: {:?} | M2: {:?} | M3: {:?}", m1, m2, m3);

        let mx_bits: u8 = (m1 << 2) | (m2 << 1) | m3;

        // Determine the display mode based on the Mx bits
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
                DisplayMode::Text1 // Default to Text 1 for unsupported modes
            }
        };

        tracing::info!(
            "[VDP] Display mode is now: {:?} ({:04b})",
            self.display_mode,
            mx_bits
        );
        // Update the VDP's state based on the new display mode
        // (e.g., update the layout, pattern, or color tables, or change the rendering method)
    }

    fn write_register(&mut self, reg: u8, value: u8) {
        let old_value = self.registers[reg as usize];
        let modified = self.registers[reg as usize] ^ value;
        self.registers[reg as usize] = value;

        info!(
            "[VDP] Set register {} - from {:02X} to {:02X} - Modified: {:02X}",
            reg, old_value, value, modified
        );

        // Handle register-specific functionality
        match reg {
            0 => {
                if modified & 0x10 != 0 {
                    // Clear FH bit immediately when IE becomes 0? Not as per https://www.mail-archive.com/msx@stack.nl/msg13886.html
                    // We clear it only at the beginning of the next line if IE === 0
                    // Laydock2 has glitches on WebMSX with Turbo and also on a real Expert3 at 10MHz
                    // if (((val & 0x10) === 0) && FH) FH = 0
                    // update_irq();
                    info!(
                        "[VDP] Update IRQ (WIP) | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x0e != 0 {
                    info!(
                        "[VDP] Updating mode... | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    self.update_mode();
                }
            }
            1 => {
                // Update mode, IRQ, sprites config, blinking, etc.
                // Implement the functionality based on the WebMSX code

                if modified & 0x20 != 0 {
                    // IE0
                    info!(
                        "[VDP] 1 - 0x20 - Enable line interrupt | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // TODO self.update_irq();
                }
                if modified & 0x40 != 0 {
                    // BL
                    info!(
                        "[VDP] 1 - 0x40 - Blanking change pending | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // IE1: Frame interrupt enable
                    // WebMSX blanking_change_pending = true
                }
                if modified & 0x18 != 0 {
                    // Mx
                    info!(
                        "[VDP] 1 - 0x18 - Update mode | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    self.update_mode();
                }
                if modified & 0x04 != 0 {
                    //CDR  (Undocumented, changes reg 13 timing to lines instead of frames)
                    info!(
                        "[VDP] 1 - 0x04 - Update blinking | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // TODO WebMSX updateBlinking();
                }
                if modified & 0x03 != 0 {
                    // SI, MAG
                    info!(
                        "[VDP] 1 - 0x03 - Update sprites config | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // TODO self.update_sprites();
                }
            }
            2 => {
                if modified & 0x7f != 0 {
                    info!(
                        "[VDP] 2 - 0x7f - Update layout table address | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );

                    // Update layout table address
                    // TODO WebMSX if (mod & 0x7f) updateLayoutTableAddress();
                }
            }
            10 => {
                if modified & 0x07 != 0 {
                    info!(
                        "[VDP] 10 - 0x07 - Update color table address | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );

                    // Update color table address
                    // Implement the functionality based on the WebMSX code
                    // TODO WebMSX - if ((mod & 0x07) === 0) break; else fallthrough
                    // which I don't understand... fallthrough how?
                }
            }
            3 => {
                info!(
                    "[VDP] 3 - Update color table base address | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );

                // Mode Register 3 defines the starting address of the Colour Table in the VDP VRAM.
                // The eight available bits only specify positions 00BB BBBB BB00 0000 of the full
                // address so register contents of FFH would result in a base address of 3FC0H. In
                // Graphics Mode only bit 7 is effective thus offering a base of 0000H or 2000H.
                // Bits 0 to 6 must be 1.

                // Update pattern table address
                // TODO WebMSX
                // add = ((register[10] << 14) | (register[3] << 6)) & 0x1ffff;
                // colorTableAddress = add & modeData.colorTBase;
                // colorTableAddressMask = add | colorTableAddressMaskBase;

                // PatternNameTableAddress = (value << 10) & 0x3fff;
            }
            4 => {
                if modified & 0x3f != 0 {
                    info!(
                        "[VDP] 4 - 0x3f - Update pattern table address | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // Update pattern table address
                    // Implement the functionality based on the WebMSX code
                    // let cpt_base = (self.registers[4] as usize & 0x07) * 0x0800;
                    // self.cpt_base_address = cpt_base;
                }
            }
            5 => {
                info!(
                    "[VDP] 5 - Update sprite attribute table address | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                // WebMSX
                // add = ((register[11] << 15) | (register[5] << 7)) & 0x1ffff;
                // spriteAttrTableAddress = add & modeData.sprAttrTBase;
            }
            11 => {
                info!(
                    "[VDP] 11 - Update sprite attribute table address | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                // Update sprite attribute table address
                // Implement the functionality based on the WebMSX code
                // if ((mod & 0x03) === 0) break;
            }
            6 => {
                if modified & 0x3f != 0 {
                    info!("[VDP] 6 - 0x3f - Update sprite pattern table address | Reg: {} | Value: 0x{:02X}",
                        reg, value);
                    // Update sprite pattern table address
                    // Implement the functionality based on the WebMSX code
                    // if (mod & 0x3f) updateSpritePatternTableAddress();
                }
            }
            7 => {
                // BD
                let fg = value & 0xF0;
                let bg = value & 0x0F;

                info!("[VDP] 7 - Update backdrop color | FG: {} | BG: {}", fg, bg);

                // Update backdrop color
                // Implement the functionality based on the WebMSX code
                // if (mod & (modeData.bdPaletted ? 0x0f : 0xff)) updateBackdropColor();  // BD

                // var newTextColor = (byte)(value >> 4);
                // var newBackdropColor = (byte)(value & 0x0F);

                // if (newBackdropColor != backdropColor)
                //     displayRenderer.SetBackdropColor(newBackdropColor);
                // if(newTextColor != textColor)
                //     displayRenderer.SetTextColor(newTextColor);

                // backdropColor = newBackdropColor;
                // textColor = newTextColor;
                // break;
            }
            8 => {
                if modified & 0x20 != 0 {
                    // TP
                    info!(
                        "[VDP] 8 - 0x20 - Update transparency | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // WebMSX if (mod & 0x20) updateTransparency();                    // TP
                }
                if modified & 0x02 != 0 {
                    // SPD
                    info!(
                        "[VDP] 8 - 0x02 - Update sprites config | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // WebMSX if (mod & 0x02) updateSpritesConfig();                   // SPD
                }
                // Update transparency and sprites config
                // Implement the functionality based on the WebMSX code
            }
            9 => {
                if modified & 0x80 != 0 {
                    // LN
                    info!(
                        "[VDP] 9 - 0x80 - Update signal metrics | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x08 != 0 {
                    // IL
                    info!(
                        "[VDP] 9 - 0x08 - Update render metrics | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x04 != 0 {
                    // EO
                    info!(
                        "[VDP] 9 - 0x04 - Update layout address mask | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                if modified & 0x02 != 0 {
                    // NT
                    info!(
                        "[VDP] 9 - 0x02 - Update video standard | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                }
                // Update signal metrics, render metrics, layout table address mask, and video standard
                // Implement the functionality based on the WebMSX code
                // if (mod & 0x80) updateSignalMetrics(false);              // LN
                // if (mod & 0x08) updateRenderMetrics(false);              // IL
                // if (mod & 0x04) updateLayoutTableAddressMask();          // EO
                // if (mod & 0x02) updateVideoStandardSoft();               // NT
            }
            13 => {
                info!(
                    "[VDP] 13 - Update blinking | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                // Update blinking
                // Implement the functionality based on the WebMSX code
            }
            14 => {
                // Update VRAM pointer
                if modified & 0x07 == 0 {
                    info!(
                        "[VDP] 14 - 0x07 - Update VRAM pointer | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );

                    self.address = ((value & 0x07) as u16) << 14 | (self.address & 0x3FFF);
                    info!("[VDP] Setting VRAM pointer: {:04X}", self.address);
                }
            }
            16 => {
                info!(
                    "[VDP] 16 - Reset palette first write | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                // WebMSX paletteFirstWrite = null;
            }
            18 => {
                if modified & 0x0f != 0 {
                    info!(
                        "[VDP] 18 - 0x0f - Horizontal adjust | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // WebMSX:
                    // if (mod & 0x0f) horizontalAdjust = -7 + ((val & 0x0f) ^ 0x07);
                }
                if modified & 0xf0 != 0 {
                    info!(
                        "[VDP] 18 - 0xf0 - Vertical adjust | Reg: {} | Value: 0x{:02X}",
                        reg, value
                    );
                    // WebMSX:
                    // if (mod & 0xf0) {
                    //     verticalAdjust = -7 + ((val >>> 4) ^ 0x07);
                    //     updateSignalMetrics(false);
                    // }
                }
            }
            19 => {
                info!(
                    "[VDP] 19 - Set horizontal int line | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                // horizontalIntLine = (val - register[23]) & 255;
            }
            23 => {
                info!(
                    "[VDP] 23 - Set horizontal int line | Reg: {} | Value: 0x{:02X}",
                    reg, value
                );
                // horizontalIntLine = (register[19] - val) & 255;
            }
            25 => {
                // 9958 only
            }
            26 => {
                // 9958 only
            }
            27 => {
                // 9958 only
            }
            44 => {
                info!("[VDP] 44 - CPU Write | Data: 0x{:02X}", value);
            }
            46 => {
                info!("[VDP] 46 - Start Command | Data: 0x{:02X}", value);
            }
            _ => {}
        }
    }

    fn write_99(&mut self, val: u8) {
        // info!(
        //     "[VDP] Port: 99 | Address: {:04X} | Data: 0x{:02X} ({}).",
        //     self.address, data, data as char
        // );

        // The Data Port address register must be set up in different ways depending on whether the subsequent access is to be a read or a write.
        // The address register can be set to any value from 0000H to 3FFFH by first writing the LSB (Least Significant Byte)
        // and then the MSB (Most Significant Byte) to the Command Port. Bits 6 and 7 of the MSB are used by the VDP to
        // determine whether the address register is being set up for subsequent reads or writes as follows:
        //
        // 00 = Read
        // 01 = Write
        //

        let Some(data_first_write) = self.first_write else {
            self.first_write = Some(val);
            self.address = (self.address & !0xFF) | val as u16;
            return;
        };

        // 1000 0000
        if val & 0x80 != 0 {
            // info!(
            //     "[VDP] Write Register: {:02X} <- Latched Value: {:02X}",
            //     val, data_first_write,
            // );
            // Set register
            // info!("[VDP] Set register: {:02X}", data);
            // let reg = data & 0x07;
            // info!("[VDP] Register is: {:08b}", reg);
            // self.registers[reg as usize] = latched_value;
            // self.write_register(data, latched_value);

            let reg = val & 0x07;
            info!("[VDP] registers[{:02X}] = {:02X}", reg, data_first_write);
            self.write_register(reg, data_first_write);

            // let before = self.address;

            // On V9918, the VRAM pointer high gets also written when writing to registers
            self.address =
                ((self.address & 0x00FF) | ((data_first_write as u16 & 0x03F) << 8)) & 0x3FFF;
            // info!(
            //     "[VDP] Also setting high part of the address to {:02X}. Address 0x{:04x} -> 0x{:04x}",
            //     data_first_write, before, self.address
            // );
            info!("");
        } else {
            // Set VRAM pointer
            // info!(
            //     "[VDP] Latched value: 0x{:02X}. Received: 0x{:02X}",
            //     data_first_write, val
            // );

            // let before = self.address;

            self.address = (((val & 0x3f) as u16) << 8) | (data_first_write as u16) & 0x3FFF;

            // info!("[VDP] Address 0x{:04x} -> 0x{:04x}", before, self.address);
            // info!("");

            // Pre-read VRAM if "WriteMode = 0"
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
            // VRAM Read
            0x98 => self.read_vram(),
            // Register read
            0x99 => self.read_register(),
            _ => {
                error!("Invalid port: {:02X}", port);
                0xFF
            }
        }
    }

    pub fn write(&mut self, port: u8, data: u8) {
        // writing to data port 0x98
        match port {
            0x98 => self.write_98(data),
            0x99 => self.write_99(data),
            _ => {
                error!("Invalid port: {:02X}", port);
            }
        }
    }
}
