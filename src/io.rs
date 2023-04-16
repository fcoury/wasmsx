use std::{cell::RefCell, rc::Rc};

use z80::Z80_io;

use crate::bus::BusMessage;

pub struct Io {
    pub queue: Rc<RefCell<Vec<BusMessage>>>,
}

impl Io {
    pub fn new(queue: Rc<RefCell<Vec<BusMessage>>>) -> Self {
        Io { queue }
    }
}

impl Z80_io for Io {
    fn read_byte(&self, address: u16) -> u8 {
        let (tx, rx) = std::sync::mpsc::channel();

        self.queue
            .borrow_mut()
            .push(BusMessage::ReadByte(address, tx));

        rx.recv().unwrap()
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        todo!()
        // if address == 0x1452 || address == 0x0d12 || address == 0x0c3c {
        //     tracing::info!("[KEYBOARD] Writing to {:04X}", address);
        // }
        // self.bus.borrow_mut().write_byte(address, value)
    }

    fn port_in(&self, port: u16) -> u8 {
        todo!()
        // self.bus.borrow_mut().input(port as u8)
    }

    fn port_out(&mut self, port: u16, value: u8) {
        todo!()
        // self.bus.borrow_mut().output(port as u8, value)
    }
}
