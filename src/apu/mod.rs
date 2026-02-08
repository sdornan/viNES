pub mod pulse;
pub mod triangle;
pub mod noise;

use pulse::Pulse;
use triangle::Triangle;
use noise::Noise;
use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

const CPU_FREQ: f64 = 1_789_773.0;
const SAMPLE_RATE: f64 = 44_100.0;
const CYCLES_PER_SAMPLE: f64 = CPU_FREQ / SAMPLE_RATE;

pub struct Apu {
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    pub triangle: Triangle,
    pub noise: Noise,

    // Frame counter
    frame_counter_mode: u8, // 0 = 4-step, 1 = 5-step
    frame_counter: u16,
    irq_inhibit: bool,

    // Downsampling
    sample_accumulator: f64,
    sample_count: f64,
    cycle_fraction: f64,

    // Output buffer
    sample_buffer: Arc<ArrayQueue<f32>>,

    // Cycle parity (APU runs at half CPU rate for pulse/noise)
    odd_cycle: bool,
}

impl Apu {
    pub fn new(sample_buffer: Arc<ArrayQueue<f32>>) -> Self {
        Apu {
            pulse1: Pulse::new(0),
            pulse2: Pulse::new(1),
            triangle: Triangle::new(),
            noise: Noise::new(),
            frame_counter_mode: 0,
            frame_counter: 0,
            irq_inhibit: true,
            sample_accumulator: 0.0,
            sample_count: 0.0,
            cycle_fraction: 0.0,
            sample_buffer,
            odd_cycle: false,
        }
    }

    /// Tick the APU for one CPU cycle.
    pub fn tick(&mut self) {
        // Triangle timer runs at CPU rate
        self.triangle.tick_timer();

        // Pulse and noise timers run at half CPU rate (every other cycle)
        self.odd_cycle = !self.odd_cycle;
        if self.odd_cycle {
            self.pulse1.tick_timer();
            self.pulse2.tick_timer();
            self.noise.tick_timer();
        }

        // Frame counter
        self.frame_counter += 1;
        self.clock_frame_counter();

        // Mix and downsample
        let sample = self.mix();
        self.sample_accumulator += sample;
        self.sample_count += 1.0;
        self.cycle_fraction += 1.0;

        if self.cycle_fraction >= CYCLES_PER_SAMPLE {
            let avg = (self.sample_accumulator / self.sample_count) as f32;
            let _ = self.sample_buffer.push(avg);
            self.sample_accumulator = 0.0;
            self.sample_count = 0.0;
            self.cycle_fraction -= CYCLES_PER_SAMPLE;
        }
    }

    fn clock_frame_counter(&mut self) {
        match self.frame_counter_mode {
            0 => self.clock_4step(),
            1 => self.clock_5step(),
            _ => {}
        }
    }

    fn clock_4step(&mut self) {
        match self.frame_counter {
            3729 => self.quarter_frame(),
            7457 => { self.quarter_frame(); self.half_frame(); }
            11186 => self.quarter_frame(),
            14915 => {
                self.quarter_frame();
                self.half_frame();
                self.frame_counter = 0;
            }
            _ => {}
        }
    }

    fn clock_5step(&mut self) {
        match self.frame_counter {
            3729 => self.quarter_frame(),
            7457 => { self.quarter_frame(); self.half_frame(); }
            11186 => self.quarter_frame(),
            18641 => {
                self.quarter_frame();
                self.half_frame();
                self.frame_counter = 0;
            }
            _ => {}
        }
    }

    fn quarter_frame(&mut self) {
        self.pulse1.tick_envelope();
        self.pulse2.tick_envelope();
        self.triangle.tick_linear();
        self.noise.tick_envelope();
    }

    fn half_frame(&mut self) {
        self.pulse1.tick_length();
        self.pulse1.tick_sweep();
        self.pulse2.tick_length();
        self.pulse2.tick_sweep();
        self.triangle.tick_length();
        self.noise.tick_length();
    }

    /// Mix all channels using the NES non-linear mixing formula (approximated).
    fn mix(&self) -> f64 {
        let p1 = self.pulse1.output() as f64;
        let p2 = self.pulse2.output() as f64;
        let t = self.triangle.output() as f64;
        let n = self.noise.output() as f64;

        // Approximation of the NES DAC mixing
        let pulse_out = if p1 + p2 > 0.0 {
            95.88 / (8128.0 / (p1 + p2) + 100.0)
        } else {
            0.0
        };
        let tnd_out = if t + n > 0.0 {
            159.79 / (1.0 / (t / 8227.0 + n / 12241.0) + 100.0)
        } else {
            0.0
        };

        pulse_out + tnd_out
    }

    // --- Register writes ---

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x4000 => self.pulse1.write_control(val),
            0x4001 => self.pulse1.write_sweep(val),
            0x4002 => self.pulse1.write_timer_lo(val),
            0x4003 => self.pulse1.write_timer_hi(val),
            0x4004 => self.pulse2.write_control(val),
            0x4005 => self.pulse2.write_sweep(val),
            0x4006 => self.pulse2.write_timer_lo(val),
            0x4007 => self.pulse2.write_timer_hi(val),
            0x4008 => self.triangle.write_linear(val),
            0x400A => self.triangle.write_timer_lo(val),
            0x400B => self.triangle.write_timer_hi(val),
            0x400C => self.noise.write_control(val),
            0x400E => self.noise.write_period(val),
            0x400F => self.noise.write_length(val),
            _ => {} // $4009, $400D, $4010-$4013 (DMC) ignored
        }
    }

    // $4015 write
    pub fn write_status(&mut self, val: u8) {
        self.pulse1.enabled = val & 0x01 != 0;
        self.pulse2.enabled = val & 0x02 != 0;
        self.triangle.enabled = val & 0x04 != 0;
        self.noise.enabled = val & 0x08 != 0;

        if !self.pulse1.enabled { self.pulse1.length_counter = 0; }
        if !self.pulse2.enabled { self.pulse2.length_counter = 0; }
        if !self.triangle.enabled { self.triangle.length_counter = 0; }
        if !self.noise.enabled { self.noise.length_counter = 0; }
    }

    // $4015 read
    pub fn read_status(&mut self) -> u8 {
        let mut val = 0u8;
        if self.pulse1.length_counter > 0 { val |= 0x01; }
        if self.pulse2.length_counter > 0 { val |= 0x02; }
        if self.triangle.length_counter > 0 { val |= 0x04; }
        if self.noise.length_counter > 0 { val |= 0x08; }
        val
    }

    // $4017 write
    pub fn write_frame_counter(&mut self, val: u8) {
        self.frame_counter_mode = (val >> 7) & 1;
        self.irq_inhibit = val & 0x40 != 0;
        self.frame_counter = 0;
        if self.frame_counter_mode == 1 {
            // 5-step mode immediately clocks
            self.quarter_frame();
            self.half_frame();
        }
    }
}
