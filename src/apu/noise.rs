use super::pulse::LENGTH_TABLE;

const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct Noise {
    pub enabled: bool,

    // Timer
    timer_period: u16,
    timer_counter: u16,

    // LFSR
    shift_register: u16,
    mode: bool, // false = long mode (bit 1), true = short mode (bit 6)

    // Length counter
    pub length_counter: u8,
    length_halt: bool,

    // Envelope
    envelope_start: bool,
    envelope_loop: bool,
    constant_volume: bool,
    envelope_period: u8,
    envelope_divider: u8,
    envelope_decay: u8,
}

impl Noise {
    pub fn new() -> Self {
        Noise {
            enabled: false,
            timer_period: 0,
            timer_counter: 0,
            shift_register: 1, // initial value
            mode: false,
            length_counter: 0,
            length_halt: false,
            envelope_start: false,
            envelope_loop: false,
            constant_volume: false,
            envelope_period: 0,
            envelope_divider: 0,
            envelope_decay: 0,
        }
    }

    // $400C
    pub fn write_control(&mut self, val: u8) {
        self.length_halt = val & 0x20 != 0;
        self.envelope_loop = val & 0x20 != 0;
        self.constant_volume = val & 0x10 != 0;
        self.envelope_period = val & 0x0F;
    }

    // $400E
    pub fn write_period(&mut self, val: u8) {
        self.mode = val & 0x80 != 0;
        self.timer_period = NOISE_PERIOD_TABLE[(val & 0x0F) as usize];
    }

    // $400F
    pub fn write_length(&mut self, val: u8) {
        if self.enabled {
            self.length_counter = LENGTH_TABLE[(val >> 3) as usize];
        }
        self.envelope_start = true;
    }

    /// Clock the timer (called every APU cycle)
    pub fn tick_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            // Clock LFSR
            let feedback_bit = if self.mode { 6 } else { 1 };
            let feedback = (self.shift_register & 1) ^ ((self.shift_register >> feedback_bit) & 1);
            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
        } else {
            self.timer_counter -= 1;
        }
    }

    /// Clock the envelope (quarter frame)
    pub fn tick_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.envelope_period;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.envelope_period;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if self.envelope_loop {
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    /// Clock the length counter (half frame)
    pub fn tick_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.shift_register & 1 != 0 {
            return 0;
        }
        if self.constant_volume {
            self.envelope_period
        } else {
            self.envelope_decay
        }
    }
}
