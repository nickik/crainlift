//! M7 SIA32 ISA facilities that are deliberately independent of CLIF/ISLE lowering.

use super::encode;
use super::regs::Reg;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum M7Op {
    Bset, Bclr, Binv, Bext, Mul, Mulh, Mulhu, Mulhsu, Mulo, Div, Divu, Rem, Remu, Rev8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct M7Inst { pub op: M7Op, pub rd: Reg, pub rs: Reg }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum M7DecodeError { NotM7, ReservedFunction }

impl M7Inst {
    pub const fn new(op: M7Op, rd: Reg, rs: Reg) -> Self { Self { op, rd, rs } }
    pub const fn encode(self) -> u16 {
        match self.op {
            M7Op::Bset => encode::bset(self.rd, self.rs), M7Op::Bclr => encode::bclr(self.rd, self.rs),
            M7Op::Binv => encode::binv(self.rd, self.rs), M7Op::Bext => encode::bext(self.rd, self.rs),
            M7Op::Mul => encode::mul(self.rd, self.rs), M7Op::Mulh => encode::mulh(self.rd, self.rs),
            M7Op::Mulhu => encode::mulhu(self.rd, self.rs), M7Op::Mulhsu => encode::mulhsu(self.rd, self.rs),
            M7Op::Mulo => encode::mulo(self.rd, self.rs), M7Op::Div => encode::div(self.rd, self.rs),
            M7Op::Divu => encode::divu(self.rd, self.rs), M7Op::Rem => encode::rem(self.rd, self.rs),
            M7Op::Remu => encode::remu(self.rd, self.rs), M7Op::Rev8 => encode::rev8(self.rd, self.rs),
        }
    }
    pub fn decode(word: u16) -> Result<Self, M7DecodeError> {
        if word >> 12 != 0xC { return Err(M7DecodeError::NotM7); }
        let rd = Reg::new(((word >> 8) & 0xf) as u8).expect("four-bit register");
        let rs = Reg::new(((word >> 4) & 0xf) as u8).expect("four-bit register");
        let op = match word & 0xf {
            0x1 => M7Op::Bset, 0x2 => M7Op::Bclr, 0x3 => M7Op::Binv, 0x4 => M7Op::Bext,
            0x5 => M7Op::Mul, 0x6 => M7Op::Mulh, 0x7 => M7Op::Mulhu, 0x8 => M7Op::Mulhsu,
            0x9 => M7Op::Mulo, 0xA => M7Op::Div, 0xB => M7Op::Divu, 0xC => M7Op::Rem,
            0xD => M7Op::Remu, 0xE => M7Op::Rev8, _ => return Err(M7DecodeError::ReservedFunction),
        };
        Ok(Self { op, rd, rs })
    }
    pub fn disassemble(self) -> alloc::string::String { alloc::format!("{} {}, {}", self.op.mnemonic(), self.rd, self.rs) }
}

impl M7Op {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Bset => "bset", Self::Bclr => "bclr", Self::Binv => "binv", Self::Bext => "bext",
            Self::Mul => "mul", Self::Mulh => "mulh", Self::Mulhu => "mulhu", Self::Mulhsu => "mulhsu",
            Self::Mulo => "mulo", Self::Div => "div", Self::Divu => "divu", Self::Rem => "rem",
            Self::Remu => "remu", Self::Rev8 => "rev8",
        }
    }
}

/// Pure architectural reference semantics. The R2F instruction form is two-address: `rd <- f(rd, rs)`.
pub fn execute_reference(inst: M7Inst, rd: u32, rs: u32) -> u32 {
    match inst.op {
        M7Op::Bset => rd | (1u32 << (rs & 31)), M7Op::Bclr => rd & !(1u32 << (rs & 31)),
        M7Op::Binv => rd ^ (1u32 << (rs & 31)), M7Op::Bext => (rd >> (rs & 31)) & 1,
        M7Op::Mul => rd.wrapping_mul(rs),
        M7Op::Mulh => (((rd as i32 as i64) * (rs as i32 as i64)) >> 32) as u32,
        M7Op::Mulhu => (((rd as u64) * (rs as u64)) >> 32) as u32,
        M7Op::Mulhsu => (((rd as i32 as i64 as i128) * (rs as u64 as i128)) >> 32) as u32,
        M7Op::Mulo => ((rd as i32 as i64) * (rs as i32 as i64) != rd.wrapping_mul(rs) as i32 as i64) as u32,
        M7Op::Div => signed_div(rd, rs), M7Op::Divu => if rs == 0 { u32::MAX } else { rd / rs },
        M7Op::Rem => signed_rem(rd, rs), M7Op::Remu => if rs == 0 { rd } else { rd % rs },
        M7Op::Rev8 => rd.swap_bytes(),
    }
}

