use crate::bus::Bus;
use super::Cpu;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Relative,
    Implied,
    Accumulator,
    None,
}

/// Returns (resolved address, extra cycles from page crossing).
pub fn resolve(cpu: &mut Cpu, bus: &mut Bus, mode: AddressingMode) -> (u16, u8) {
    match mode {
        AddressingMode::Immediate => {
            let addr = cpu.pc;
            cpu.pc = cpu.pc.wrapping_add(1);
            (addr, 0)
        }
        AddressingMode::ZeroPage => {
            let addr = bus.cpu_read(cpu.pc) as u16;
            cpu.pc = cpu.pc.wrapping_add(1);
            (addr, 0)
        }
        AddressingMode::ZeroPageX => {
            let base = bus.cpu_read(cpu.pc);
            cpu.pc = cpu.pc.wrapping_add(1);
            (base.wrapping_add(cpu.x) as u16, 0)
        }
        AddressingMode::ZeroPageY => {
            let base = bus.cpu_read(cpu.pc);
            cpu.pc = cpu.pc.wrapping_add(1);
            (base.wrapping_add(cpu.y) as u16, 0)
        }
        AddressingMode::Absolute => {
            let lo = bus.cpu_read(cpu.pc) as u16;
            let hi = bus.cpu_read(cpu.pc.wrapping_add(1)) as u16;
            cpu.pc = cpu.pc.wrapping_add(2);
            ((hi << 8) | lo, 0)
        }
        AddressingMode::AbsoluteX => {
            let lo = bus.cpu_read(cpu.pc) as u16;
            let hi = bus.cpu_read(cpu.pc.wrapping_add(1)) as u16;
            cpu.pc = cpu.pc.wrapping_add(2);
            let base = (hi << 8) | lo;
            let addr = base.wrapping_add(cpu.x as u16);
            let extra = if Cpu::pages_differ(base, addr) { 1 } else { 0 };
            (addr, extra)
        }
        AddressingMode::AbsoluteY => {
            let lo = bus.cpu_read(cpu.pc) as u16;
            let hi = bus.cpu_read(cpu.pc.wrapping_add(1)) as u16;
            cpu.pc = cpu.pc.wrapping_add(2);
            let base = (hi << 8) | lo;
            let addr = base.wrapping_add(cpu.y as u16);
            let extra = if Cpu::pages_differ(base, addr) { 1 } else { 0 };
            (addr, extra)
        }
        AddressingMode::Indirect => {
            // Only used by JMP - handled inline in CPU, but provide for completeness
            let ptr_lo = bus.cpu_read(cpu.pc) as u16;
            let ptr_hi = bus.cpu_read(cpu.pc.wrapping_add(1)) as u16;
            cpu.pc = cpu.pc.wrapping_add(2);
            let ptr = (ptr_hi << 8) | ptr_lo;
            let lo = bus.cpu_read(ptr) as u16;
            let hi_addr = if ptr_lo == 0xFF { ptr & 0xFF00 } else { ptr + 1 };
            let hi = bus.cpu_read(hi_addr) as u16;
            ((hi << 8) | lo, 0)
        }
        AddressingMode::IndirectX => {
            let base = bus.cpu_read(cpu.pc);
            cpu.pc = cpu.pc.wrapping_add(1);
            let ptr = base.wrapping_add(cpu.x);
            let lo = bus.cpu_read(ptr as u16) as u16;
            let hi = bus.cpu_read(ptr.wrapping_add(1) as u16) as u16;
            ((hi << 8) | lo, 0)
        }
        AddressingMode::IndirectY => {
            let ptr = bus.cpu_read(cpu.pc);
            cpu.pc = cpu.pc.wrapping_add(1);
            let lo = bus.cpu_read(ptr as u16) as u16;
            let hi = bus.cpu_read(ptr.wrapping_add(1) as u16) as u16;
            let base = (hi << 8) | lo;
            let addr = base.wrapping_add(cpu.y as u16);
            let extra = if Cpu::pages_differ(base, addr) { 1 } else { 0 };
            (addr, extra)
        }
        AddressingMode::Relative | AddressingMode::Implied | AddressingMode::Accumulator | AddressingMode::None => {
            // These don't resolve to a memory address through the normal path
            (0, 0)
        }
    }
}
