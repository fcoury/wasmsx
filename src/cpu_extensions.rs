// CPU Extension mechanism for handling special ED E0-FF instructions
// Used to implement disk BIOS calls and other extended operations

use z80::Z80;

#[derive(Debug, Clone)]
pub struct CpuExtensionState {
    pub ext_num: u8,
    pub ext_pc: u16,
    pub pc: u16,
    pub sp: u16,
    pub a: u8,
    pub f: u8,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub ix: u16,
    pub iy: u16,
}

impl CpuExtensionState {
    pub fn from_z80<T: z80::Z80_io>(z80: &Z80<T>, ext_num: u8) -> Self {
        Self {
            ext_num,
            ext_pc: z80.pc.wrapping_sub(2), // PC before ED XX instruction
            pc: z80.pc,
            sp: z80.sp,
            a: z80.get_a(),
            f: z80.get_f(),
            bc: z80.get_bc(),
            de: z80.get_de(),
            hl: z80.get_hl(),
            ix: z80.ix,
            iy: z80.iy,
        }
    }
    
    pub fn apply_to_z80<T: z80::Z80_io>(&self, z80: &mut Z80<T>) {
        z80.pc = self.pc;
        z80.sp = self.sp;
        z80.set_a(self.a);
        z80.set_f(self.f);
        z80.set_bc(self.bc);
        z80.set_de(self.de);
        z80.set_hl(self.hl);
        z80.ix = self.ix;
        z80.iy = self.iy;
    }

    pub fn b(&self) -> u8 {
        (self.bc >> 8) as u8
    }

    pub fn c(&self) -> u8 {
        self.bc as u8
    }

    pub fn d(&self) -> u8 {
        (self.de >> 8) as u8
    }

    pub fn e(&self) -> u8 {
        self.de as u8
    }

    pub fn h(&self) -> u8 {
        (self.hl >> 8) as u8
    }

    pub fn l(&self) -> u8 {
        self.hl as u8
    }

    pub fn set_b(&mut self, value: u8) {
        self.bc = (self.bc & 0x00FF) | ((value as u16) << 8);
    }

    pub fn set_c(&mut self, value: u8) {
        self.bc = (self.bc & 0xFF00) | (value as u16);
    }

    pub fn carry_flag(&self) -> bool {
        (self.f & 0x01) != 0
    }

    pub fn set_carry_flag(&mut self, carry: bool) {
        if carry {
            self.f |= 0x01;
        } else {
            self.f &= !0x01;
        }
    }
}

pub trait CpuExtensionHandler {
    fn extension_begin(&mut self, state: &mut CpuExtensionState) -> bool;
    fn extension_finish(&mut self, state: &mut CpuExtensionState) -> bool;
}

pub trait MemoryAccess {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
    fn read_block(&self, address: u16, buffer: &mut [u8]);
    fn write_block(&mut self, address: u16, buffer: &[u8]);
}