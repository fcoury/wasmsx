#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AY38910 {
    registers: [u8; 16],
    selected_register: u8,
    channel: AudioChannel,
    clock_divider: u32,
    sample_counter: u32,
    // Resampling buffer for 112kHz to 44.1kHz conversion
    resample_buffer: Vec<f32>,
    resample_accumulator: f32,
    resample_cycles: u32,
    // Joystick state (0xFF means no buttons pressed)
    pub joystick_port_a: u8,
    pub joystick_port_b: u8,
}

impl AY38910 {
    pub fn new() -> Self {
        let mut psg = Self {
            registers: [0; 16],
            selected_register: 0,
            channel: AudioChannel::new(),
            clock_divider: 0,
            sample_counter: 0,
            resample_buffer: Vec::with_capacity(4096),
            resample_accumulator: 0.0,
            resample_cycles: 0,
            joystick_port_a: 0xFF, // All bits set = no buttons pressed
            joystick_port_b: 0xFF, // All bits set = no buttons pressed
        };

        // Initialize register 7 (mixer) to 0xFF (all channels disabled by default)
        psg.registers[7] = 0xFF;
        psg.channel.set_mixer_control(0xFF);

        psg
    }

    pub fn reset(&mut self) {
        self.registers = [0; 16];
        self.selected_register = 0;
        self.channel.reset();
        self.clock_divider = 0;
        self.sample_counter = 0;
        self.resample_buffer.clear();
        self.resample_accumulator = 0.0;
        self.resample_cycles = 0;
        self.joystick_port_a = 0xFF;
        self.joystick_port_b = 0xFF;
    }

    // Handle joystick button presses
    pub fn joystick_key_down(&mut self, key: String) {
        // Map keyboard keys to joystick bits
        // Space is typically mapped to the fire button (bit 4)
        match key.as_str() {
            "Space" => self.joystick_port_a &= !(1 << 4), // Clear bit 4 (fire button)
            "ArrowUp" => self.joystick_port_a &= !(1 << 0), // Clear bit 0 (up)
            "ArrowDown" => self.joystick_port_a &= !(1 << 1), // Clear bit 1 (down)
            "ArrowLeft" => self.joystick_port_a &= !(1 << 2), // Clear bit 2 (left)
            "ArrowRight" => self.joystick_port_a &= !(1 << 3), // Clear bit 3 (right)
            _ => {}                                       // Ignore other keys
        }
        tracing::info!(
            "[PSG] Joystick key down: {}, state: {:08b}",
            key,
            self.joystick_port_a
        );
    }

    // Handle joystick button releases
    pub fn joystick_key_up(&mut self, key: String) {
        match key.as_str() {
            "Space" => self.joystick_port_a |= 1 << 4, // Set bit 4 (fire button)
            "ArrowUp" => self.joystick_port_a |= 1 << 0, // Set bit 0 (up)
            "ArrowDown" => self.joystick_port_a |= 1 << 1, // Set bit 1 (down)
            "ArrowLeft" => self.joystick_port_a |= 1 << 2, // Set bit 2 (left)
            "ArrowRight" => self.joystick_port_a |= 1 << 3, // Set bit 3 (right)
            _ => {}                                    // Ignore other keys
        }
        tracing::info!(
            "[PSG] Joystick key up: {}, state: {:08b}",
            key,
            self.joystick_port_a
        );
    }

    // Get next audio sample from the resample buffer
    pub fn get_audio_sample(&mut self) -> f32 {
        if !self.resample_buffer.is_empty() {
            self.resample_buffer.remove(0)
        } else {
            0.0
        }
    }

    // Check if we have enough samples in the buffer
    pub fn has_samples(&self, count: usize) -> bool {
        self.resample_buffer.len() >= count
    }

