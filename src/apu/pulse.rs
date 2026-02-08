const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 75% (inverted 25%)
];

pub const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

pub struct Pulse {
    pub enabled: bool,
    pub channel: u8, // 0 = pulse1, 1 = pulse2

    // Duty
    duty_mode: u8,
    duty_pos: u8,

    // Timer
    timer_period: u16,
    timer_counter: u16,

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

    // Sweep
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_divider: u8,
    sweep_reload: bool,
}

impl Pulse {
    pub fn new(channel: u8) -> Self {
        Pulse {
            enabled: false,
            channel,
            duty_mode: 0,
            duty_pos: 0,
            timer_period: 0,
            timer_counter: 0,
            length_counter: 0,
            length_halt: false,
            envelope_start: false,
            envelope_loop: false,
            constant_volume: false,
            envelope_period: 0,
            envelope_divider: 0,
            envelope_decay: 0,
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_divider: 0,
            sweep_reload: false,
        }
    }

    // $4000/$4004
    pub fn write_control(&mut self, val: u8) {
        self.duty_mode = (val >> 6) & 3;
        self.length_halt = val & 0x20 != 0;
        self.envelope_loop = val & 0x20 != 0;
        self.constant_volume = val & 0x10 != 0;
        self.envelope_period = val & 0x0F;
    }

    // $4001/$4005
    pub fn write_sweep(&mut self, val: u8) {
        self.sweep_enabled = val & 0x80 != 0;
        self.sweep_period = (val >> 4) & 7;
        self.sweep_negate = val & 0x08 != 0;
        self.sweep_shift = val & 7;
        self.sweep_reload = true;
    }

    // $4002/$4006
    pub fn write_timer_lo(&mut self, val: u8) {
        self.timer_period = (self.timer_period & 0x0700) | val as u16;
    }

    // $4003/$4007
    pub fn write_timer_hi(&mut self, val: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | ((val as u16 & 7) << 8);
        if self.enabled {
            self.length_counter = LENGTH_TABLE[(val >> 3) as usize];
        }
        self.duty_pos = 0;
        self.envelope_start = true;
    }

    /// Clock the timer (called every APU cycle = every other CPU cycle)
    pub fn tick_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            self.duty_pos = (self.duty_pos + 1) & 7;
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

    /// Clock the sweep unit (half frame)
    pub fn tick_sweep(&mut self) {
        let target = self.sweep_target_period();
        if self.sweep_divider == 0 && self.sweep_enabled && self.sweep_shift > 0
            && self.timer_period >= 8 && target <= 0x7FF
        {
            self.timer_period = target;
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    /// Clock the length counter (half frame)
    pub fn tick_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn sweep_target_period(&self) -> u16 {
        let shift = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            // Pulse 1 uses one's complement (subtract shift - 1)
            // Pulse 2 uses two's complement (subtract shift)
            if self.channel == 0 {
                self.timer_period.wrapping_sub(shift).wrapping_sub(1)
            } else {
                self.timer_period.wrapping_sub(shift)
            }
        } else {
            self.timer_period.wrapping_add(shift)
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || DUTY_TABLE[self.duty_mode as usize][self.duty_pos as usize] == 0
            || self.timer_period < 8
            || self.sweep_target_period() > 0x7FF
        {
            return 0;
        }
        if self.constant_volume {
            self.envelope_period
        } else {
            self.envelope_decay
        }
    }
}
