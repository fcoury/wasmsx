#![allow(dead_code)]
use crate::keyboard::Keyboard;

#[derive(Clone, Debug)]
pub struct Ppi {
    pub keyboard: Keyboard,
    pub primary_slot_config: u8,
    register_b: u8,
    register_c: u8,
    control: u8,

    keyboard_row_selected: u8,
}

impl Ppi {
    pub fn new() -> Self {
        Ppi::default()
    }

    pub fn reset(&mut self) {
        self.register_c = 0x50; // Everything OFF. Motor and CapsLed = 1 means OFF
        self.keyboard_row_selected = 0;
        self.update_pulse_signal();
        self.update_caps_led();
    }

    pub fn key_down(&mut self, key: String) {
        self.keyboard.key_down(key);
    }

    pub fn key_up(&mut self, key: String) {
        self.keyboard.key_up(key);
    }

    fn update_pulse_signal(&self) {
        // TODO: psg.set_pulse_signal((register_c & 0xa0) > 0);
    }

    pub fn read(&mut self, port: u8) -> u8 {
        match port {
            0xA8 => {
                // get primary slot config
                tracing::trace!(
                    "[PPI] [RD] [PrimarySlot] [{:02X}] = {:02X}",
                    port,
                    self.primary_slot_config,
                );
                self.primary_slot_config
            }
            0xA9 => {
                tracing::trace!(
                    "[PPI] [RD] [KeyboardPrt] [{:02X}] = {:02X}",
                    port,
                    self.register_b
                );
                self.read_keyboard()
            }
            0xAA => {
                tracing::trace!(
                    "[PPI] [RD] [Register C ] [{:02X}] = {:02X}",
                    port,
                    self.register_c
                );
                self.register_c
            }
            0xAB => {
                // ignored output port
                tracing::trace!("[PPI] [RD] [AB IgnoredP] [{:02X}] = {:02X}", port, 0xFF);
                0xFF
            }
            _ => 0xFF,
        }
    }

    pub fn read_keyboard(&mut self) -> u8 {
        self.keyboard.get_row(self.keyboard_row_selected)
    }

    pub fn write(&mut self, port: u8, value: u8) {
        match port {
            0xA8 => {
                // info!("[PPI] [WR] [PrimarySlot] [{:02X}] = {:02X}", port, value);
                // set primary slot config
                self.primary_slot_config = value;
            }
            0xA9 => {
                // this port is ignored as output -- input only
                // info!("[PPI] [WR] [IgnoredPort] [{:02X}] = {:02X}", port, value);
            }
            0xAA => {
                let modf = self.register_c ^ value;
                if modf == 0x00 {
                    return;
                }
                self.register_c = value;

                if modf & 0x0f != 0 {
                    self.update_keyboard_config();
                }

                // var bit = (val & 0x0e) >>> 1;
                // if ((val & 0x01) === 0) registerC &= ~(1 << bit);
                // else registerC |= 1 << bit;

                // if (bit <= 3) updateKeyboardConfig();
                // else if (bit === 5 || bit === 7) updatePulseSignal();
                // else if (bit === 6) updateCapsLed();
            }
            0xAB => {
                let bit = (value & 0x0e) >> 1;
                tracing::trace!(
                    "[PPI] [WR] [Port AB    ] [{:02X}] = {:02X} bit {}",
                    port,
                    value,
                    bit
                );
                //       info!("[PPI] [WR] [PpiControl2] [{:02X}] = {:02X}", port, value);
                if (value & 0x01) == 0 {
                    self.register_c &= !(1 << bit);
                } else {
                    self.register_c |= 1 << bit;
                }

                match bit {
                    0..=3 => self.update_keyboard_config(),
                    // 5 | 7 => self.update_pulse_signal(),
                    6 => self.update_caps_led(),
                    _ => (),
                }
            }
            _ => (),
        }
    }

    fn update_keyboard_config(&mut self) {
        self.keyboard_row_selected = self.register_c & 0x0f;
        tracing::trace!("[PPI] Keyboard row: {}", self.keyboard_row_selected);
    }

    fn update_caps_led(&mut self) {
        tracing::trace!(
            "[PPI] CapsLed: {}",
            if (self.register_c & 0x40) == 0 {
                "ON"
            } else {
                "OFF"
            }
        );
    }
}

impl Default for Ppi {
    fn default() -> Self {
        Ppi {
            keyboard: Keyboard::new(),
            primary_slot_config: 0,
            register_b: 0,
            register_c: 0x50, // Everything OFF. Motor and CapsLed = 1 means OFF
            control: 0,

            keyboard_row_selected: 0,
        }
    }
}