fn signed_div(lhs: u32, rhs: u32) -> u32 {
    let lhs = lhs as i32; let rhs = rhs as i32;
    if rhs == 0 { u32::MAX } else if lhs == i32::MIN && rhs == -1 { i32::MIN as u32 } else { lhs.wrapping_div(rhs) as u32 }
}
fn signed_rem(lhs: u32, rhs: u32) -> u32 {
    let lhs = lhs as i32; let rhs = rhs as i32;
    if rhs == 0 { lhs as u32 } else if lhs == i32::MIN && rhs == -1 { 0 } else { lhs.wrapping_rem(rhs) as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(index: u8) -> Reg { Reg::new(index).unwrap() }
    #[test]
    fn all_m7_encodings_round_trip_and_match_frozen_words() {
        let cases = [(M7Op::Bset,0xC121),(M7Op::Bclr,0xC122),(M7Op::Binv,0xC123),(M7Op::Bext,0xC124),(M7Op::Mul,0xC125),(M7Op::Mulh,0xC126),(M7Op::Mulhu,0xC127),(M7Op::Mulhsu,0xC128),(M7Op::Mulo,0xC129),(M7Op::Div,0xC12A),(M7Op::Divu,0xC12B),(M7Op::Rem,0xC12C),(M7Op::Remu,0xC12D),(M7Op::Rev8,0xC12E)];
        for (op, word) in cases { let inst=M7Inst::new(op,r(1),r(2)); assert_eq!(inst.encode(),word); assert_eq!(M7Inst::decode(word),Ok(inst)); }
    }
    #[test]
    fn decoder_rejects_wrong_opcode_and_reserved_functions() {
        assert_eq!(M7Inst::decode(0xB125),Err(M7DecodeError::NotM7)); assert_eq!(M7Inst::decode(0xC120),Err(M7DecodeError::ReservedFunction)); assert_eq!(M7Inst::decode(0xC12F),Err(M7DecodeError::ReservedFunction));
    }
    #[test]
    fn register_and_immediate_contract_is_explicit() {
        assert!(Reg::new(0).is_some()); assert!(Reg::new(15).is_some()); assert!(Reg::new(16).is_none()); assert!(r(12).is_backend_scratch()); assert!(r(13).is_sp()); assert!(r(14).is_lr()); assert_eq!(M7Inst::new(M7Op::Mul,r(15),r(0)).encode(),0xCF05);
    }
    #[test]
    fn disassembly_is_stable() {
        assert_eq!(M7Inst::new(M7Op::Mul,r(1),r(2)).disassemble(),"mul r1, r2"); assert_eq!(M7Inst::new(M7Op::Divu,r(13),r(14)).disassemble(),"divu sp, lr");
    }
    #[test]
    fn arithmetic_reference_model_covers_edges() {
        let i=|op| M7Inst::new(op,r(1),r(2));
        assert_eq!(execute_reference(i(M7Op::Mul),0xffff_ffff,2),0xffff_fffe); assert_eq!(execute_reference(i(M7Op::Mulh),0xffff_ffff,2),0xffff_ffff); assert_eq!(execute_reference(i(M7Op::Mulhu),0xffff_ffff,2),1); assert_eq!(execute_reference(i(M7Op::Mulhsu),0xffff_ffff,2),0xffff_ffff); assert_eq!(execute_reference(i(M7Op::Div),(-7i32) as u32,3),(-2i32) as u32); assert_eq!(execute_reference(i(M7Op::Rem),(-7i32) as u32,3),(-1i32) as u32); assert_eq!(execute_reference(i(M7Op::Divu),7,0),u32::MAX); assert_eq!(execute_reference(i(M7Op::Remu),7,0),7); assert_eq!(execute_reference(i(M7Op::Div),i32::MIN as u32,(-1i32) as u32),i32::MIN as u32); assert_eq!(execute_reference(i(M7Op::Rem),i32::MIN as u32,(-1i32) as u32),0); assert_eq!(execute_reference(i(M7Op::Rev8),0x1122_3344,0),0x4433_2211);
    }
}
