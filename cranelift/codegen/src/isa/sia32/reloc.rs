//! SIA32 external relocation contract.
//!
//! M8 deliberately keeps object-format policy separate from CLIF lowering. The
//! machine backend emits absolute 32-bit words and leaves final symbol placement
//! to the object writer/linker. Local SIA branches and LDPC literal references
//! are `LabelUse` fixups and must be resolved before this boundary.

use super::inst::Inst;
use crate::binemit::{CodeOffset, Reloc};
use crate::ir::ExternalName;
use crate::machinst::MachBuffer;

/// The Cranelift relocation used by the SIA32 object contract for an absolute
/// 32-bit address word.
pub(crate) const ABS32: Reloc = Reloc::Abs4;

/// Emit one aligned, zero-filled SIA32 address word carrying an absolute
/// 32-bit relocation.
///
/// The caller must place this word in a non-executed literal/data region. SIA
/// code that loads an address is responsible for branching around inline words,
/// exactly as the existing direct-call sequence does.
pub(crate) fn emit_abs32_word(
    code: &mut MachBuffer<Inst>,
    target: &ExternalName,
    addend: i32,
) -> CodeOffset {
    code.align_to(4);
    let offset = code.cur_offset();
    code.add_reloc(ABS32, target, i64::from(addend));
    code.put4(0);
    offset
}

/// Resolve the mathematical value of a SIA32 ABS32 relocation.
///
/// Object writers/linkers must reject `None`; truncating a value outside the
/// unsigned 32-bit address space is not part of the SIA32 contract.
pub(crate) fn checked_abs32_value(symbol_address: u64, addend: i32) -> Option<u32> {
    let value = i128::from(symbol_address) + i128::from(addend);
    u32::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::inst::EmitState;
    use crate::ir::ExternalName;
    use crate::isa::CallConv;
    use crate::machinst::{CallInfo, MachInstEmit};
    use crate::RelocTarget;
    use alloc::boxed::Box;

    #[test]
    fn abs32_word_records_exact_target_addend_and_alignment() {
        let target = ExternalName::testcase("sia_data");
        let mut code = MachBuffer::<Inst>::new();

        // Start on the other legal halfword alignment to prove that the
        // relocated word itself is forced to a four-byte boundary.
        code.put2(0);
        let offset = emit_abs32_word(&mut code, &target, -4);

        assert_eq!(offset, 4);
        assert_eq!(offset % 4, 0);
        assert_eq!(code.cur_offset(), 8);
        assert_eq!(&code.data[4..8], &[0, 0, 0, 0]);
        assert_eq!(code.relocs.len(), 1);

        let reloc = &code.relocs[0];
        assert_eq!(reloc.offset, 4);
        assert_eq!(reloc.kind, Reloc::Abs4);
        assert_eq!(reloc.target, RelocTarget::ExternalName(target));
        assert_eq!(reloc.addend, -4);
    }

    #[test]
    fn direct_external_call_uses_abs32_target_literal() {
        let target = ExternalName::testcase("sia_callee");
        let info = CallInfo::empty(target.clone(), CallConv::SystemV);
        let call = Inst::Call { info: Box::new(info) };
        let mut code = MachBuffer::<Inst>::new();
        let mut state = EmitState::default();

        call.emit(&mut code, &(), &mut state);

        assert_eq!(code.cur_offset(), 12);
        assert_eq!(code.relocs.len(), 1);
        let reloc = &code.relocs[0];
        assert_eq!(reloc.offset, 8);
        assert_eq!(reloc.offset % 4, 0);
        assert_eq!(reloc.kind, Reloc::Abs4);
        assert_eq!(reloc.target, RelocTarget::ExternalName(target));
        assert_eq!(reloc.addend, 0);
        assert_eq!(&code.data[8..12], &[0, 0, 0, 0]);
    }

    #[test]
    fn abs32_range_is_checked_without_host_width_truncation() {
        assert_eq!(checked_abs32_value(0, 0), Some(0));
        assert_eq!(checked_abs32_value(u64::from(u32::MAX), 0), Some(u32::MAX));
        assert_eq!(checked_abs32_value(0x1000, -4), Some(0x0ffc));

        assert_eq!(checked_abs32_value(0, -1), None);
        assert_eq!(checked_abs32_value(u64::from(u32::MAX) + 1, 0), None);
        assert_eq!(checked_abs32_value(u64::MAX, i32::MAX), None);
    }
}
