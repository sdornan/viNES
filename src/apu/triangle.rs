use super::pulse::LENGTH_TABLE;

const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

#[derive(Clone)]
pub struct Triangle {
    pub enabled: bool,

    // Timer
    timer_period: u16,
    timer_counter: u16,

    // Sequencer
    seq_pos: u8,

    // Length counter
    pub length_counter: u8,
    length_halt: bool, // also controls linear counter reload flag

    // Linear counter
    linear_counter: u8,
    linear_period: u8,
    linear_reload: bool,
}

impl Triangle {
    pub fn new() -> Self {
        Triangle {
            enabled: false,
            timer_period: 0,
            timer_counter: 0,
            seq_pos: 0,
            length_counter: 0,
            length_halt: false,
            linear_counter: 0,
            linear_period: 0,
            linear_reload: false,
        }
    }

    // $4008
    pub fn write_linear(&mut self, val: u8) {
        self.length_halt = val & 0x80 != 0;
        self.linear_period = val & 0x7F;
    }

    // $400A
    pub fn write_timer_lo(&mut self, val: u8) {
        self.timer_period = (self.timer_period & 0x0700) | val as u16;
    }

    // $400B
    pub fn write_timer_hi(&mut self, val: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | ((val as u16 & 7) << 8);
        if self.enabled {
            self.length_counter = LENGTH_TABLE[(val >> 3) as usize];
        }
        self.linear_reload = true;
    }

    /// Clock the timer (called every CPU cycle — triangle runs at CPU rate, not APU half-rate)
    pub fn tick_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.seq_pos = (self.seq_pos + 1) & 31;
            }
        } else {
            self.timer_counter -= 1;
        }
    }

    /// Clock the linear counter (quarter frame)
    pub fn tick_linear(&mut self) {
        if self.linear_reload {
            self.linear_counter = self.linear_period;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.length_halt {
            self.linear_reload = false;
        }
    }

    /// Clock the length counter (half frame)
    pub fn tick_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.linear_counter == 0 {
            return 0;
        }
        TRIANGLE_SEQUENCE[self.seq_pos as usize]
    }
}
