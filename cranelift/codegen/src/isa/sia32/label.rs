//! SIA32 local-label fixup kinds.
//!
//! Branch displacements are signed halfword counts relative to the following
//! 16-bit instruction. `LDPC.W` is different: its displacement is a signed word
//! count relative to `align_down(instruction_pc + 4, 4)`.

use super::encode;
use crate::binemit::{Addend, CodeOffset, Reloc};
use crate::machinst::MachInstLabelUse;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LabelUse {
    Cond7,
    Branch11,
    Literal8,
}

impl LabelUse {
    fn read_word(buffer: &[u8]) -> u16 { u16::from_le_bytes([buffer[0], buffer[1]]) }
    fn write_word(buffer: &mut [u8], word: u16) { buffer[..2].copy_from_slice(&word.to_le_bytes()); }

    fn branch_disp_halfwords(use_offset: CodeOffset, label_offset: CodeOffset) -> i64 {
        let delta = i64::from(label_offset) - (i64::from(use_offset) + 2);
        assert_eq!(delta & 1, 0, "SIA branch target must be 2-byte aligned");
        delta / 2
    }

    pub(crate) fn literal_disp_words(use_offset: CodeOffset, label_offset: CodeOffset) -> i64 {
        assert_eq!(label_offset & 3, 0, "SIA literal target must be 4-byte aligned");
        let base = (u64::from(use_offset) + 4) & !3;
        let delta = i64::from(label_offset) - base as i64;
        assert_eq!(delta & 3, 0, "SIA literal displacement must be word-scaled");
        delta / 4
    }
}

impl MachInstLabelUse for LabelUse {
    const ALIGN: CodeOffset = 2;

    fn max_pos_range(self) -> CodeOffset {
        match self {
            Self::Cond7 => 128,
            Self::Branch11 => 2048,
            Self::Literal8 => 508,
        }
    }

    fn max_neg_range(self) -> CodeOffset {
        match self {
            Self::Cond7 => 126,
            Self::Branch11 => 2046,
            Self::Literal8 => 508,
        }
    }

    fn patch_size(self) -> CodeOffset { let _ = self; 2 }

    fn patch(self, buffer: &mut [u8], use_offset: CodeOffset, label_offset: CodeOffset) {
        assert!(buffer.len() >= 2);
        let old = Self::read_word(buffer);
        let patched = match self {
            Self::Cond7 => {
                let disp = Self::branch_disp_halfwords(use_offset, label_offset);
                assert!((-64..=63).contains(&disp));
                (old & !0x007f) | ((disp as u16) & 0x007f)
            }
            Self::Branch11 => {
                let disp = Self::branch_disp_halfwords(use_offset, label_offset);
                assert!((-1024..=1023).contains(&disp));
                (old & !0x07ff) | ((disp as u16) & 0x07ff)
            }
            Self::Literal8 => {
                let disp = Self::literal_disp_words(use_offset, label_offset);
                assert!((-128..=127).contains(&disp));
                (old & !0x00ff) | ((disp as u16) & 0x00ff)
            }
        };
        Self::write_word(buffer, patched);
    }

    fn supports_veneer(self) -> bool { matches!(self, Self::Cond7 | Self::Branch11) }

    fn veneer_size(self) -> CodeOffset {
        match self {
            Self::Cond7 | Self::Branch11 => 2,
            Self::Literal8 => 0,
        }
    }

    fn worst_case_veneer_size() -> CodeOffset { 2 }

    fn generate_veneer(self, buffer: &mut [u8], veneer_offset: CodeOffset) -> (CodeOffset, Self) {
        assert!(matches!(self, Self::Cond7 | Self::Branch11));
        assert!(buffer.len() >= 2);
        buffer[..2].copy_from_slice(&encode::b(0).unwrap().to_le_bytes());
        // Cond7 is promoted to Branch11. Branch11 deliberately returns another
        // Branch11 use: MachBuffer can therefore chain 2 KiB hops for arbitrary
        // in-function reach without an architectural PC-address instruction.
        (veneer_offset, Self::Branch11)
    }

