use crate::{vdp::DisplayMode, TMS9918};

pub struct Renderer<'a> {
    vdp: &'a TMS9918,
    pub screen_buffer: [u8; 256 * 192],
}

impl<'a> Renderer<'a> {
    pub fn new(vdp: &'a TMS9918) -> Self {
        let screen_buffer = [0; 256 * 192];
        Self { vdp, screen_buffer }
    }

    pub fn as_text(&mut self) -> String {
        let (base, size) = self.vdp.name_table_base_and_size();
        let mut text = String::new();
        for i in 0..size {
            let c = self.vdp.vram[base + i];
            if c == 0 {
                text.push(' ');
            } else {
                text.push(c as char);
            }
        }
        text
    }

    pub fn draw(&mut self) {
        // TODO check for scroll delta

        // The Border Colour bits determine the colour of the region surrounding the active video area in all
        // four VDP modes. They also determine the colour of all 0 pixels on the screen in 40x24 Text Mode.
        // Note that the border region actually extends across the entire screen but will only become visible
        // in the active area if the overlying pixel is transparent.
        //
        // The Text Colour 1 bits determine the colour of all 1 pixels in 40x24 Text Mode. They have no effect
        // in the other three modes where greater flexibility is provided through the use of the Colour Table.
        // The VDP colour codes are:
        //
        // 0 Transparent   4 Dark Blue      8 Red              12 Dark Green
        // 1 Black         5 Light Blue     9 Bright Red       13 Purple
        // 2 Green         6 Dark Red      10 Yellow           14 Grey
        // 3 Light Green   7 Sky Blue      11 Light Yellow     15 White

        let y0 = 0;
        let y1 = 192;
        let height = y1 - y0;

        for y in y0..height {
            // renders this raster line
            match self.vdp.display_mode {
                DisplayMode::Text1 => {
                    // screen 0
                    self.render_text1(y as usize);
                }
                DisplayMode::Graphic1 => {
                    // screen 1
                    self.render_graphic1(y as usize);
                }
                DisplayMode::Graphic2 => {
                    // screen 2
                    self.render_graphic2(y as usize);
                    
                }
                // DisplayMode::Multicolor => { // screen 3
                //     self.render_text2(y as usize, fg, bg);
                // }
                _ => panic!("Unsupported screen mode: {:?}", self.vdp.display_mode),
            }
        }
    }

    pub fn render_text1(&mut self, line: usize) {
        let fg = self.vdp.registers[7] & 0xF0;
        let bg = self.vdp.registers[7] & 0x0F;

        let caracter_pattern_area = self.vdp.char_pattern_table();
        let l = (line + self.vdp.get_vertical_scroll()) & 7;

        // Calculate the base address of the PNT using register R#2
        let pnt_base = (self.vdp.registers[2] as usize & 0x0F) * 0x0400;

        let name_start = (line / 8) * 40;
        let name_end = name_start + 40;
        let mut pixel_ptr = line * 256;
        for name in name_start..name_end {
            let screen_offset = pnt_base + name; // Calculate the proper offset in the VRAM
            let char_code = self.vdp.vram[screen_offset]; // Get the value directly from the VRAM array
            let pattern = caracter_pattern_area[l + char_code as usize * 8];

            for i in 0..6 {
                let mask = 0x80 >> i;
                self.screen_buffer[pixel_ptr + i] = if (pattern & mask) != 0 { fg } else { bg };
            }

            pixel_ptr += 6;
        }
    }

    pub fn render_graphic1(&mut self, line: usize) {
        let caracter_pattern_area = self.vdp.char_pattern_table();
        let l = (line + self.vdp.get_vertical_scroll()) & 7;

        // Calculate the base address of the PNT using register R#2
        let (pnt_base, _) = self.vdp.name_table_base_and_size();

        // Calculate the color table base address
        let color_table = self.vdp.color_table();

        let name_start = (line / 8) * 32;
        let name_end = name_start + 32;
        let mut pixel_ptr = line * 256;
        for name in name_start..name_end {
            let screen_offset = pnt_base + name;
            let char_code = self.vdp.vram[screen_offset];
            let color = color_table[char_code as usize / 8];
            let pattern = caracter_pattern_area[l + char_code as usize * 8];
            let fg = color >> 4;
            let bg = color & 0x0f;

            for i in 0..8 {
                let mask = 0x80 >> i;
                self.screen_buffer[pixel_ptr + i] = if (pattern & mask) != 0 { fg } else { bg };
            }

            pixel_ptr += 8;
        }
    }

    // New function to handle Screen 2 (Graphics Mode 2) rendering
    pub fn render_graphic2(&mut self, line: usize) {
        // Screen 2 specific configurations
        let name_table_base = 0x1800;
        let color_table_base = 0x2000;
        let pattern_table_base = 0x0000;

        let name_table = &self.vdp.vram[name_table_base..(name_table_base + 0x300)];
        let color_table = &self.vdp.vram[color_table_base..(color_table_base + 0x1800)];
        let pattern_table = &self.vdp.vram[pattern_table_base..(pattern_table_base + 0x1800)];

        let pattern_row = line % 8;
        let name_row = (line / 8) * 32;
        let mut pixel_ptr = line * 256;

        for x in 0..32 {
            let name_index = name_row + x;
            let char_code = name_table[name_index];
            let pattern_index = (char_code as usize * 8) + pattern_row;
            let pattern = pattern_table[pattern_index];

            let color_index = ((char_code as usize / 8) * 8) + pattern_row;
            let color = color_table[color_index];
            let fg = (color >> 4) & 0x0F;
            let bg = color & 0x0F;

            for i in 0..8 {
                let mask = 0x80 >> i;
                let pixel_index = pixel_ptr + i;
                if pixel_index < self.screen_buffer.len() {
                    self.screen_buffer[pixel_index] = if (pattern & mask) != 0 { fg } else { bg };
                }
            }

            pixel_ptr += 8;
        }
    }
}
