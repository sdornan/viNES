use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::cartridge::mapper::{Mapper, Mapper0};
use crate::controller::Controller;
use crate::ppu::Ppu;

#[derive(Clone)]
pub struct Bus {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub mapper: Box<dyn Mapper>,
    pub controller1: Controller,
    pub controller2: Controller,
    pub cycles: u64,
}

impl Bus {
    pub fn new(cartridge: Cartridge, sample_buffer: Arc<ArrayQueue<f32>>) -> Self {
        let mirroring = cartridge.mirroring;
        let chr_rom = cartridge.chr_rom.clone();
        let mapper: Box<dyn Mapper> = Box::new(Mapper0::new(
            cartridge.prg_rom,
            cartridge.chr_rom,
            cartridge.mirroring,
        ));

        Bus {
            ram: [0; 2048],
            ppu: Ppu::new(chr_rom, mirroring),
            apu: Apu::new(sample_buffer),
            mapper,
            controller1: Controller::new(),
            controller2: Controller::new(),
            cycles: 0,
        }
    }

    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read(0x2000 + (addr & 0x07)),
            0x4014 => 0,
            0x4015 => self.apu.read_status(),
            0x4016 => self.controller1.read(),
            0x4017 => self.controller2.read(),
            0x4000..=0x4017 => 0, // write-only APU regs
            0x4018..=0x401F => 0,
            0x4020..=0xFFFF => self.mapper.cpu_read(addr),
        }
    }

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = val,
            0x2000..=0x3FFF => self.ppu.cpu_write(0x2000 + (addr & 0x07), val),
            0x4014 => self.oam_dma(val),
            0x4000..=0x4013 => self.apu.cpu_write(addr, val),
            0x4015 => self.apu.write_status(val),
            0x4016 => self.controller1.write(val),
            0x4017 => self.apu.write_frame_counter(val),
            0x4018..=0x401F => {}
            0x4020..=0xFFFF => self.mapper.cpu_write(addr, val),
        }
    }

    fn oam_dma(&mut self, page: u8) {
        let base = (page as u16) << 8;
        for i in 0..256u16 {
            let val = self.cpu_read(base + i);
            self.ppu.oam[self.ppu.oam_addr.wrapping_add(i as u8) as usize] = val;
        }
        // DMA takes 513 or 514 CPU cycles - handled by CPU stall
    }
}