    pub fn clock(&mut self, cycles: u32) {
        // PSG runs at CPU_CLOCK / 8 = ~447kHz for internal updates
        // PSG generates samples at CPU_CLOCK / 32 = ~112kHz
        const PSG_CLOCK_DIVIDER: u32 = 8;
        const PSG_SAMPLE_DIVIDER: u32 = 32;

        // Constants for resampling from 112kHz to 44.1kHz
        const CPU_CLOCK: u32 = 3_579_545;
        const PSG_NATIVE_RATE: u32 = CPU_CLOCK / PSG_SAMPLE_DIVIDER; // ~112kHz
        const AUDIO_SAMPLE_RATE: u32 = 44100;

        self.clock_divider += cycles;
        self.resample_cycles += cycles;

        // Update PSG internal state
        while self.clock_divider >= PSG_CLOCK_DIVIDER {
            self.clock_divider -= PSG_CLOCK_DIVIDER;
            // The channel's next_sample method updates counters internally
        }

        // Generate samples at PSG native rate (112kHz)
        while self.resample_cycles >= PSG_SAMPLE_DIVIDER {
            self.resample_cycles -= PSG_SAMPLE_DIVIDER;

            // Generate next PSG sample
            let samples = self.channel.next_sample();

            // Convert to float in range -1.0 to 1.0
            let raw_value = samples[0] as f32 / 255.0;
            let mono_sample = (raw_value * 0.66 * 2.0) - 1.0;

            // Resample from 112kHz to 44.1kHz
            // PSG_NATIVE_RATE / AUDIO_SAMPLE_RATE ≈ 2.54
            self.resample_accumulator += AUDIO_SAMPLE_RATE as f32 / PSG_NATIVE_RATE as f32;

            while self.resample_accumulator >= 1.0 {
                self.resample_accumulator -= 1.0;
                self.resample_buffer.push(mono_sample);

                // Prevent buffer from growing too large
                if self.resample_buffer.len() > 8192 {
                    self.resample_buffer.drain(0..4096);
                }
            }
        }
    }

    pub fn read(&mut self, port: u8) -> u8 {
        match port {
            0xA0 => self.selected_register,
            0xA1 | 0xA2 => {
                // For register 14 (0x0E), return joystick port A state
                if self.selected_register == 14 {
                    self.joystick_port_a
                } else if self.selected_register == 15 {
                    self.joystick_port_b
                } else {
                    self.registers[self.selected_register as usize]
                }
            }
            _ => 0,
        }
    }

    pub fn write(&mut self, port: u8, data: u8) {
        match port {
            0xA0 => {
                self.selected_register = data & 0x0F;
            }
            0xA1 => {
                self.registers[self.selected_register as usize] = data;
                self.update_channel_from_register(self.selected_register, data);
            }
            _ => {}
        }
    }

    pub fn set_pulse_signal(&mut self, active: bool) {
        self.channel.pulse_signal = active;
        if active {
            self.channel.pulse_signal_on_clocks = self.sample_counter as u8;
            self.channel.current_sample_p = 1.0;
        }
    }

