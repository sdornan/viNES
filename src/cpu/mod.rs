pub mod opcodes;
pub mod addressing;
pub mod trace;

use bitflags::bitflags;
use crate::bus::Bus;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct CpuFlags: u8 {
        const CARRY     = 0b0000_0001;
        const ZERO      = 0b0000_0010;
        const IRQ_DIS   = 0b0000_0100;
        const DECIMAL   = 0b0000_1000;
        const BREAK     = 0b0001_0000;
        const BREAK2    = 0b0010_0000;
        const OVERFLOW  = 0b0100_0000;
        const NEGATIVE  = 0b1000_0000;
    }
}

#[derive(Clone)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: CpuFlags,
    pub cycles: u64,
    pub stall: u16,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: CpuFlags::from_bits_truncate(0x24), // IRQ disabled, BREAK2 set
            cycles: 0,
            stall: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = CpuFlags::from_bits_truncate(0x24);

        let lo = bus.cpu_read(0xFFFC) as u16;
        let hi = bus.cpu_read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles = 7;
    }

    pub fn nmi(&mut self, bus: &mut Bus) {
        self.push_u16(bus, self.pc);
        let flags = (self.status.bits() | 0x20) & !0x10; // set bit 5, clear bit 4
        self.push(bus, flags);
        self.status.insert(CpuFlags::IRQ_DIS);

        let lo = bus.cpu_read(0xFFFA) as u16;
        let hi = bus.cpu_read(0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles += 7;
    }

    pub fn irq(&mut self, bus: &mut Bus) {
        if self.status.contains(CpuFlags::IRQ_DIS) {
            return;
        }
        self.push_u16(bus, self.pc);
        let flags = (self.status.bits() | 0x20) & !0x10;
        self.push(bus, flags);
        self.status.insert(CpuFlags::IRQ_DIS);

        let lo = bus.cpu_read(0xFFFE) as u16;
        let hi = bus.cpu_read(0xFFFF) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles += 7;
    }

    fn push(&mut self, bus: &mut Bus, val: u8) {
        bus.cpu_write(0x0100 | self.sp as u16, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pull(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.cpu_read(0x0100 | self.sp as u16)
    }

    fn push_u16(&mut self, bus: &mut Bus, val: u16) {
        self.push(bus, (val >> 8) as u8);
        self.push(bus, val as u8);
    }

    fn pull_u16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.pull(bus) as u16;
        let hi = self.pull(bus) as u16;
        (hi << 8) | lo
    }

    fn update_zero_negative(&mut self, val: u8) {
        self.status.set(CpuFlags::ZERO, val == 0);
        self.status.set(CpuFlags::NEGATIVE, val & 0x80 != 0);
    }

    fn pages_differ(a: u16, b: u16) -> bool {
        (a & 0xFF00) != (b & 0xFF00)
    }

    fn branch(&mut self, bus: &mut Bus, condition: bool) -> u8 {
        let offset = bus.cpu_read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        if condition {
            let new_pc = self.pc.wrapping_add(offset as u16);
            let extra = if Self::pages_differ(self.pc, new_pc) { 2 } else { 1 };
            self.pc = new_pc;
            extra
        } else {
            0
        }
    }

    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        if self.stall > 0 {
            self.stall -= 1;
            self.cycles += 1;
            return 1;
        }

        let opcode = bus.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        let (cycles, extra) = self.execute(bus, opcode);
        let total = cycles + extra;
        self.cycles += total as u64;
        total
    }

    fn execute(&mut self, bus: &mut Bus, opcode: u8) -> (u8, u8) {
        let info = &opcodes::OPCODES[opcode as usize];
        let mode = info.mode;

        match opcode {
            // === LDA ===
            0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                self.a = bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                (info.cycles, extra)
            }
            // === LDX ===
            0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => {
                let (addr, extra) = self.resolve_address(bus, mode);
                self.x = bus.cpu_read(addr);
                self.update_zero_negative(self.x);
                (info.cycles, extra)
            }
            // === LDY ===
            0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => {
                let (addr, extra) = self.resolve_address(bus, mode);
                self.y = bus.cpu_read(addr);
                self.update_zero_negative(self.y);
                (info.cycles, extra)
            }
            // === STA ===
            0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => {
                let (addr, _) = self.resolve_address(bus, mode);
                bus.cpu_write(addr, self.a);
                (info.cycles, 0)
            }
            // === STX ===
            0x86 | 0x96 | 0x8E => {
                let (addr, _) = self.resolve_address(bus, mode);
                bus.cpu_write(addr, self.x);
                (info.cycles, 0)
            }
            // === STY ===
            0x84 | 0x94 | 0x8C => {
                let (addr, _) = self.resolve_address(bus, mode);
                bus.cpu_write(addr, self.y);
                (info.cycles, 0)
            }

            // === Transfers ===
            0xAA => { self.x = self.a; self.update_zero_negative(self.x); (info.cycles, 0) } // TAX
            0xA8 => { self.y = self.a; self.update_zero_negative(self.y); (info.cycles, 0) } // TAY
            0xBA => { self.x = self.sp; self.update_zero_negative(self.x); (info.cycles, 0) } // TSX
            0x8A => { self.a = self.x; self.update_zero_negative(self.a); (info.cycles, 0) } // TXA
            0x9A => { self.sp = self.x; (info.cycles, 0) } // TXS
            0x98 => { self.a = self.y; self.update_zero_negative(self.a); (info.cycles, 0) } // TYA

            // === ADC ===
            0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.adc(val);
                (info.cycles, extra)
            }
            // === SBC ===
            0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xE1 | 0xF1 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.sbc(val);
                (info.cycles, extra)
            }

            // === AND ===
            0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                self.a &= bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                (info.cycles, extra)
            }
            // === ORA ===
            0x09 | 0x05 | 0x15 | 0x0D | 0x1D | 0x19 | 0x01 | 0x11 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                self.a |= bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                (info.cycles, extra)
            }
            // === EOR ===
            0x49 | 0x45 | 0x55 | 0x4D | 0x5D | 0x59 | 0x41 | 0x51 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                self.a ^= bus.cpu_read(addr);
                self.update_zero_negative(self.a);
                (info.cycles, extra)
            }

            // === ASL ===
            0x0A => { // Accumulator
                let carry = self.a & 0x80 != 0;
                self.a <<= 1;
                self.status.set(CpuFlags::CARRY, carry);
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            0x06 | 0x16 | 0x0E | 0x1E => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                let carry = val & 0x80 != 0;
                val <<= 1;
                bus.cpu_write(addr, val);
                self.status.set(CpuFlags::CARRY, carry);
                self.update_zero_negative(val);
                (info.cycles, 0)
            }
            // === LSR ===
            0x4A => { // Accumulator
                let carry = self.a & 0x01 != 0;
                self.a >>= 1;
                self.status.set(CpuFlags::CARRY, carry);
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            0x46 | 0x56 | 0x4E | 0x5E => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                let carry = val & 0x01 != 0;
                val >>= 1;
                bus.cpu_write(addr, val);
                self.status.set(CpuFlags::CARRY, carry);
                self.update_zero_negative(val);
                (info.cycles, 0)
            }
            // === ROL ===
            0x2A => { // Accumulator
                let old_carry = self.status.contains(CpuFlags::CARRY) as u8;
                let new_carry = self.a & 0x80 != 0;
                self.a = (self.a << 1) | old_carry;
                self.status.set(CpuFlags::CARRY, new_carry);
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            0x26 | 0x36 | 0x2E | 0x3E => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                let old_carry = self.status.contains(CpuFlags::CARRY) as u8;
                let new_carry = val & 0x80 != 0;
                val = (val << 1) | old_carry;
                bus.cpu_write(addr, val);
                self.status.set(CpuFlags::CARRY, new_carry);
                self.update_zero_negative(val);
                (info.cycles, 0)
            }
            // === ROR ===
            0x6A => { // Accumulator
                let old_carry = self.status.contains(CpuFlags::CARRY) as u8;
                let new_carry = self.a & 0x01 != 0;
                self.a = (self.a >> 1) | (old_carry << 7);
                self.status.set(CpuFlags::CARRY, new_carry);
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            0x66 | 0x76 | 0x6E | 0x7E => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                let old_carry = self.status.contains(CpuFlags::CARRY) as u8;
                let new_carry = val & 0x01 != 0;
                val = (val >> 1) | (old_carry << 7);
                bus.cpu_write(addr, val);
                self.status.set(CpuFlags::CARRY, new_carry);
                self.update_zero_negative(val);
                (info.cycles, 0)
            }

            // === CMP ===
            0xC9 | 0xC5 | 0xD5 | 0xCD | 0xDD | 0xD9 | 0xC1 | 0xD1 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.compare(self.a, val);
                (info.cycles, extra)
            }
            // === CPX ===
            0xE0 | 0xE4 | 0xEC => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.compare(self.x, val);
                (info.cycles, extra)
            }
            // === CPY ===
            0xC0 | 0xC4 | 0xCC => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.compare(self.y, val);
                (info.cycles, extra)
            }

            // === INC ===
            0xE6 | 0xF6 | 0xEE | 0xFE => {
                let (addr, _) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr).wrapping_add(1);
                bus.cpu_write(addr, val);
                self.update_zero_negative(val);
                (info.cycles, 0)
            }
            // === DEC ===
            0xC6 | 0xD6 | 0xCE | 0xDE => {
                let (addr, _) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr).wrapping_sub(1);
                bus.cpu_write(addr, val);
                self.update_zero_negative(val);
                (info.cycles, 0)
            }
            // INX
            0xE8 => { self.x = self.x.wrapping_add(1); self.update_zero_negative(self.x); (info.cycles, 0) }
            // INY
            0xC8 => { self.y = self.y.wrapping_add(1); self.update_zero_negative(self.y); (info.cycles, 0) }
            // DEX
            0xCA => { self.x = self.x.wrapping_sub(1); self.update_zero_negative(self.x); (info.cycles, 0) }
            // DEY
            0x88 => { self.y = self.y.wrapping_sub(1); self.update_zero_negative(self.y); (info.cycles, 0) }

            // === Branches ===
            0x90 => { let e = self.branch(bus, !self.status.contains(CpuFlags::CARRY)); (info.cycles, e) }    // BCC
            0xB0 => { let e = self.branch(bus, self.status.contains(CpuFlags::CARRY)); (info.cycles, e) }     // BCS
            0xF0 => { let e = self.branch(bus, self.status.contains(CpuFlags::ZERO)); (info.cycles, e) }      // BEQ
            0xD0 => { let e = self.branch(bus, !self.status.contains(CpuFlags::ZERO)); (info.cycles, e) }     // BNE
            0x30 => { let e = self.branch(bus, self.status.contains(CpuFlags::NEGATIVE)); (info.cycles, e) }  // BMI
            0x10 => { let e = self.branch(bus, !self.status.contains(CpuFlags::NEGATIVE)); (info.cycles, e) } // BPL
            0x50 => { let e = self.branch(bus, !self.status.contains(CpuFlags::OVERFLOW)); (info.cycles, e) } // BVC
            0x70 => { let e = self.branch(bus, self.status.contains(CpuFlags::OVERFLOW)); (info.cycles, e) }  // BVS

            // === JMP ===
            0x4C => { // Absolute
                let lo = bus.cpu_read(self.pc) as u16;
                let hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                self.pc = (hi << 8) | lo;
                (info.cycles, 0)
            }
            0x6C => { // Indirect (with page boundary bug)
                let ptr_lo = bus.cpu_read(self.pc) as u16;
                let ptr_hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                let ptr = (ptr_hi << 8) | ptr_lo;

                let lo = bus.cpu_read(ptr) as u16;
                // 6502 bug: wraps within page instead of crossing
                let hi_addr = if ptr_lo == 0xFF {
                    ptr & 0xFF00
                } else {
                    ptr.wrapping_add(1)
                };
                let hi = bus.cpu_read(hi_addr) as u16;
                self.pc = (hi << 8) | lo;
                (info.cycles, 0)
            }
            // === JSR ===
            0x20 => {
                let lo = bus.cpu_read(self.pc) as u16;
                let hi = bus.cpu_read(self.pc.wrapping_add(1)) as u16;
                let target = (hi << 8) | lo;
                self.push_u16(bus, self.pc.wrapping_add(1)); // push return addr - 1
                self.pc = target;
                (info.cycles, 0)
            }
            // === RTS ===
            0x60 => {
                let addr = self.pull_u16(bus);
                self.pc = addr.wrapping_add(1);
                (info.cycles, 0)
            }
            // === RTI ===
            0x40 => {
                let flags = self.pull(bus);
                self.status = CpuFlags::from_bits_truncate((flags & 0xCF) | (self.status.bits() & 0x30));
                self.status.insert(CpuFlags::BREAK2);
                self.pc = self.pull_u16(bus);
                (info.cycles, 0)
            }

            // === Stack ===
            0x48 => { let a = self.a; self.push(bus, a); (info.cycles, 0) } // PHA
            0x08 => { // PHP
                let flags = self.status.bits() | 0x30; // set B and bit 5
                self.push(bus, flags);
                (info.cycles, 0)
            }
            0x68 => { // PLA
                self.a = self.pull(bus);
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            0x28 => { // PLP
                let flags = self.pull(bus);
                self.status = CpuFlags::from_bits_truncate((flags & 0xCF) | (self.status.bits() & 0x30));
                self.status.insert(CpuFlags::BREAK2);
                (info.cycles, 0)
            }

            // === Flags ===
            0x18 => { self.status.remove(CpuFlags::CARRY); (info.cycles, 0) }    // CLC
            0xD8 => { self.status.remove(CpuFlags::DECIMAL); (info.cycles, 0) }  // CLD
            0x58 => { self.status.remove(CpuFlags::IRQ_DIS); (info.cycles, 0) }  // CLI
            0xB8 => { self.status.remove(CpuFlags::OVERFLOW); (info.cycles, 0) } // CLV
            0x38 => { self.status.insert(CpuFlags::CARRY); (info.cycles, 0) }    // SEC
            0xF8 => { self.status.insert(CpuFlags::DECIMAL); (info.cycles, 0) }  // SED
            0x78 => { self.status.insert(CpuFlags::IRQ_DIS); (info.cycles, 0) }  // SEI

            // === BIT ===
            0x24 | 0x2C => {
                let (addr, _) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.status.set(CpuFlags::ZERO, self.a & val == 0);
                self.status.set(CpuFlags::OVERFLOW, val & 0x40 != 0);
                self.status.set(CpuFlags::NEGATIVE, val & 0x80 != 0);
                (info.cycles, 0)
            }

            // === BRK ===
            0x00 => {
                self.pc = self.pc.wrapping_add(1); // BRK skips the byte after it
                self.push_u16(bus, self.pc);
                let flags = self.status.bits() | 0x30; // set B and bit 5
                self.push(bus, flags);
                self.status.insert(CpuFlags::IRQ_DIS);

                let lo = bus.cpu_read(0xFFFE) as u16;
                let hi = bus.cpu_read(0xFFFF) as u16;
                self.pc = (hi << 8) | lo;
                (info.cycles, 0)
            }

            // === NOP ===
            0xEA => (info.cycles, 0),

            // Unofficial NOPs (various sizes)
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => (info.cycles, 0), // 1-byte NOPs
            0x04 | 0x44 | 0x64 => { // 2-byte NOPs (zero page)
                self.pc = self.pc.wrapping_add(1);
                (info.cycles, 0)
            }
            0x0C => { // 3-byte NOP (absolute)
                self.pc = self.pc.wrapping_add(2);
                (info.cycles, 0)
            }
            0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => { // 2-byte NOPs (zero page X)
                self.pc = self.pc.wrapping_add(1);
                (info.cycles, 0)
            }
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => { // 3-byte NOPs (absolute X)
                let (_, extra) = self.resolve_address(bus, mode);
                (info.cycles, extra)
            }
            0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => { // 2-byte NOPs (immediate)
                self.pc = self.pc.wrapping_add(1);
                (info.cycles, 0)
            }

            // === Unofficial opcodes used by some games ===
            // LAX (LDA + LDX)
            0xA7 | 0xB7 | 0xAF | 0xBF | 0xA3 | 0xB3 => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.a = val;
                self.x = val;
                self.update_zero_negative(val);
                (info.cycles, extra)
            }
            // SAX (store A & X)
            0x87 | 0x97 | 0x83 | 0x8F => {
                let (addr, _) = self.resolve_address(bus, mode);
                bus.cpu_write(addr, self.a & self.x);
                (info.cycles, 0)
            }
            // DCP (DEC + CMP)
            0xC7 | 0xD7 | 0xCF | 0xDF | 0xDB | 0xC3 | 0xD3 => {
                let (addr, _) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr).wrapping_sub(1);
                bus.cpu_write(addr, val);
                self.compare(self.a, val);
                (info.cycles, 0)
            }
            // ISB/ISC (INC + SBC)
            0xE7 | 0xF7 | 0xEF | 0xFF | 0xFB | 0xE3 | 0xF3 => {
                let (addr, _) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr).wrapping_add(1);
                bus.cpu_write(addr, val);
                self.sbc(val);
                (info.cycles, 0)
            }
            // SLO (ASL + ORA)
            0x07 | 0x17 | 0x0F | 0x1F | 0x1B | 0x03 | 0x13 => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                self.status.set(CpuFlags::CARRY, val & 0x80 != 0);
                val <<= 1;
                bus.cpu_write(addr, val);
                self.a |= val;
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            // RLA (ROL + AND)
            0x27 | 0x37 | 0x2F | 0x3F | 0x3B | 0x23 | 0x33 => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                let old_carry = self.status.contains(CpuFlags::CARRY) as u8;
                self.status.set(CpuFlags::CARRY, val & 0x80 != 0);
                val = (val << 1) | old_carry;
                bus.cpu_write(addr, val);
                self.a &= val;
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            // SRE (LSR + EOR)
            0x47 | 0x57 | 0x4F | 0x5F | 0x5B | 0x43 | 0x53 => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                self.status.set(CpuFlags::CARRY, val & 0x01 != 0);
                val >>= 1;
                bus.cpu_write(addr, val);
                self.a ^= val;
                self.update_zero_negative(self.a);
                (info.cycles, 0)
            }
            // RRA (ROR + ADC)
            0x67 | 0x77 | 0x6F | 0x7F | 0x7B | 0x63 | 0x73 => {
                let (addr, _) = self.resolve_address(bus, mode);
                let mut val = bus.cpu_read(addr);
                let old_carry = self.status.contains(CpuFlags::CARRY) as u8;
                self.status.set(CpuFlags::CARRY, val & 0x01 != 0);
                val = (val >> 1) | (old_carry << 7);
                bus.cpu_write(addr, val);
                self.adc(val);
                (info.cycles, 0)
            }
            // SBC unofficial duplicate
            0xEB => {
                let (addr, extra) = self.resolve_address(bus, mode);
                let val = bus.cpu_read(addr);
                self.sbc(val);
                (info.cycles, extra)
            }

            // Catch-all for remaining unofficial opcodes - treat as NOP
            _ => {
                // Advance PC past operand bytes
                let bytes = info.bytes;
                if bytes > 1 {
                    self.pc = self.pc.wrapping_add(bytes as u16 - 1);
                }
                (info.cycles, 0)
            }
        }
    }

    fn adc(&mut self, val: u8) {
        let carry = self.status.contains(CpuFlags::CARRY) as u16;
        let sum = self.a as u16 + val as u16 + carry;
        self.status.set(CpuFlags::CARRY, sum > 0xFF);
        let result = sum as u8;
        self.status.set(
            CpuFlags::OVERFLOW,
            (self.a ^ result) & (val ^ result) & 0x80 != 0,
        );
        self.a = result;
        self.update_zero_negative(self.a);
    }

    fn sbc(&mut self, val: u8) {
        self.adc(val ^ 0xFF); // SBC = ADC with complement
    }

    fn compare(&mut self, reg: u8, val: u8) {
        let result = reg.wrapping_sub(val);
        self.status.set(CpuFlags::CARRY, reg >= val);
        self.update_zero_negative(result);
    }

    fn resolve_address(&mut self, bus: &mut Bus, mode: addressing::AddressingMode) -> (u16, u8) {
        addressing::resolve(self, bus, mode)
    }
}
