use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct PpuCtrl: u8 {
        const NAMETABLE_LO   = 0b0000_0001;
        const NAMETABLE_HI   = 0b0000_0010;
        const VRAM_INCREMENT = 0b0000_0100; // 0=+1, 1=+32
        const SPRITE_TABLE   = 0b0000_1000;
        const BG_TABLE       = 0b0001_0000;
        const SPRITE_SIZE    = 0b0010_0000; // 0=8x8, 1=8x16
        const MASTER_SLAVE   = 0b0100_0000;
        const NMI_ENABLE     = 0b1000_0000;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct PpuMask: u8 {
        const GREYSCALE       = 0b0000_0001;
        const SHOW_BG_LEFT    = 0b0000_0010;
        const SHOW_SPR_LEFT   = 0b0000_0100;
        const SHOW_BG         = 0b0000_1000;
        const SHOW_SPR        = 0b0001_0000;
        const EMPHASIZE_RED   = 0b0010_0000;
        const EMPHASIZE_GREEN = 0b0100_0000;
        const EMPHASIZE_BLUE  = 0b1000_0000;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct PpuStatus: u8 {
        const SPRITE_OVERFLOW  = 0b0010_0000;
        const SPRITE_ZERO_HIT  = 0b0100_0000;
        const VBLANK           = 0b1000_0000;
    }
}

impl PpuCtrl {
    pub fn nametable_base(&self) -> u16 {
        match self.bits() & 0x03 {
            0 => 0x2000,
            1 => 0x2400,
            2 => 0x2800,
            3 => 0x2C00,
            _ => unreachable!(),
        }
    }

    pub fn vram_increment(&self) -> u16 {
        if self.contains(PpuCtrl::VRAM_INCREMENT) { 32 } else { 1 }
    }

    pub fn sprite_pattern_table(&self) -> u16 {
        if self.contains(PpuCtrl::SPRITE_TABLE) { 0x1000 } else { 0x0000 }
    }

    pub fn bg_pattern_table(&self) -> u16 {
        if self.contains(PpuCtrl::BG_TABLE) { 0x1000 } else { 0x0000 }
    }
}