    fn from_reloc(_reloc: Reloc, _addend: Addend) -> Option<Self> { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isa::sia32::regs::Reg;

    fn r(n: u8) -> Reg { Reg::new(n).unwrap() }

    #[test]
    fn cond_fixup_is_relative_to_following_instruction() {
        let mut bytes = encode::bnz(r(3), 0).unwrap().to_le_bytes();
        LabelUse::Cond7.patch(&mut bytes, 100, 102 + 63 * 2);
        assert_eq!(u16::from_le_bytes(bytes), encode::bnz(r(3), 63).unwrap());
        let mut bytes = encode::dbnz(r(4), 0).unwrap().to_le_bytes();
        LabelUse::Cond7.patch(&mut bytes, 200, 202 - 64 * 2);
        assert_eq!(u16::from_le_bytes(bytes), encode::dbnz(r(4), -64).unwrap());
    }

    #[test]
    fn branch11_fixup_preserves_b_vs_bl_opcode() {
        let mut b = encode::b(0).unwrap().to_le_bytes();
        LabelUse::Branch11.patch(&mut b, 0x1000, 0x1002 + 1023 * 2);
        assert_eq!(u16::from_le_bytes(b), encode::b(1023).unwrap());
        let mut bl = encode::bl(0).unwrap().to_le_bytes();
        LabelUse::Branch11.patch(&mut bl, 0x2000, 0x2002 - 1024 * 2);
        assert_eq!(u16::from_le_bytes(bl), encode::bl(-1024).unwrap());
    }

    #[test]
    fn literal_fixup_uses_aligned_pc_plus_four_base() {
        let mut a = encode::ldpc_w(r(5), 0).unwrap().to_le_bytes();
        LabelUse::Literal8.patch(&mut a, 0x1000, 0x1004 + 127 * 4);
        assert_eq!(u16::from_le_bytes(a), encode::ldpc_w(r(5), 127).unwrap());
        let mut b = encode::ldpc_w(r(6), 0).unwrap().to_le_bytes();
        LabelUse::Literal8.patch(&mut b, 0x1002, 0x1004 - 128 * 4);
        assert_eq!(u16::from_le_bytes(b), encode::ldpc_w(r(6), -128).unwrap());
    }

    #[test]
    fn declared_branch_ranges_match_exact_encoding_boundaries() {
        assert_eq!(LabelUse::Cond7.max_pos_range(), 128);
        assert_eq!(LabelUse::Cond7.max_neg_range(), 126);
        assert_eq!(LabelUse::Branch11.max_pos_range(), 2048);
        assert_eq!(LabelUse::Branch11.max_neg_range(), 2046);
        assert_eq!(LabelUse::Cond7.patch_size(), 2);
        assert_eq!(LabelUse::Literal8.patch_size(), 2);
    }

    #[test]
    fn conditional_veneer_promotes_to_branch11() {
        let mut bytes = [0u8; 2];
        let (fixup, next) = LabelUse::Cond7.generate_veneer(&mut bytes, 0x400);
        assert_eq!(u16::from_le_bytes(bytes), encode::b(0).unwrap());
        assert_eq!(fixup, 0x400);
        assert_eq!(next, LabelUse::Branch11);
    }

    #[test]
    fn branch11_veneer_can_chain_without_relinking_lr() {
        let mut first = [0u8; 2];
        let (fixup, next) = LabelUse::Branch11.generate_veneer(&mut first, 0x800);
        assert_eq!(u16::from_le_bytes(first), encode::b(0).unwrap());
        assert_eq!(fixup, 0x800);
        assert_eq!(next, LabelUse::Branch11);
        assert_eq!(LabelUse::Branch11.veneer_size(), 2);
        assert!(LabelUse::Branch11.supports_veneer());
        assert!(!LabelUse::Literal8.supports_veneer());
    }
}
