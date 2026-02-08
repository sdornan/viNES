pub mod mapper;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

#[derive(Debug)]
pub enum CartridgeError {
    InvalidHeader,
    UnsupportedMapper(u8),
    TruncatedFile,
}

impl fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeError::InvalidHeader => write!(f, "Invalid iNES header (missing NES\\x1A magic)"),
            CartridgeError::UnsupportedMapper(id) => write!(f, "Unsupported mapper: {}", id),
            CartridgeError::TruncatedFile => write!(f, "ROM file is truncated"),
        }
    }
}

impl std::error::Error for CartridgeError {}

const INES_MAGIC: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const PRG_ROM_PAGE_SIZE: usize = 16384; // 16KB
const CHR_ROM_PAGE_SIZE: usize = 8192; // 8KB
const TRAINER_SIZE: usize = 512;

pub struct Cartridge {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper_id: u8,
    pub mirroring: Mirroring,
}

impl Cartridge {
    pub fn from_ines(raw: &[u8]) -> Result<Self, CartridgeError> {
        if raw.len() < 16 {
            return Err(CartridgeError::TruncatedFile);
        }

        if raw[0..4] != INES_MAGIC {
            return Err(CartridgeError::InvalidHeader);
        }

        let prg_rom_pages = raw[4] as usize;
        let chr_rom_pages = raw[5] as usize;
        let flags6 = raw[6];
        let flags7 = raw[7];

        let mapper_id = (flags7 & 0xF0) | (flags6 >> 4);

        if mapper_id != 0 {
            return Err(CartridgeError::UnsupportedMapper(mapper_id));
        }

        let mirroring = if flags6 & 0x08 != 0 {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let has_trainer = flags6 & 0x04 != 0;

        let prg_rom_size = prg_rom_pages * PRG_ROM_PAGE_SIZE;
        let chr_rom_size = chr_rom_pages * CHR_ROM_PAGE_SIZE;

        let mut offset = 16;
        if has_trainer {
            offset += TRAINER_SIZE;
        }

        if raw.len() < offset + prg_rom_size + chr_rom_size {
            return Err(CartridgeError::TruncatedFile);
        }

        let prg_rom = raw[offset..offset + prg_rom_size].to_vec();
        offset += prg_rom_size;

        let chr_rom = if chr_rom_size > 0 {
            raw[offset..offset + chr_rom_size].to_vec()
        } else {
            // CHR RAM: allocate 8KB of zeros
            vec![0u8; CHR_ROM_PAGE_SIZE]
        };

        Ok(Cartridge {
            prg_rom,
            chr_rom,
            mapper_id,
            mirroring,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(prg_pages: u8, chr_pages: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut header = vec![0x4E, 0x45, 0x53, 0x1A, prg_pages, chr_pages, flags6, flags7];
        header.extend_from_slice(&[0u8; 8]); // remaining header bytes
        // Add PRG ROM data
        header.extend_from_slice(&vec![0xEA; prg_pages as usize * PRG_ROM_PAGE_SIZE]);
        // Add CHR ROM data
        header.extend_from_slice(&vec![0x00; chr_pages as usize * CHR_ROM_PAGE_SIZE]);
        header
    }

    #[test]
    fn test_valid_header_nrom128() {
        let data = make_header(1, 1, 0x00, 0x00);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.prg_rom.len(), PRG_ROM_PAGE_SIZE);
        assert_eq!(cart.chr_rom.len(), CHR_ROM_PAGE_SIZE);
        assert_eq!(cart.mapper_id, 0);
        assert_eq!(cart.mirroring, Mirroring::Horizontal);
    }

    #[test]
    fn test_valid_header_nrom256() {
        let data = make_header(2, 1, 0x01, 0x00); // vertical mirroring
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.prg_rom.len(), 2 * PRG_ROM_PAGE_SIZE);
        assert_eq!(cart.mirroring, Mirroring::Vertical);
    }

    #[test]
    fn test_chr_ram_when_no_chr_rom() {
        let data = make_header(1, 0, 0x00, 0x00);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.chr_rom.len(), CHR_ROM_PAGE_SIZE);
    }

    #[test]
    fn test_invalid_magic() {
        let data = vec![0x00; 32];
        assert!(matches!(
            Cartridge::from_ines(&data),
            Err(CartridgeError::InvalidHeader)
        ));
    }

    #[test]
    fn test_truncated_file() {
        let data = vec![0x4E, 0x45, 0x53, 0x1A, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(matches!(
            Cartridge::from_ines(&data),
            Err(CartridgeError::TruncatedFile)
        ));
    }

    #[test]
    fn test_unsupported_mapper() {
        let data = make_header(1, 1, 0x10, 0x00); // mapper 1
        assert!(matches!(
            Cartridge::from_ines(&data),
            Err(CartridgeError::UnsupportedMapper(1))
        ));
    }

    #[test]
    fn test_four_screen_mirroring() {
        let data = make_header(1, 1, 0x08, 0x00);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.mirroring, Mirroring::FourScreen);
    }

    #[test]
    fn test_trainer_skip() {
        let mut data = vec![0x4E, 0x45, 0x53, 0x1A, 1, 1, 0x04, 0x00];
        data.extend_from_slice(&[0u8; 8]);
        data.extend_from_slice(&[0xFF; TRAINER_SIZE]); // trainer
        data.extend_from_slice(&vec![0xEA; PRG_ROM_PAGE_SIZE]);
        data.extend_from_slice(&vec![0x00; CHR_ROM_PAGE_SIZE]);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.prg_rom[0], 0xEA); // should be PRG data, not trainer
    }
}
