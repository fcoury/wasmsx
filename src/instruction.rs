use std::fmt;

use tracing::error;
use z80::{Z80_io, Z80};

use crate::io::Io;

pub struct Instruction<'a> {
    pub opcode: u8,
    pub cpu: &'a Z80<Io>,
    pub pc: u16,
}

impl<'a> Instruction<'a> {
    pub fn parse(cpu: &'a Z80<Io>) -> Self {
        Self::parse_at(cpu, cpu.pc)
    }

    pub fn parse_at(cpu: &'a Z80<Io>, pc: u16) -> Self {
        let opcode = cpu.io.read_byte(pc);
        Instruction { opcode, cpu, pc }
    }

    pub fn name(&self) -> String {
        let name = self.as_def().0;
        if name.contains('$') {
            // finds $1, $2, $3, etc and replaces with pc + n
            let mut name = name.to_string();
            let mut i = 1;
            while name.contains(&format!("${}", i)) {
                let pc = self.pc.wrapping_add(i);
                let arg = self.cpu.io.read_byte(pc);
                name = name.replace(&format!("${}", i), &format!("{:02X}", arg));
                i += 1;
            }
            return name;
        }
        name.to_string()
    }

    pub fn len(&self) -> u8 {
        self.as_def().1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[allow(unused)]
    pub fn as_hex_vector(&self) -> Vec<String> {
        let (_, length) = self.as_def();
        let mut res = Vec::new();
        res.push(format!("{:02X}", self.opcode));
        for i in 1..length {
            let pc = self.pc.wrapping_add(i as u16);
            let arg = self.cpu.io.read_byte(pc);
            res.push(format!("{:02X}", arg));
        }
        res
    }

    pub fn opcode_with_args(&self) -> String {
        let (_, length) = self.as_def();
        let mut args = String::new();
        for i in 1..length {
            let pc = self.pc.wrapping_add(i as u16);
            let arg = self.cpu.io.read_byte(pc);
            args.push_str(&format!("{:02X} ", arg));
        }

        format!("{:02X} {}", self.opcode, args)
    }

    pub fn as_def(&self) -> (&str, u8) {
        match self.opcode {
            0x00 => ("NOP", 1),
            0xCF => ("RST 08H", 1),
            0xC7 => ("RST 00H", 1),
            0xD7 => ("RST 10H", 1),
            0xDF => ("RST 18H", 1),
            0xE7 => ("RST 20H", 1),
            0xEF => ("RST 28H", 1),
            0xFF => ("RST 38H", 1),
            0xF7 => ("RST 30H", 1),
            0x3E => ("LD A, #$1", 2),
            0x06 => ("LD B, #$1", 2),
            0x0E => ("LD C, #$1", 2),
            0x16 => ("LD D, #$1", 2),
            0x64 => ("LD H, H", 1),
            0x46 => ("LD B, (HL)", 1),
            0x4E => ("LD C, (HL)", 1),
            0x56 => ("LD D, (HL)", 1),
            0x66 => ("LD H, (HL)", 1),
            0x5E => ("LD E, (HL)", 1),
            0x1E => ("LD E, #$1", 2),
            0x26 => ("LD H, #$1", 2),
            0x2E => ("LD L, #$1", 2),
            0x78 => ("LD A, B", 1),
            0x79 => ("LD A, C", 1),
            0x7A => ("LD A, D", 1),
            0x7B => ("LD A, E", 1),
            0x7C => ("LD A, H", 1),
            0x7D => ("LD A, L", 1),
            0x47 => ("LD B, A", 1),
            0x40 => ("LD B, B", 1),
            0x41 => ("LD B, C", 1),
            0x42 => ("LD B, D", 1),
            0x43 => ("LD B, E", 1),
            0x44 => ("LD B, H", 1),
            0x45 => ("LD B, L", 1),
            0x4F => ("LD C, A", 1),
            0x48 => ("LD C, B", 1),
            0x49 => ("LD C, C", 1),
            0x4A => ("LD C, D", 1),
            0x4B => ("LD C, E", 1),
            0x4C => ("LD C, H", 1),
            0x4D => ("LD C, L", 1),
            0x57 => ("LD D, A", 1),
            0x50 => ("LD D, B", 1),
            0x51 => ("LD D, C", 1),
            0x52 => ("LD D, D", 1),
            0x53 => ("LD D, E", 1),
            0x54 => ("LD D, H", 1),
            0x55 => ("LD D, L", 1),
            0x5F => ("LD E, A", 1),
            0x58 => ("LD E, B", 1),
            0x59 => ("LD E, C", 1),
            0x5A => ("LD E, D", 1),
            0x5C => ("LD E, H", 1),
            0x5D => ("LD E, L", 1),
            0x67 => ("LD H, A", 1),
            0x60 => ("LD H, B", 1),
            0x61 => ("LD H, C", 1),
            0x62 => ("LD H, D", 1),
            0x63 => ("LD H, E", 1),
            0x65 => ("LD H, L", 1),
            0x6F => ("LD L, A", 1),
            0x68 => ("LD L, B", 1),
            0x69 => ("LD L, C", 1),
            0x6A => ("LD L, D", 1),
            0x6B => ("LD L, E", 1),
            0x6C => ("LD L, H", 1),
            0x77 => ("LD (HL), A", 1),
            0x70 => ("LD (HL), B", 1),
            0x71 => ("LD (HL), C", 1),
            0x72 => ("LD (HL), D", 1),
            0x73 => ("LD (HL), E", 1),
            0x74 => ("LD (HL), H", 1),
            0x75 => ("LD (HL), L", 1),
            0x36 => ("LD (HL), #$1", 2),
            0x21 => ("LD HL, $2$1", 3),
            0x2A => ("LD HL, ($2$1)", 3),
            0xF9 => ("LD SP, HL", 1),
            0x31 => ("LD SP, #$2$1", 3),
            0x0A => ("LD A, (BC)", 1),
            0x1A => ("LD A, (DE)", 1),
            0x3A => ("LD A, (#$2$1)", 3),
            0x7E => ("LD A, (HL)", 1),
            0x01 => ("LD BC, #$2$1", 3),
            0x11 => ("LD DE, #$2$1", 3),
            0x12 => ("LD (DE), A", 1),
            0x02 => ("LD (BC), A", 1),
            0x32 => ("LD (#$2$1), A", 3),
            0x22 => ("LD (#$2$1), HL", 3),
            0x10 => ("DJNZ #$1", 2),
            0x3C => ("INC A", 1),
            0x04 => ("INC B", 1),
            0x0C => ("INC C", 1),
            0x14 => ("INC D", 1),
            0x1C => ("INC E", 1),
            0x03 => ("INC BC", 1),
            0x13 => ("INC DE", 1),
            0x23 => ("INC HL", 1),
            0x33 => ("INC SP", 1),
            0x24 => ("INC H", 1),
            0x2C => ("INC L", 1),
            0x34 => ("INC (HL)", 1),
            0x3D => ("DEC A", 1),
            0x05 => ("DEC B", 1),
            0x0D => ("DEC C", 1),
            0x15 => ("DEC D", 1),
            0x1D => ("DEC E", 1),
            0x25 => ("DEC H", 1),
            0x2D => ("DEC L", 1),
            0x2B => ("DEC HL", 1),
            0x0B => ("DEC BC", 1),
            0x1B => ("DEC DE", 1),
            0x3B => ("DEC SP", 1),
            0x35 => ("DEC (HL)", 1),
            0x87 => ("ADD A, A", 1),
            0x80 => ("ADD A, B", 1),
            0x81 => ("ADD A, C", 1),
            0x82 => ("ADD A, D", 1),
            0x83 => ("ADD A, E", 1),
            0x84 => ("ADD A, H", 1),
            0x85 => ("ADD A, L", 1),
            0x86 => ("ADD A, (HL)", 1),
            0xC6 => ("ADD A, #$1", 2),
            0x09 => ("ADD HL, BC", 1),
            0x19 => ("ADD HL, DE", 1),
            0x29 => ("ADD HL, HL", 1),
            0x39 => ("ADD HL, SP", 1),
            0x8F => ("ADC A, A", 1),
            0x88 => ("ADC A, B", 1),
            0x89 => ("ADC A, C", 1),
            0x8A => ("ADC A, D", 1),
            0x8B => ("ADC A, E", 1),
            0x8C => ("ADC A, H", 1),
            0x8D => ("ADC A, L", 1),
            0x8E => ("ADC A, (HL)", 1),
            0xCE => ("ADC A, #$1", 2),
            0x97 => ("SUB A", 1),
            0x90 => ("SUB B", 1),
            0x91 => ("SUB C", 1),
            0x92 => ("SUB D", 1),
            0x93 => ("SUB E", 1),
            0x94 => ("SUB H", 1),
            0x95 => ("SUB L", 1),
            0x96 => ("SUB (HL)", 1),
            0xD6 => ("SUB #$1", 2),
            0x9F => ("SBC A, A", 1),
            0x98 => ("SBC A, B", 1),
            0x99 => ("SBC A, C", 1),
            0x9A => ("SBC A, D", 1),
            0x9B => ("SBC A, E", 1),
            0x9C => ("SBC A, H", 1),
            0x9D => ("SBC A, L", 1),
            0x9E => ("SBC A, (HL)", 1),
            0xDE => ("SBC A, #$1", 2),
            0xA7 => ("AND A", 1),
            0xA0 => ("AND B", 1),
            0xA1 => ("AND C", 1),
            0xA2 => ("AND D", 1),
            0xA3 => ("AND E", 1),
            0xA4 => ("AND H", 1),
            0xA5 => ("AND L", 1),
            0xA6 => ("AND (HL)", 1),
            0xE6 => ("AND #$1", 2),
            0xB7 => ("OR A", 1),
            0x07 => ("RLCA", 1),
            0x17 => ("RCA", 1),
            0xB0 => ("OR B", 1),
            0xB1 => ("OR C", 1),
            0xB2 => ("OR D", 1),
            0xB3 => ("OR E", 1),
            0xB4 => ("OR H", 1),
            0xB5 => ("OR L", 1),
            0xB6 => ("OR (HL)", 1),
            0xF6 => ("OR #$1", 2),
            0xAF => ("XOR A", 1),
            0xA8 => ("XOR B", 1),
            0xA9 => ("XOR C", 1),
            0xAA => ("XOR D", 1),
            0xAB => ("XOR E", 1),
            0xAC => ("XOR H", 1),
            0xAD => ("XOR L", 1),
            0xAE => ("XOR (HL)", 1),
            0xEE => ("XOR #$1", 2),
            0x18 => ("JR #$1", 2),
            0x76 => ("HALT", 1),
            0x2F => ("CPL", 1),
            0xBF => ("CP A", 1),
            0xB8 => ("CP B", 1),
            0xB9 => ("CP C", 1),
            0xBA => ("CP D", 1),
            0xBB => ("CP E", 1),
            0xBC => ("CP H", 1),
            0xBD => ("CP L", 1),
            0xFE => ("CP #$1", 2),
            0xBE => ("CP (HL)", 1),
            0xDD => {
                let opcode = self.cpu.io.read_byte(self.pc.wrapping_add(1));
                match opcode {
                    0xBE => ("CP (IX+d)", 4),
                    0x21 => ("LD IX, nn", 4),
                    0xE5 => ("PUSH IX", 2),
                    0xE1 => ("POP IX", 2),
                    _ => {
                        error!("Unknown opcode (CP (IX+d)) 0xDD 0x{:02X}", opcode);
                        ("Unknown", 1)
                    }
                }
            }
            0xFD => {
                let opcode = self.cpu.io.read_byte(self.pc.wrapping_add(1));
                match opcode {
                    0xBE => ("CP (IY+d)", 4),
                    0x22 => ("LD ($2$1), IY", 4),
                    0x2A => ("LD IY, ($2$1)", 4),
                    0x2D => ("DEC IYL", 2),
                    0xE5 => ("PUSH IY", 2),
                    0xE1 => ("POP IY", 2),
                    0xAF => ("XOR A", 2),
                    _ => {
                        error!("Unknown opcode (CP (IY+d)) 0xFD 0x{:02X}", opcode);
                        ("Unknown", 1)
                    }
                }
            }
            0x3F => ("CCF", 1),
            0x37 => ("SCF", 1),
            0xEB => ("EX DE, HL", 1),
            0xE3 => ("EX (SP), HL", 1),
            0x08 => ("EX AF, AF'", 1),
            0xD9 => ("EXX", 1),
            0xCC => ("CALL Z, #$2$1", 3),
            0xC4 => ("CALL NZ, #$2$1", 3),
            0xDC => ("CALL C, #$2$1", 3),
            0xD4 => ("CALL NC, #$2$1", 3),
            0xE4 => ("CALL PO, #$2$1", 3),
            0xFC => ("CALL M, #$2$1", 3),
            0xCD => ("CALL #$2$1", 3),
            0xC9 => ("RET", 1),
            0xC8 => ("RET Z", 1),
            0xD8 => ("RET C", 1),
            0xC0 => ("RET NZ", 1),
            0xD0 => ("RET NC", 1),
            0xF8 => ("RET M", 1),
            0xE0 => ("RET PO", 1),
            0xE8 => ("RET PE", 1),
            0xF0 => ("RET P", 1),
            0xC5 => ("PUSH BC", 1),
            0xD5 => ("PUSH DE", 1),
            0xE5 => ("PUSH HL", 1),
            0xF5 => ("PUSH AF", 1),
            0xC1 => ("POP BC", 1),
            0xD1 => ("POP DE", 1),
            0xE1 => ("POP HL", 1),
            0xF1 => ("POP AF", 1),
            0xF2 => ("JP P, #$2$1", 3),
            0xEA => ("JP PE, #$2$1", 3),
            0xE2 => ("JP PO, #$2$1", 3),
            0xC2 => ("JP NZ, #$2$1", 3),
            0xCA => ("JP Z, #$2$1", 3),
            0xD2 => ("JP NC, #$2$1", 3),
            0xDA => ("JP C, #$2$1", 3),
            0xFA => ("JP M, #$2$1", 3),
            0xC3 => ("JP #$2$1", 3),
            0x20 => ("JR NZ, #$1", 2),
            0x28 => ("JR Z, #$1", 2),
            0x30 => ("JR NC, #$1", 2),
            0x38 => ("JR C, #$1", 2),
            0x0F => ("RRCA", 1),
            0x1F => ("RRA", 1),
            0xCB => {
                // Read extended opcode and execute it
                let extended_opcode = self.cpu.io.read_byte(self.pc.wrapping_add(1));
                match extended_opcode {
                    0x00..=0x1F => ("RLC r", 2),
                    0x28..=0x2F => ("RR r", 2),
                    0x20..=0x3F => ("SLA r", 2),
                    0x40..=0x7F => ("BIT b, r", 2),
                    0x80..=0xBF => ("RES b, r", 2),
                    0xC0..=0xFF => ("SET b, r", 2),
                }
            }

            // I/O
            0xDB => ("IN A, #$1", 2),
            0xD3 => ("OUT #$1, A", 2),

            // Extended opcodes
            0xED => {
                let extended_opcode = self.cpu.io.read_byte(self.pc.wrapping_add(1));
                match extended_opcode {
                    0xB0 => ("LDIR", 2),
                    0x42 => ("SBC HL, BC", 2),
                    0x52 => ("SBC HL, DE", 2),
                    0x56 => ("IM 1", 2),
                    0xA2 => ("INI", 2),
                    0xA3 => ("OUTI", 2),
                    0x51 => ("OUT (C), D", 2),
                    0x58 => ("OUT (C), E", 2),
                    0x53 => ("LD ($2$1), DE", 4),
                    0x5B => ("LD DE, ($2$1)", 4),
                    _ => {
                        error!("Unknown opcode (ED) 0xED 0x{:02X}", extended_opcode);
                        ("Unknown", 1)
                    }
                }
            }

            // Interrupts
            0xFB => ("EI", 1),
            0xF3 => ("DI", 1),
            _ => {
                error!("Unknown opcode 0x{:02X}", self.opcode);
                ("Unknown", 1)
            }
        }
    }
}

impl<'a> fmt::Display for Instruction<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (name, length) = self.as_def();
        let mut args = String::new();
        for i in 1..length {
            let arg = self.cpu.io.read_byte(self.pc + i as u16);
            args.push_str(&format!("{:02X} ", arg));
        }

        write!(
            f,
            "{:04X} {:02X} {:<10} {}",
            self.pc, self.opcode, args, name
        )
    }
}
