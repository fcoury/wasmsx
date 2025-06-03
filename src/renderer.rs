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

    // In render_text1, after the main character rendering loop for a line:
    // let mut pixel_ptr has advanced by 240 at this point for the current line.
    // Let's rename pixel_ptr inside the loop to avoid confusion, or track current_x.

    pub fn render_text1(&mut self, line: usize) {
        let r7 = self.vdp.registers[7];
        let fg_color = (r7 & 0xF0) >> 4; // Corrected foreground
        let bg_and_border_color = r7 & 0x0F; // Background and Border for Text1

        let caracter_pattern_area = self.vdp.char_pattern_table();
        let l = (line + self.vdp.get_vertical_scroll()) & 7;

        let pnt_base = (self.vdp.registers[2] as usize & 0x0F) * 0x0400;
        let name_start_for_row = (line / 8) * 40; // Starting character index in PNT for this row

        let mut current_x_on_scanline = 0;

        // Render 40 characters (240 pixels)
        for char_column_idx in 0..40 {
            // It's safer to calculate full VRAM addresses and screen buffer indices
            // to avoid off-by-one errors with `pixel_ptr` logic.
            let char_code_vram_addr = pnt_base + name_start_for_row + char_column_idx;
            // Add bounds check for VRAM access if necessary
            let char_code = self.vdp.vram[char_code_vram_addr % self.vdp.vram.len()]; // Modulo for safety for now

            let pattern_offset_in_cpt = (char_code as usize * 8) + l;
            // Add bounds check for CPT access if necessary
            let pattern =
                caracter_pattern_area[pattern_offset_in_cpt % caracter_pattern_area.len()]; // Modulo for safety

            for bit_idx in 0..6 {
                // 6 pixels per character
                let screen_buffer_idx = (line * 256) + current_x_on_scanline + bit_idx;
                if screen_buffer_idx < self.screen_buffer.len() {
                    self.screen_buffer[screen_buffer_idx] = if (pattern & (0x80 >> bit_idx)) != 0 {
                        fg_color
                    } else {
                        bg_and_border_color // Text background
                    };
                }
            }
            current_x_on_scanline += 6;
        }

        // current_x_on_scanline is now 240.
        // Fill the remaining 16 pixels on the right with the border color.
        for i in 0..16 {
            let screen_buffer_idx = (line * 256) + current_x_on_scanline + i;
            if screen_buffer_idx < self.screen_buffer.len() {
                self.screen_buffer[screen_buffer_idx] = bg_and_border_color; // Border
            }
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

        // Render sprites on this line
        if line < self.vdp.sprites_visible.len() {
            let visible_sprites = &self.vdp.sprites_visible[line];
            self.vdp.render_sprites_on_line(line, &mut self.screen_buffer, visible_sprites);
        }
    }

    // New function to handle Screen 2 (Graphics Mode 2) rendering
    pub fn render_graphic2(&mut self, line: usize) {
        // Get table base addresses from VDP registers
        let (name_table_base, _) = self.vdp.name_table_base_and_size();
        let pattern_table = self.vdp.char_pattern_table();
        let color_table = self.vdp.color_table();

        let pattern_row = line % 8;
        let char_row = line / 8;  // Which character row (0-23)
        let name_offset = char_row * 32;  // Offset into name table
        let mut pixel_ptr = line * 256;

        // Screen 2 divides the screen into 3 banks (thirds) of 8 character rows each
        // Bank 0: lines 0-63 (character rows 0-7)
        // Bank 1: lines 64-127 (character rows 8-15)
        // Bank 2: lines 128-191 (character rows 16-23)
        let bank = (line / 64) as usize;  // 0, 1, or 2

        for x in 0..32 {
            let name_index = name_offset + x;
            let char_code = self.vdp.vram[name_table_base + name_index] as usize;
            
            // In Screen 2, pattern/color tables are organized differently:
            // Each bank (third of screen) can use different pattern definitions for the same character
            // The effective character code in the pattern table is: char_code + (bank * 256)
            // But since pattern table only has 256 entries per bank, we wrap at 256
            let effective_char = char_code & 0xFF;  // Ensure we stay within 0-255
            let pattern_index = (bank * 2048) + (effective_char * 8) + pattern_row;
            let pattern = if pattern_index < pattern_table.len() {
                pattern_table[pattern_index]
            } else {
                0
            };

            // Color table has same structure as pattern table in Screen 2
            let color_index = (bank * 2048) + (effective_char * 8) + pattern_row;
            let color = if color_index < color_table.len() {
                color_table[color_index]
            } else {
                0x1F  // Default to white on black if out of bounds
            };
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

        // Render sprites on this line
        if line < self.vdp.sprites_visible.len() {
            let visible_sprites = &self.vdp.sprites_visible[line];
            self.vdp.render_sprites_on_line(line, &mut self.screen_buffer, visible_sprites);
        }
    }
}
