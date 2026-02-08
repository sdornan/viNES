pub mod registers;
pub mod frame;
pub mod render;

use registers::{PpuCtrl, PpuMask, PpuStatus};
use frame::Frame;
use crate::cartridge::Mirroring;

pub struct Ppu {
    // CHR data (from cartridge, static for Mapper 0)
    pub chr_rom: Vec<u8>,

    // VRAM
    pub palette_ram: [u8; 32],
    pub vram: [u8; 2048],
    pub oam: [u8; 256],

    // Registers
    pub ctrl: PpuCtrl,
    pub mask: PpuMask,
    pub status: PpuStatus,
    pub oam_addr: u8,

    // Internal registers
    pub v: u16,      // current VRAM address
    pub t: u16,      // temporary VRAM address
    pub fine_x: u8,  // fine X scroll
    pub w: bool,     // write toggle

    // Scroll values (simplified for scanline renderer)
    pub scroll_x: u8,
    pub scroll_y: u8,

    // Latches
    pub read_buffer: u8,

    // Rendering state
    pub scanline: u16,
    pub cycle: u16,
    pub frame_count: u64,

    // NMI
    pub nmi_pending: bool,

    // Output
    pub frame: Frame,

    // Mirroring
    pub mirroring: Mirroring,
}

impl Ppu {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        Ppu {
            chr_rom,
            palette_ram: [0; 32],
            vram: [0; 2048],
            oam: [0; 256],
            ctrl: PpuCtrl::empty(),
            mask: PpuMask::empty(),
            status: PpuStatus::empty(),
            oam_addr: 0,
            v: 0,
            t: 0,
            fine_x: 0,
            w: false,
            scroll_x: 0,
            scroll_y: 0,
            read_buffer: 0,
            scanline: 0,
            cycle: 0,
            frame_count: 0,
            nmi_pending: false,
            frame: Frame::new(),
            mirroring,
        }
    }

    fn rendering_enabled(&self) -> bool {
        self.mask.contains(PpuMask::SHOW_BG) || self.mask.contains(PpuMask::SHOW_SPR)
    }

    /// Increment the fine Y scroll in V, wrapping through coarse Y and nametable.
    fn increment_v_y(&mut self) {
        if (self.v & 0x7000) != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let mut coarse_y = (self.v & 0x03E0) >> 5;
            if coarse_y == 29 {
                coarse_y = 0;
                self.v ^= 0x0800; // switch vertical nametable
            } else if coarse_y == 31 {
                coarse_y = 0;
            } else {
                coarse_y += 1;
            }
            self.v = (self.v & !0x03E0) | (coarse_y << 5);
        }
    }

    /// Tick the PPU by one cycle. Returns true when a frame is complete.
    pub fn tick(&mut self) -> bool {
        let mut frame_complete = false;
        let visible = self.scanline < 240;
        let pre_render = self.scanline == 261;

        // Render visible scanline at cycle 0 (reads V but doesn't modify it)
        if visible && self.cycle == 0 {
            self.render_scanline(self.scanline);
        }

        // V register updates at correct cycle timing (visible + pre-render)
        if (visible || pre_render) && self.rendering_enabled() {
            // Cycle 256: increment fine Y
            if self.cycle == 256 {
                self.increment_v_y();
            }
            // Cycle 257: copy horizontal bits from T to V
            if self.cycle == 257 {
                self.v = (self.v & !0x041F) | (self.t & 0x041F);
            }
            // Pre-render line cycles 280-304: copy vertical bits from T to V
            if pre_render && self.cycle >= 280 && self.cycle <= 304 {
                self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
            }
        }

        // Pre-render line: clear flags
        if pre_render && self.cycle == 1 {
            self.status.remove(PpuStatus::VBLANK);
            self.status.remove(PpuStatus::SPRITE_ZERO_HIT);
            self.status.remove(PpuStatus::SPRITE_OVERFLOW);
        }

        // Vblank start
        if self.scanline == 241 && self.cycle == 1 {
            self.status.insert(PpuStatus::VBLANK);
            if self.ctrl.contains(PpuCtrl::NMI_ENABLE) {
                self.nmi_pending = true;
            }
            frame_complete = true;
        }

        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.frame_count += 1;
            }
        }

        frame_complete
    }

    /// CPU read from PPU register ($2000-$2007)
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x2002 => {
                // PPUSTATUS
                let val = self.status.bits() | (self.read_buffer & 0x1F);
                self.status.remove(PpuStatus::VBLANK);
                self.w = false;
                val
            }
            0x2004 => {
                // OAMDATA
                self.oam[self.oam_addr as usize]
            }
            0x2007 => {
                // PPUDATA
                let addr = self.v;
                self.v = self.v.wrapping_add(self.ctrl.vram_increment());
                self.v &= 0x3FFF;

                if addr >= 0x3F00 {
                    // Palette reads are not buffered
                    let result = self.palette_read(addr);
                    // But the buffer gets filled with the nametable "under" the palette
                    self.read_buffer = self.internal_read(addr - 0x1000);
                    result
                } else {
                    let result = self.read_buffer;
                    self.read_buffer = self.internal_read(addr);
                    result
                }
            }
            _ => 0, // write-only registers return 0
        }
    }

    /// CPU write to PPU register ($2000-$2007)
    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x2000 => {
                // PPUCTRL
                let was_nmi = self.ctrl.contains(PpuCtrl::NMI_ENABLE);
                self.ctrl = PpuCtrl::from_bits_truncate(val);
                // If NMI enabled while in vblank, trigger NMI
                if !was_nmi && self.ctrl.contains(PpuCtrl::NMI_ENABLE) && self.status.contains(PpuStatus::VBLANK) {
                    self.nmi_pending = true;
                }
                // Update nametable select in t register
                self.t = (self.t & 0xF3FF) | ((val as u16 & 0x03) << 10);
            }
            0x2001 => {
                // PPUMASK
                self.mask = PpuMask::from_bits_truncate(val);
            }
            0x2003 => {
                // OAMADDR
                self.oam_addr = val;
            }
            0x2004 => {
                // OAMDATA
                self.oam[self.oam_addr as usize] = val;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                // PPUSCROLL
                if !self.w {
                    // First write: X scroll
                    self.scroll_x = val;
                    self.fine_x = val & 0x07;
                    self.t = (self.t & 0xFFE0) | ((val as u16) >> 3);
                } else {
                    // Second write: Y scroll
                    self.scroll_y = val;
                    self.t = (self.t & 0x8C1F)
                        | (((val as u16) & 0x07) << 12)
                        | (((val as u16) >> 3) << 5);
                }
                self.w = !self.w;
            }
            0x2006 => {
                // PPUADDR
                if !self.w {
                    // First write: high byte
                    self.t = (self.t & 0x00FF) | ((val as u16 & 0x3F) << 8);
                } else {
                    // Second write: low byte
                    self.t = (self.t & 0xFF00) | val as u16;
                    self.v = self.t;
                }
                self.w = !self.w;
            }
            0x2007 => {
                // PPUDATA
                let addr = self.v;
                self.v = self.v.wrapping_add(self.ctrl.vram_increment());
                self.v &= 0x3FFF;
                self.internal_write(addr, val);
            }
            _ => {}
        }
    }

    /// Read from PPU internal address space
    pub fn internal_read(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => {
                // Pattern tables (CHR ROM/RAM)
                if (addr as usize) < self.chr_rom.len() {
                    self.chr_rom[addr as usize]
                } else {
                    0
                }
            }
            0x2000..=0x3EFF => {
                // Nametables
                let mirrored = self.mirror_vram_addr(addr);
                self.vram[mirrored]
            }
            0x3F00..=0x3FFF => {
                self.palette_read(addr)
            }
            _ => 0,
        }
    }

    /// Write to PPU internal address space
    fn internal_write(&mut self, addr: u16, val: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => {
                // CHR RAM write (if using CHR RAM)
                if (addr as usize) < self.chr_rom.len() {
                    self.chr_rom[addr as usize] = val;
                }
            }
            0x2000..=0x3EFF => {
                let mirrored = self.mirror_vram_addr(addr);
                self.vram[mirrored] = val;
            }
            0x3F00..=0x3FFF => {
                self.palette_write(addr, val);
            }
            _ => {}
        }
    }

    fn palette_read(&self, addr: u16) -> u8 {
        let index = self.palette_mirror(addr);
        self.palette_ram[index]
    }

    fn palette_write(&mut self, addr: u16, val: u8) {
        let index = self.palette_mirror(addr);
        self.palette_ram[index] = val;
    }

    fn palette_mirror(&self, addr: u16) -> usize {
        let mut index = (addr - 0x3F00) as usize & 0x1F;
        // Mirrors: $3F10 -> $3F00, $3F14 -> $3F04, $3F18 -> $3F08, $3F1C -> $3F0C
        if index == 0x10 || index == 0x14 || index == 0x18 || index == 0x1C {
            index -= 0x10;
        }
        index
    }

    fn mirror_vram_addr(&self, addr: u16) -> usize {
        let addr = (addr - 0x2000) as usize & 0x0FFF; // remove mirroring above $2FFF
        let nametable = addr / 0x400;
        let offset = addr % 0x400;
        let mirrored_nt = match self.mirroring {
            Mirroring::Horizontal => match nametable {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => 0,
            },
            Mirroring::Vertical => match nametable {
                0 | 2 => 0,
                1 | 3 => 1,
                _ => 0,
            },
            Mirroring::FourScreen => nametable,
        };
        mirrored_nt * 0x400 + offset
    }
}
