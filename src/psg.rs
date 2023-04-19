#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use tracing::trace;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AY38910 {
    registers: [u8; 16],
    selected_register: u8,
}

impl AY38910 {
    pub fn new() -> Self {
        Self {
            registers: [0; 16],
            selected_register: 0,
            // ... (Initialize other fields)
        }
    }

    pub fn reset(&mut self) {
        self.registers = [0; 16];
        self.selected_register = 0;
        // ... (Reset other fields)
    }

    pub fn generate_sample(&mut self) -> f32 {
        // Generate a single audio sample
        todo!()
    }

    pub fn read(&mut self, port: u8) -> u8 {
        match port {
            0xA0 => self.selected_register,
            0xA1 => self.registers[self.selected_register as usize],
            _ => 0,
        }
    }

    pub fn write(&mut self, port: u8, data: u8) {
        match port {
            0xA0 => {
                trace!("[psg] Selecting register {:02X}", data);
                self.selected_register = data & 0x0F;
            }
            0xA1 => {
                trace!(
                    "[psg] Writing {:02X} to register {:02X}",
                    data,
                    self.selected_register
                );
                self.registers[self.selected_register as usize] = data;
                // ... (Update the internal state of the PSG based on the new register value)
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct AudioChannel {
    period_a: u8,
    period_a_counter: u8,
    current_sample_a: f32,
    amplitude_a: f32,
    tone_a: bool,
    noise_a: bool,
    envelope_a: bool,

    period_b: u8,
    period_b_counter: u8,
    current_sample_b: f32,
    amplitude_b: f32,
    tone_b: bool,
    noise_b: bool,
    envelope_b: bool,

    period_c: u8,
    period_c_counter: u8,
    current_sample_c: f32,
    amplitude_c: f32,
    tone_c: bool,
    noise_c: bool,
    envelope_c: bool,

    period_n: u8,
    period_n_countdown: u8,
    current_sample_n: f32,

    period_e: u8,
    period_e_countdown: u8,
    current_value_e: f32,
    direction_e: i8,
    continue_e: bool,
    attack_e: bool,
    alternate_e: bool,
    hold_e: bool,

    pulse_signal: bool,
    pulse_signal_on_clocks: u8,
    current_sample_p: f32,

    sample_result: [u8; 2],

    volume_curve: Vec<f32>,

    vol_pan_l: [f32; 4],
    vol_pan_r: [f32; 4],

    lfsr: u32,
}

impl AudioChannel {
    pub fn new() -> Self {
        let mut volume_curve = Vec::new();
        for i in 0..16 {
            let volume = (i as f32) / 15.0;
            let volume = volume.powf(CHANNEL_VOLUME_CURVE_POWER as f32);
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
        // this.setMixerControl(0xff);
        // this.setAmplitudeA(0);
        // this.setAmplitudeB(0);
        // this.setAmplitudeC(0);
        // pulseSignal = false; pulseSignalOnClock = 0; currentSampleP = 0;
    }

    fn set_period_a(&mut self, new_period: u8) {
        if self.period_a == new_period {
            return;
        }
        if new_period < 2 {
            self.period_a = 0;
            self.current_sample_a = 1.0;
        } else {
            self.period_a = new_period;
        }
    }

    fn set_period_b(&mut self, new_period: u8) {
        if self.period_b == new_period {
            return;
        }
        if new_period < 2 {
            self.period_b = 0;
            self.current_sample_b = 1.0;
        } else {
            self.period_b = new_period;
        }
    }

    fn set_period_c(&mut self, new_period: u8) {
        if self.period_c == new_period {
            return;
        }
        if new_period < 2 {
            self.period_c = 0;
            self.current_sample_c = 1.0;
        } else {
            self.period_c = new_period;
        }
    }

    fn set_period_n(&mut self, new_period: u8) {
        if self.period_n == new_period {
            return;
        }
        self.period_n = if new_period < 1 { 1 } else { new_period };
    }

    fn set_period_e(&mut self, new_period: u8) {
        if self.period_e == new_period {
            return;
        }
        self.period_e = if new_period < 1 { 1 } else { new_period };
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
        if self.period_a > 0 {
            self.period_a_counter += 2;
            if self.period_a_counter >= self.period_a {
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

        let vol_pan = !self.vol_pan_l.is_empty() && !self.vol_pan_r.is_empty();
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
                // if !self.pulse_signal
                //     && self.get_bus_cycles() - self.pulse_signal_on_clocks >= MIN_PULSE_ON_CLOCKS
                // {
                //     self.current_sample_p = 0.0;
                // }
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
            let m_sample_result = if self.amplitude_a == 0.0
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
            // if self.current_sample_p != 0.0 {
            //     if !self.pulse_signal
            //         && self.get_bus_cycles() - self.pulse_signal_on_clocks >= MIN_PULSE_ON_CLOCKS
            //     {
            //         self.current_sample_p = 0.0;
            //     }
            //     m_sample_result += CHANNEL_MAX_VOLUME;
            // }
            [m_sample_result as u8, m_sample_result as u8]
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

const CHANNEL_MAX_VOLUME: f32 = 0.25;
const CHANNEL_VOLUME_CURVE_POWER: u8 = 30;

const MIN_PULSE_ON_CLOCKS: u8 = 160;

const BASE_VOLUME: f32 = 0.66;
const SAMPLE_RATE: u32 = 112005; // Main CPU clock / 32 = 112005 Hz

const VOL: &str = "F";
const PAN: &str = "0";
