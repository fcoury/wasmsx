// use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub struct InternalState {
    // 8-bit registers
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,

    // 16-bit registers
    pub sp: u16,
    pub pc: u16,
    pub hl: u16,
    pub bc: u16,

    // contents
    pub hl_contents: u8,
    pub opcode: u8,
}

// impl fmt::Display for InternalState {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let fv = self.f;
//         let flags = format!(
//             "S: {} Z: {} H: {} P/V: {} N: {} C: {}",
//             if fv & (Flag::S as u8) != 0 { "1" } else { "0" },
//             if fv & (Flag::Z as u8) != 0 { "1" } else { "0" },
//             if fv & (Flag::H as u8) != 0 { "1" } else { "0" },
//             if fv & (Flag::P as u8) != 0 { "1" } else { "0" },
//             if fv & (Flag::N as u8) != 0 { "1" } else { "0" },
//             if fv & (Flag::C as u8) != 0 { "1" } else { "0" },
//         );
//         // FIXME apparently the F3 and F5 registers are accounted for on the openMSX, we're skipping it for now
//         // write!(
//         //     f,
//         //     "#{:04X} - A: #{:02X} B: #{:02X} C: #{:02X} D: #{:02X} E: #{:02X} F: #{:02X} H: #{:02X} L: #{:02X} - {}",
//         //     self.pc, self.a, self.b, self.c, self.d, self.e, self.f, self.h, self.l, flags
//         // )
//         write!(
//             f,
//             "#{:04X} #{:02X} - A: #{:02X} B: #{:02X} C: #{:02X} D: #{:02X} E: #{:02X} H: #{:02X} L: #{:02X} - HL: #{:04X}(#{:02X}) SP: #{:04X} BC: #{:04X} - {}",
//             self.pc, self.opcode, self.a, self.b, self.c, self.d, self.e, self.h, self.l, self.hl, self.hl_contents, self.sp, self.bc, flags
//         )
//     }
// }

pub trait ReportState {
    fn report_state(&mut self) -> anyhow::Result<InternalState>;
}
