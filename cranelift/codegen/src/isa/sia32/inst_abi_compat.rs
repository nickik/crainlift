//! Small bridge between the semantic SIA32 instruction model and Cranelift's
//! generic ABI implementation.

use super::inst::{self, Inst};
use crate::ir::Type;
use crate::machinst::{Reg, Writable};
use crate::CodegenResult;
use regalloc2::RegClass;

impl Inst {
    /// Return the SIA32 register representation used by the ABI layer without
    /// requiring trait-method lookup at every call site.
    pub(crate) fn rc_for_type(
        ty: &Type,
    ) -> CodegenResult<(&'static [RegClass], &'static [Type])> {
        inst::rc_for_type(ty)
    }

    /// Construct the canonical integer move used by the ABI layer.
    pub(crate) fn gen_move(to_reg: Writable<Reg>, from_reg: Reg, ty: Type) -> Self {
        debug_assert_eq!(ty.bits(), 32);
        Inst::Mov {
            dst: to_reg,
            src: from_reg,
        }
    }
}
