use crate::bus::Bus;
use super::Cpu;
use super::opcodes::OPCODES;

impl Cpu {
    /// Generate a nestest-compatible trace line for the current instruction.
    /// Format: "C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD CYC:7"
    pub fn trace(&self, bus: &mut Bus) -> String {
        let pc = self.pc;
        let opcode = bus.cpu_read(pc);
        let info = &OPCODES[opcode as usize];

        let bytes: Vec<u8> = (0..info.bytes as u16)
            .map(|i| bus.cpu_read(pc.wrapping_add(i)))
            .collect();

        let hex_bytes = match info.bytes {
            1 => format!("{:02X}      ", bytes[0]),
            2 => format!("{:02X} {:02X}   ", bytes[0], bytes[1]),
            3 => format!("{:02X} {:02X} {:02X}", bytes[0], bytes[1], bytes[2]),
            _ => format!("{:02X}      ", bytes[0]),
        };

        format!(
            "{:04X}  {}  {:4} {:27}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}",
            pc,
            hex_bytes,
            info.mnemonic,
            "", // operand disassembly placeholder
            self.a,
            self.x,
            self.y,
            self.status.bits(),
            self.sp,
            self.cycles,
        )
    }
}