    fn update_channel_from_register(&mut self, reg: u8, value: u8) {
        match reg {
            // Channel A tone period
            0 => {
                // Register 0 is the low byte
                let period = (self.registers[1] as u16) << 8 | value as u16;
                self.channel.set_period_a(period);
            }
            1 => {
                // Register 1 is the high byte
                let period = (value as u16) << 8 | self.registers[0] as u16;
                self.channel.set_period_a(period);
            }
            // Channel B tone period
            2 => {
                let period = (self.registers[3] as u16) << 8 | value as u16;
                self.channel.set_period_b(period);
            }
            3 => {
                let period = (value as u16) << 8 | self.registers[2] as u16;
                self.channel.set_period_b(period);
            }
            // Channel C tone period
            4 => {
                let period = (self.registers[5] as u16) << 8 | value as u16;
                self.channel.set_period_c(period);
            }
            5 => {
                let period = (value as u16) << 8 | self.registers[4] as u16;
                self.channel.set_period_c(period);
            }
            // Noise period
            6 => self.channel.set_period_n(value & 0x1F),
            // Mixer control
            7 => {
                self.channel.set_mixer_control(value);
            }
            // Channel volumes
            8 => self.channel.set_amplitude_a(value & 0x1F),
            9 => self.channel.set_amplitude_b(value & 0x1F),
            10 => self.channel.set_amplitude_c(value & 0x1F),
            // Envelope period
            11 => {
                let period = (self.registers[12] as u16) << 8 | value as u16;
                self.channel.set_period_e(period);
            }
            12 => {
                let period = (value as u16) << 8 | self.registers[11] as u16;
                self.channel.set_period_e(period);
            }
            // Envelope shape
            13 => {
                self.channel.continue_e = (value & 0x08) != 0;
                self.channel.attack_e = (value & 0x04) != 0;
                self.channel.alternate_e = (value & 0x02) != 0;
                self.channel.hold_e = (value & 0x01) != 0;
                self.channel
                    .cycle_envelope(self.channel.alternate_e, self.channel.hold_e);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct AudioChannel {
    period_a: u16,
    period_a_counter: u16,
    current_sample_a: f32,
    amplitude_a: f32,
    tone_a: bool,
    noise_a: bool,
    envelope_a: bool,

    period_b: u16,
    period_b_counter: u16,
    current_sample_b: f32,
    amplitude_b: f32,
    tone_b: bool,
    noise_b: bool,
    envelope_b: bool,

    period_c: u16,
    period_c_counter: u16,
    current_sample_c: f32,
    amplitude_c: f32,
    tone_c: bool,
    noise_c: bool,
    envelope_c: bool,

    period_n: u8,
    period_n_countdown: u8,
    current_sample_n: f32,

    period_e: u16,
    period_e_countdown: u16,
    current_value_e: f32,
    pub direction_e: i8,
    pub continue_e: bool,
    pub attack_e: bool,
    pub alternate_e: bool,
    pub hold_e: bool,

    pub pulse_signal: bool,
    pub pulse_signal_on_clocks: u8,
    pub current_sample_p: f32,

    sample_result: [u8; 2],

    #[serde(skip)]
    volume_curve: Vec<f32>,

    #[serde(skip)]
    vol_pan_l: Vec<f32>,
    #[serde(skip)]
    vol_pan_r: Vec<f32>,

    lfsr: u32,
}

impl AudioChannel {
    pub fn new() -> Self {
        let mut volume_curve = Vec::new();
        // WebMSX volume curve: volumeCurve[v] = Math.pow(2, -(15 - v) / 2) * CHANNEL_MAX_VOLUME
        volume_curve.push(0.0); // Volume 0 is always silent
        for v in 1..16 {
            let volume = (2.0_f32).powf(-((15 - v) as f32) / 2.0) * CHANNEL_MAX_VOLUME;
            volume_curve.push(volume);
        }

        // if (VOLPAN) wmsx.AudioTables.setupVolPan(4, VOL, PAN, volPanL, volPanR);

        Self {
            volume_curve,
            lfsr: 0x01fffe,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.set_mixer_control(0xff);
        self.set_amplitude_a(0);
        self.set_amplitude_b(0);
        self.set_amplitude_c(0);
        self.pulse_signal = false;
        self.pulse_signal_on_clocks = 0;
        self.current_sample_p = 0.0;
    }

    fn set_period_a(&mut self, new_period: u16) {
        let period = new_period & 0xFFF;
        if self.period_a == period {
            return;
        }
        if period < 2 {
            self.period_a = 0;
            self.current_sample_a = 1.0;
        } else {
            self.period_a = period;
        }
    }

    fn set_period_b(&mut self, new_period: u16) {
        let period = new_period & 0xFFF;
        if self.period_b == period {
            return;
        }
        if period < 2 {
            self.period_b = 0;
            self.current_sample_b = 1.0;
        } else {
            self.period_b = period;
        }
    }

    fn set_period_c(&mut self, new_period: u16) {
        let period = new_period & 0xFFF;
        if self.period_c == period {
            return;
        }
        if period < 2 {
            self.period_c = 0;
            self.current_sample_c = 1.0;
        } else {
            self.period_c = period;
        }
    }

    fn set_period_n(&mut self, new_period: u8) {
        if self.period_n == new_period {
            return;
        }
        self.period_n = if new_period < 1 { 1 } else { new_period };
    }

    fn set_period_e(&mut self, new_period: u16) {
        let period = new_period & 0xFFFF;
        if self.period_e == period {
            return;
        }
        self.period_e = if period < 1 { 1 } else { period };
    }

    fn set_amplitude_a(&mut self, new_amplitude: u8) {
        if new_amplitude & 0x10 != 0 {
            self.envelope_a = true;
            self.amplitude_a = self.volume_curve[self.current_value_e as usize];
        } else {
            self.envelope_a = false;
            self.amplitude_a = self.volume_curve[(new_amplitude & 0x0f) as usize];
        }
    }

    fn set_amplitude_b(&mut self, new_amplitude: u8) {
        if new_amplitude & 0x10 != 0 {
            self.envelope_b = true;
            self.amplitude_b = self.volume_curve[self.current_value_e as usize];
        } else {
            self.envelope_b = false;
            self.amplitude_b = self.volume_curve[(new_amplitude & 0x0f) as usize];
        }
    }

    fn set_amplitude_c(&mut self, new_amplitude: u8) {
        if new_amplitude & 0x10 != 0 {
            self.envelope_c = true;
            self.amplitude_c = self.volume_curve[self.current_value_e as usize];
        } else {
            self.envelope_c = false;
            self.amplitude_c = self.volume_curve[(new_amplitude & 0x0f) as usize];
        }
    }

    fn set_mixer_control(&mut self, control: u8) {
        self.tone_a = (control & 0x01) == 0;
        self.noise_a = (control & 0x08) == 0;
        self.tone_b = (control & 0x02) == 0;
        self.noise_b = (control & 0x10) == 0;
        self.tone_c = (control & 0x04) == 0;
        self.noise_c = (control & 0x20) == 0;
    }

    fn next_sample(&mut self) -> [u8; 2] {
        // Update values
        // The PSG runs at CPU_CLOCK / 8, and we're calling this per sample
        // So we need to advance counters appropriately
        // WebMSX increments by 2 per PSG clock
        if self.period_a > 0 {
            self.period_a_counter += 2;
            if self.period_a_counter >= self.period_a {
                // Preserve the remainder (0 or 1) for odd dividers, as the step is 2
                self.period_a_counter = (self.period_a_counter - self.period_a) & 1;
                self.current_sample_a = if self.current_sample_a != 0.0 {
                    0.0
                } else {
                    1.0
                };
            }
        }
        if self.period_b > 0 {
            self.period_b_counter += 2;
            if self.period_b_counter >= self.period_b {
                self.period_b_counter = (self.period_b_counter - self.period_b) & 1;
                self.current_sample_b = if self.current_sample_b != 0.0 {
                    0.0
                } else {
                    1.0
                };
            }
        }
        if self.period_c > 0 {
            self.period_c_counter += 2;
            if self.period_c_counter >= self.period_c {
                self.period_c_counter = (self.period_c_counter - self.period_c) & 1;
                self.current_sample_c = if self.current_sample_c != 0.0 {
                    0.0
                } else {
                    1.0
                };
            }
        }
        if self.noise_a || self.noise_b || self.noise_c {
            self.period_n_countdown += 1;
            if self.period_n_countdown >= self.period_n {
                self.period_n_countdown = 0;
                self.current_sample_n = self.next_lfsr() as f32;
            }
        }
        if self.direction_e != 0 {
            self.period_e_countdown += 1;
            if self.period_e_countdown >= self.period_e {
                self.period_e_countdown = 0;
                self.current_value_e += self.direction_e as f32;
                if self.current_value_e < 0.0 || self.current_value_e > 15.0 {
                    if self.continue_e {
                        self.cycle_envelope(self.alternate_e, self.hold_e);
                    } else {
                        self.attack_e = true;
                        self.cycle_envelope(true, true);
                    }
                }
                self.set_envelope_amplitudes();
            }
        }

        let vol_pan = self.vol_pan_l.len() >= 4 && self.vol_pan_r.len() >= 4;
        if vol_pan {
            // has to be VOLPAN, the const
            // Complete Stereo path (VOL/PAN)
            let sample_a = if self.amplitude_a == 0.0
                || (self.tone_a && self.current_sample_a == 0.0)
                || (self.noise_a && self.current_sample_n == 0.0)
            {
                0.0
            } else {
                self.amplitude_a
            };
            let sample_b = if self.amplitude_b == 0.0
                || (self.tone_b && self.current_sample_b == 0.0)
                || (self.noise_b && self.current_sample_n == 0.0)
            {
                0.0
            } else {
                self.amplitude_b
            };
            let sample_c = if self.amplitude_c == 0.0
                || (self.tone_c && self.current_sample_c == 0.0)
                || (self.noise_c && self.current_sample_n == 0.0)
            {
                0.0
            } else {
                self.amplitude_c
            };
            let sample_p = if self.current_sample_p != 0.0 {
                if !self.pulse_signal
                // && self.get_bus_cycles() - self.pulse_signal_on_clocks >= MIN_PULSE_ON_CLOCKS
                {
                    self.current_sample_p = 0.0;
                }
                CHANNEL_MAX_VOLUME
            } else {
                0.0
            };
            self.sample_result[0] = (sample_a * self.vol_pan_l[0]
                + sample_b * self.vol_pan_l[1]
                + sample_c * self.vol_pan_l[2]
                + sample_p * self.vol_pan_l[3]) as u8;
            self.sample_result[1] = (sample_a * self.vol_pan_r[0]
                + sample_b * self.vol_pan_r[1]
                + sample_c * self.vol_pan_r[2]
                + sample_p * self.vol_pan_r[3]) as u8;
            self.sample_result
        } else {
            // Simple Mono path (no VOL/PAN)
            // WebMSX: Mix tone with noise. Tone or noise if turned off produce a fixed high value (1)
            let mut m_sample_result = if self.amplitude_a == 0.0
                || (self.tone_a && self.current_sample_a == 0.0)
                || (self.noise_a && self.current_sample_n == 0.0)
            {
                0.0
            } else {
                self.amplitude_a
            } + if self.amplitude_b == 0.0
                || (self.tone_b && self.current_sample_b == 0.0)
                || (self.noise_b && self.current_sample_n == 0.0)
            {
                0.0
            } else {
                self.amplitude_b
            } + if self.amplitude_c == 0.0
                || (self.tone_c && self.current_sample_c == 0.0)
                || (self.noise_c && self.current_sample_n == 0.0)
            {
                0.0
            } else {
                self.amplitude_c
            };
            if self.current_sample_p != 0.0 {
                if !self.pulse_signal
                // && self.get_bus_cycles() - self.pulse_signal_on_clocks >= MIN_PULSE_ON_CLOCKS
                {
                    self.current_sample_p = 0.0;
                }
                m_sample_result += CHANNEL_MAX_VOLUME;
            }
            // Return the raw float value, let generate_sample handle conversion
            // WebMSX returns the sum directly (max ~0.84 with 3 channels at 0.28 each)
            [((m_sample_result * 255.0) as u8).min(255), 0]
        }
    }

    fn cycle_envelope(&mut self, alternate: bool, hold: bool) {
        if alternate ^ hold {
            self.attack_e = !self.attack_e;
        }
        self.current_value_e = if self.attack_e { 0.0 } else { 15.0 };
        self.direction_e = if hold {
            0
        } else if self.attack_e {
            1
        } else {
            -1
        };
    }

    fn set_envelope_amplitudes(&mut self) {
        if self.envelope_a {
            self.amplitude_a = self.volume_curve[self.current_value_e as usize];
        }
        if self.envelope_b {
            self.amplitude_b = self.volume_curve[self.current_value_e as usize];
        }
        if self.envelope_c {
            self.amplitude_c = self.volume_curve[self.current_value_e as usize];
        }
    }
    fn next_lfsr(&mut self) -> u32 {
        // bit 16 = bit 2 XOR bit 0
        self.lfsr = (self.lfsr >> 1) | ((((self.lfsr >> 2) ^ (self.lfsr & 0x01)) & 0x01) << 16); // shift right, push to left
        self.lfsr & 0x01
    }

    fn create_volume_curve(&mut self) {
        // Assuming CHANNEL_VOLUME_CURVE_POWER and CHANNEL_MAX_VOLUME are constants
        let channel_volume_curve_power = 2.0; // Replace with the correct value
        let channel_max_volume = 15; // Replace with the correct value

        for v in 0..16 {
            let value = (f32::powf(channel_volume_curve_power, v as f32 / 15.0) - 1.0)
                / (channel_volume_curve_power - 1.0)
                * (channel_max_volume as f32);
            self.volume_curve.push(value);
        }
    }
}

const CHANNEL_MAX_VOLUME: f32 = 0.28;
const CHANNEL_VOLUME_CURVE_POWER: u8 = 30;

const MIN_PULSE_ON_CLOCKS: u8 = 160;

const BASE_VOLUME: f32 = 0.66;
const SAMPLE_RATE: u32 = 112005; // Main CPU clock / 32 = 112005 Hz

const VOL: &str = "F";
const PAN: &str = "0";
