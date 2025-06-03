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
    slot_config_log_counter: u32,
}

impl Ppi {
    pub fn new() -> Self {
        Ppi::default()
    }

    pub fn reset(&mut self) {
        self.register_c = 0x70; // Everything OFF. Motor bits 4,5 and CapsLed bit 6 = 1 means OFF
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

    pub fn register_c(&self) -> u8 {
        self.register_c
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
                // set primary slot config
                let old_config = self.primary_slot_config;
                self.primary_slot_config = value;

                if (old_config & 0b00001100) != (value & 0b00001100) {
                    // Log if Page 1 slot changes
                    // tracing::info!(
                    //     "[PPI] Primary slot config (Port A8 Write): Old {:02X} -> New {:02X} (Page0:{} Page1:{} Page2:{} Page3:{})",
                    //     old_config, value,
                    //     value & 0x03, (value >> 2) & 0x03,
                    //     (value >> 4) & 0x03, (value >> 6) & 0x03
                    // );
                }

                // if old_config != value {
                //     let page1_slot = (value >> 2) & 0x03;
                //
                //     // Rate limit the logging - assuming ~3.5MHz Z80 and this being called frequently
                //     // Log approximately once every 30 seconds (very rough approximation)
                //     self.slot_config_log_counter += 1;
                //     if self.slot_config_log_counter >= 10000 {
                //         self.slot_config_log_counter = 0;
                //         tracing::info!(
                //             "[PPI] Primary slot config: {:02X} -> {:02X} (Page3:{} Page2:{} Page1:{} Page0:{})",
                //             old_config,
                //             value,
                //             (value >> 6) & 0x03,
                //             (value >> 4) & 0x03,
                //             page1_slot,
                //             value & 0x03
                //         );
                //     }
                //
                //     if page1_slot == 1 {
                //         tracing::trace!(
                //             "[PPI] Slot 1 (Disk ROM) now mapped to page 1 (0x4000-0x7FFF)"
                //         );
                //     }
                // }
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
                let old_register_c = self.register_c;
                self.register_c = value;

                // Log motor control bit changes
                if modf & 0x30 != 0 {
                    tracing::info!(
                        "[PPI] Port AA (PortC Write): {:02X}. RegC: {:02X} -> {:02X}. DriveA Motor: {}, DriveB Motor: {}",
                        value,
                        old_register_c,
                        self.register_c,
                        if (self.register_c & 0x10) == 0 { "ON" } else { "OFF" },
                        if (self.register_c & 0x20) == 0 { "ON" } else { "OFF" }
                    );
                }

                if modf & 0x0f != 0 {
                    self.update_keyboard_config();
                }

                if modf & 0x40 != 0 {
                    self.update_caps_led();
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
                let old_register_c = self.register_c;

                if (value & 0x01) == 0 {
                    self.register_c &= !(1 << bit);
                } else {
                    self.register_c |= 1 << bit;
                }

                // Log motor control bit changes (bits 4 and 5)
                if bit == 4 || bit == 5 {
                    let op = if (value & 0x01) == 0 { "CLEAR" } else { "SET" };
                    tracing::info!(
                        "[PPI] Port AB (Bit Set/Reset): Value {:02X} (bit {}, {}). RegC: {:02X} -> {:02X}. DriveA Motor: {}, DriveB Motor: {}",
                        value,
                        bit,
                        op,
                        old_register_c,
                        self.register_c,
                        if (self.register_c & 0x10) == 0 { "ON" } else { "OFF" },
                        if (self.register_c & 0x20) == 0 { "ON" } else { "OFF" }
                    );
                } else {
                    tracing::trace!(
                        "[PPI] [WR] [Port AB    ] [{:02X}] = {:02X} bit {}",
                        port,
                        value,
                        bit
                    );
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
            register_c: 0x70, // Everything OFF. Motor bits 4,5 and CapsLed bit 6 = 1 means OFF
            control: 0,

            keyboard_row_selected: 0,
            slot_config_log_counter: 0,
        }
    }
}
