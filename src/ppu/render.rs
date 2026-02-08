use super::Ppu;
use super::frame::SYSTEM_PALETTE;

impl Ppu {
    pub fn render_scanline(&mut self, scanline: u16) {
        // Clear scanline to universal background color
        let bg_color = SYSTEM_PALETTE[self.palette_ram[0] as usize % 64];
        for x in 0..256 {
            self.frame.set_pixel(x, scanline as usize, bg_color);
        }

        if self.mask.contains(super::registers::PpuMask::SHOW_BG) {
            self.render_bg_scanline(scanline);
        }
        if self.mask.contains(super::registers::PpuMask::SHOW_SPR) {
            self.render_sprite_scanline(scanline);
        }
    }

    fn render_bg_scanline(&mut self, _scanline: u16) {
        let bg_table = self.ctrl.bg_pattern_table();
        let show_left = self.mask.contains(super::registers::PpuMask::SHOW_BG_LEFT);

        // Extract scroll position from V register (loopy scrolling)
        let coarse_x_start = self.v & 0x001F;
        let coarse_y = (self.v >> 5) & 0x001F;
        let fine_y = ((self.v >> 12) & 0x0007) as u8;
        let nt_select = (self.v >> 10) & 0x0003;
        let nt_base_x = nt_select & 1;    // horizontal nametable bit
        let nt_base_y = (nt_select >> 1) & 1; // vertical nametable bit
        let fine_x_start = self.fine_x;

        for screen_x in 0u16..256 {
            // Left-column masking: hide BG in leftmost 8 pixels if not enabled
            if !show_left && screen_x < 8 {
                continue; // already cleared to BG color
            }

            // Absolute pixel X within the two-nametable-wide space
            let abs_x = coarse_x_start * 8 + fine_x_start as u16 + screen_x;
            let tile_col = (abs_x / 8) & 0x1F; // wrap within 32 tiles
            let fine_x_pos = (abs_x % 8) as u8;
            // Determine if we've crossed into the adjacent horizontal nametable
            let nt_x = nt_base_x ^ ((abs_x / 256) & 1);

            let nt_addr = 0x2000 + (nt_base_y * 2 + nt_x) * 0x0400
                + coarse_y * 32 + tile_col;
            let tile_index = self.internal_read(nt_addr) as u16;

            // Fetch pattern data
            let pattern_addr = bg_table + tile_index * 16 + fine_y as u16;
            let plane0 = self.internal_read(pattern_addr);
            let plane1 = self.internal_read(pattern_addr + 8);

            let bit = 7 - fine_x_pos;
            let color_lo = (plane0 >> bit) & 1;
            let color_hi = (plane1 >> bit) & 1;
            let pixel = (color_hi << 1) | color_lo;

            // Fetch attribute byte
            let attr_base = 0x2000 + (nt_base_y * 2 + nt_x) * 0x0400 + 0x03C0;
            let attr_addr = attr_base + (coarse_y / 4) * 8 + (tile_col / 4);
            let attr_byte = self.internal_read(attr_addr);
            let shift = ((coarse_y % 4) / 2 * 2 + (tile_col % 4) / 2) * 2;
            let palette_index = (attr_byte >> shift) & 0x03;

            let color = if pixel == 0 {
                self.palette_ram[0] as usize
            } else {
                self.palette_ram[(palette_index as usize * 4 + pixel as usize) & 0x1F] as usize
            };

            let rgb = SYSTEM_PALETTE[color % 64];
            self.frame.set_pixel(screen_x as usize, _scanline as usize, rgb);
        }
    }

    fn render_sprite_scanline(&mut self, scanline: u16) {
        let sprite_table = self.ctrl.sprite_pattern_table();
        let sprite_height: u16 = if self.ctrl.contains(super::registers::PpuCtrl::SPRITE_SIZE) { 16 } else { 8 };
        let show_left = self.mask.contains(super::registers::PpuMask::SHOW_SPR_LEFT);

        // Evaluate sprites in forward order (0-63) for correct overflow behavior,
        // then render in reverse for priority (sprite 0 on top)
        let mut sprite_indices: [usize; 8] = [0; 8];
        let mut sprites_on_line = 0usize;
        for i in 0..64 {
            let oam_offset = i * 4;
            let sprite_y = self.oam[oam_offset] as u16 + 1;
            if scanline >= sprite_y && scanline < sprite_y + sprite_height {
                if sprites_on_line < 8 {
                    sprite_indices[sprites_on_line] = i;
                    sprites_on_line += 1;
                } else {
                    self.status.insert(super::registers::PpuStatus::SPRITE_OVERFLOW);
                    break;
                }
            }
        }

        // Render in reverse order so lower-indexed sprites (higher priority) draw last
        for slot in (0..sprites_on_line).rev() {
            let i = sprite_indices[slot];
            let oam_offset = i * 4;
            let sprite_y = self.oam[oam_offset] as u16 + 1;
            let tile_index = self.oam[oam_offset + 1];
            let attributes = self.oam[oam_offset + 2];
            let sprite_x = self.oam[oam_offset + 3] as u16;

            let flip_h = attributes & 0x40 != 0;
            let flip_v = attributes & 0x80 != 0;
            let behind_bg = attributes & 0x20 != 0;
            let palette_index = (attributes & 0x03) + 4; // sprite palettes are 4-7

            let mut row = (scanline - sprite_y) as u8;
            if flip_v {
                row = (sprite_height as u8 - 1) - row;
            }

            let pattern_addr = sprite_table + tile_index as u16 * 16 + row as u16;
            let plane0 = self.internal_read(pattern_addr);
            let plane1 = self.internal_read(pattern_addr + 8);

            for col in 0u8..8 {
                let bit = if flip_h { col } else { 7 - col };
                let color_lo = (plane0 >> bit) & 1;
                let color_hi = (plane1 >> bit) & 1;
                let pixel = (color_hi << 1) | color_lo;

                if pixel == 0 {
                    continue; // transparent
                }

                let px = sprite_x.wrapping_add(col as u16);
                if px >= 256 {
                    continue;
                }

                // Left-column masking: hide sprites in leftmost 8 pixels if not enabled
                if !show_left && px < 8 {
                    continue;
                }

                // Sprite 0 hit detection (not triggered at x=255)
                if i == 0 && px < 255 && self.is_bg_pixel_opaque(px as usize, scanline as usize) {
                    self.status.insert(super::registers::PpuStatus::SPRITE_ZERO_HIT);
                }

                // Priority: if behind_bg and BG pixel is non-zero, don't draw
                if behind_bg && self.is_bg_pixel_opaque(px as usize, scanline as usize) {
                    continue;
                }

                let color = self.palette_ram[(palette_index as usize * 4 + pixel as usize) & 0x1F] as usize;
                let rgb = SYSTEM_PALETTE[color % 64];
                self.frame.set_pixel(px as usize, scanline as usize, rgb);
            }
        }
    }

    /// Check if the background pixel at (x, y) is non-transparent.
    /// We check the frame buffer: if the pixel matches the universal background color, it's transparent.
    fn is_bg_pixel_opaque(&self, x: usize, y: usize) -> bool {
        let bg_color = SYSTEM_PALETTE[self.palette_ram[0] as usize % 64];
        let idx = (y * 256 + x) * 3;
        if idx + 2 >= self.frame.data.len() {
            return false;
        }
        let r = self.frame.data[idx];
        let g = self.frame.data[idx + 1];
        let b = self.frame.data[idx + 2];
        (r, g, b) != bg_color
    }
}
