use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;

pub struct Nes {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Nes {
    pub fn new(cartridge: Cartridge, sample_buffer: Arc<ArrayQueue<f32>>) -> Self {
        Nes {
            cpu: Cpu::new(),
            bus: Bus::new(cartridge, sample_buffer),
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    /// Run one CPU instruction, then catch up PPU and APU. Returns true if frame is complete.
    pub fn step(&mut self) -> bool {
        let cpu_cycles = self.cpu.step(&mut self.bus);
        let ppu_cycles = cpu_cycles as u16 * 3;
        let mut frame_complete = false;

        for _ in 0..ppu_cycles {
            if self.bus.ppu.tick() {
                frame_complete = true;
            }
        }

        // APU ticks at CPU rate
        for _ in 0..cpu_cycles {
            self.bus.apu.tick();
        }

        if self.bus.ppu.nmi_pending {
            self.bus.ppu.nmi_pending = false;
            self.cpu.nmi(&mut self.bus);
        }

        frame_complete
    }

    /// Run until a full frame is rendered (with safety limit).
    /// Returns true if frame completed normally, false if safety limit hit.
    pub fn step_frame(&mut self) -> bool {
        // ~29,781 CPU steps per frame; 40,000 is a generous safety margin
        for _ in 0..40_000 {
            if self.step() {
                return true;
            }
        }
        false
    }
}
