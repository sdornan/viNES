use super::Mirroring;

pub trait Mapper {
    fn cpu_read(&self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, val: u8);
    fn chr_read(&self, addr: u16) -> u8;
    fn chr_write(&mut self, addr: u16, val: u8);
    fn mirroring(&self) -> Mirroring;
    fn clone_box(&self) -> Box<dyn Mapper>;
}

impl Clone for Box<dyn Mapper> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Mapper 0 (NROM): No bank switching.
/// NROM-128: 16KB PRG ROM mirrored at $8000 and $C000.
/// NROM-256: 32KB PRG ROM at $8000-$FFFF.
/// 8KB CHR ROM (or CHR RAM) at PPU $0000-$1FFF.
pub struct Mapper0 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    mirroring: Mirroring,
    prg_ram: [u8; 8192],
}

impl Mapper0 {
    pub fn new(prg_rom: Vec<u8>, chr: Vec<u8>, mirroring: Mirroring) -> Self {
        Mapper0 {
            prg_rom,
            chr,
            mirroring,
            prg_ram: [0; 8192],
        }
    }
}

impl Mapper for Mapper0 {
    fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000..=0xFFFF => {
                let mut index = (addr - 0x8000) as usize;
                if self.prg_rom.len() == 16384 {
                    index %= 16384; // mirror for NROM-128
                }
                self.prg_rom[index]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if let 0x6000..=0x7FFF = addr {
            self.prg_ram[(addr - 0x6000) as usize] = val;
        }
    }

    fn chr_read(&self, addr: u16) -> u8 {
        self.chr[addr as usize]
    }

    fn chr_write(&mut self, addr: u16, val: u8) {
        // Only writable if CHR RAM (no CHR ROM on cart)
        self.chr[addr as usize] = val;
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn clone_box(&self) -> Box<dyn Mapper> {
        Box::new(Mapper0 {
            prg_rom: self.prg_rom.clone(),
            chr: self.chr.clone(),
            mirroring: self.mirroring,
            prg_ram: self.prg_ram,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapper0_nrom128_mirroring() {
        let mut prg = vec![0u8; 16384];
        prg[0] = 0xAA;
        prg[0x3FFF] = 0xBB;
        let chr = vec![0u8; 8192];
        let mapper = Mapper0::new(prg, chr, Mirroring::Horizontal);

        // $8000 and $C000 should mirror
        assert_eq!(mapper.cpu_read(0x8000), 0xAA);
        assert_eq!(mapper.cpu_read(0xC000), 0xAA);
        assert_eq!(mapper.cpu_read(0xBFFF), 0xBB);
        assert_eq!(mapper.cpu_read(0xFFFF), 0xBB);
    }

    #[test]
    fn test_mapper0_nrom256() {
        let mut prg = vec![0u8; 32768];
        prg[0] = 0xAA;
        prg[0x4000] = 0xBB;
        let chr = vec![0u8; 8192];
        let mapper = Mapper0::new(prg, chr, Mirroring::Vertical);

        assert_eq!(mapper.cpu_read(0x8000), 0xAA);
        assert_eq!(mapper.cpu_read(0xC000), 0xBB); // different in NROM-256
    }

    #[test]
    fn test_mapper0_prg_ram() {
        let prg = vec![0u8; 16384];
        let chr = vec![0u8; 8192];
        let mut mapper = Mapper0::new(prg, chr, Mirroring::Horizontal);

        mapper.cpu_write(0x6000, 0x42);
        assert_eq!(mapper.cpu_read(0x6000), 0x42);
    }

    #[test]
    fn test_mapper0_chr() {
        let prg = vec![0u8; 16384];
        let mut chr = vec![0u8; 8192];
        chr[0x100] = 0xFF;
        let mapper = Mapper0::new(prg, chr, Mirroring::Horizontal);

        assert_eq!(mapper.chr_read(0x100), 0xFF);
    }
}
