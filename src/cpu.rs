use std::{cell::RefCell, fmt, rc::Weak};

use z80::Z80;

use crate::{bus::Bus, machine::Io};

#[derive(Default)]
pub struct Cpu {
    cpu: Option<Z80<Io>>,
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_bus(&mut self, bus: Weak<RefCell<Bus>>) {
        self.cpu = Some(Z80::new(Io::new(bus)));
    }

    pub fn step(&mut self) {
        if let Some(mut cpu) = self.cpu.take() {
            cpu.step();
            self.cpu = Some(cpu);
        } else {
            panic!("CPU instance not set");
        }
    }

    pub fn pc(&self) -> u16 {
        self.cpu.as_ref().expect("CPU instance").pc
    }

    pub fn halted(&self) -> bool {
        self.cpu.as_ref().expect("CPU instance").halted
    }

    pub fn set_irq(&mut self) {
        if let Some(mut cpu) = self.cpu.take() {
            cpu.assert_irq(0);
            self.cpu = Some(cpu);
        } else {
            panic!("CPU instance not set");
        }
    }

    pub fn clear_irq(&mut self) {
        if let Some(mut cpu) = self.cpu.take() {
            cpu.clr_irq();
            self.cpu = Some(cpu);
        } else {
            panic!("CPU instance not set");
        }
    }
}

impl fmt::Debug for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(cpu) = self.cpu.as_ref() {
            f.debug_struct("Cpu")
                .field("pc", &cpu.pc)
                .field("sp", &cpu.sp)
                .field("ix", &cpu.ix)
                .field("iy", &cpu.iy)
                .field("i", &cpu.i)
                .field("r", &cpu.r)
                .field("iff1", &cpu.iff1)
                .field("iff2", &cpu.iff2)
                .field("halted", &cpu.halted)
                .finish()
        } else {
            f.debug_struct("Cpu").finish()
        }
    }
}
