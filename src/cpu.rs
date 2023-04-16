use std::{cell::RefCell, fmt, rc::Rc};

use z80::Z80;

use crate::{bus::BusMessage, io::Io};

pub struct Cpu {
    cpu: Z80<Io>,
}

impl Cpu {
    pub fn new(queue: Rc<RefCell<Vec<BusMessage>>>) -> Self {
        let cpu = Z80::new(Io::new(queue));
        Self { cpu }
    }

    pub fn step(&mut self) {
        println!("Cpu step");
        self.cpu.step();
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn halted(&self) -> bool {
        self.cpu.halted
    }

    pub fn set_irq(&mut self) {
        self.cpu.assert_irq(0);
    }

    pub fn clear_irq(&mut self) {
        self.cpu.clr_irq();
    }
}

impl fmt::Debug for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cpu")
            .field("pc", &self.cpu.pc)
            .field("sp", &self.cpu.sp)
            .field("ix", &self.cpu.ix)
            .field("iy", &self.cpu.iy)
            .field("i", &self.cpu.i)
            .field("r", &self.cpu.r)
            .field("iff1", &self.cpu.iff1)
            .field("iff2", &self.cpu.iff2)
            .field("halted", &self.cpu.halted)
            .finish()
    }
}
