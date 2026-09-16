//! SIA32 machine-instruction vocabulary used by the MachInst backend.
//!
//! This first layer intentionally separates semantic machine operations from
//! final 16-bit encodings. Some semantic operations (for example destructive
//! two-operand SIA instructions whose destination differs from the CLIF lhs)
//! may expand to more than one architectural instruction during emission.

use crate::ir::types::{I32, I64};
use crate::ir::Type;
use crate::machinst::{ArgPair, MachLabel, Reg, RetPair, Writable};
use crate::{CodegenError, CodegenResult};
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use regalloc2::RegClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TwoOp {
    Sub,
    Addo,
    Subo,
    CmpEq,
    CmpLt,
    CmpLtu,
    Min,
    MinU,
    Max,
    MaxU,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    BSet,
    BClr,
    BInv,
    BExt,
    Rev8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnaryOp {
    Clz,
    Ctz,
    Cpop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoadOp {
    I8,
    U8,
    I16,
    U16,
    I32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreOp {
    I8,
    I16,
    I32,
}

#[derive(Clone, Debug)]
pub(crate) enum Inst {
    /// Bind incoming physical argument registers to SSA virtual registers.
    Args { args: Vec<ArgPair> },
    /// Constrain outgoing return values to physical ABI registers.
    Rets { rets: Vec<RetPair> },
    /// Keep a virtual register live without emitting machine code.
    DummyUse { reg: Reg },

    Nop,
    Trap { code: u8 },

    Mov { dst: Writable<Reg>, src: Reg },
    Add { dst: Writable<Reg>, lhs: Reg, rhs: Reg },
    Unary { op: UnaryOp, dst: Writable<Reg>, src: Reg },

    /// Semantic three-operand form for SIA's destructive two-operand family.
    /// Emission can omit the move when `dst == lhs` after allocation.
    TwoOp {
        op: TwoOp,
        dst: Writable<Reg>,
        lhs: Reg,
        rhs: Reg,
    },

    ShiftImm {
        op: TwoOp,
        dst: Writable<Reg>,
        src: Reg,
        amount: u8,
    },
    Li7 { dst: Writable<Reg>, imm: i8 },
    Addi7 { dst: Writable<Reg>, src: Reg, imm: i8 },
    /// Arbitrary 32-bit constant. Expanded through LI/ADDI or an LDPC island.
    LoadConst32 { dst: Writable<Reg>, value: u32 },

    Load {
        op: LoadOp,
        dst: Writable<Reg>,
        base: Reg,
    },
    Store {
        op: StoreOp,
        src: Reg,
        base: Reg,
    },
    IndexedLoad {
        dst: Writable<Reg>,
        base: Reg,
        index: Reg,
    },
    IndexedStore {
        src: Reg,
        base: Reg,
        index: Reg,
    },

    Jump { target: MachLabel },
    /// Two-target conditional form used by VCode. SIA has BNZ but no BZ, so
    /// emission/relaxation chooses the short/fallthrough arrangement.
    BrNz {
        test: Reg,
        taken: MachLabel,
        not_taken: MachLabel,
    },
    Ret,

    /// Reserve room for the future direct-call implementation without fixing a
    /// relocation format yet. The target remains out-of-line to keep `Inst`
    /// compact when calls gain full `CallInfo`.
    CallPlaceholder { target: Box<str> },
}

const INT1_RCS: [RegClass; 1] = [RegClass::Int];
const INT1_TYS: [Type; 1] = [I32];
const INT2_RCS: [RegClass; 2] = [RegClass::Int, RegClass::Int];
const INT2_TYS: [Type; 2] = [I32, I32];

/// Register representation for a CLIF SSA value on SIA32.
///
/// Narrow integers live in a full I32 GPR. I64 is represented as two I32 GPRs
/// in little-endian order (low word first). Floating-point, vector, and I128
/// values are deliberately unsupported until their ABI/lowering is defined.
pub(crate) fn rc_for_type(ty: &Type) -> CodegenResult<(&'static [RegClass], &'static [Type])> {
    match *ty {
        t if t.is_int() && t.bits() <= 32 => Ok((&INT1_RCS, &INT1_TYS)),
        I64 => Ok((&INT2_RCS, &INT2_TYS)),
        _ => Err(CodegenError::Unsupported(format!(
            "SIA32 does not yet support SSA value type {ty}"
        ))),
    }
}

impl Inst {
    pub(crate) fn is_move(&self) -> Option<(Writable<Reg>, Reg)> {
        match *self {
            Self::Mov { dst, src } => Some((dst, src)),
            _ => None,
        }
    }

    pub(crate) fn is_args(&self) -> bool {
        matches!(self, Self::Args { .. })
    }

    pub(crate) fn is_memory_access(&self) -> bool {
        matches!(
            self,
            Self::Load { .. }
                | Self::Store { .. }
                | Self::IndexedLoad { .. }
                | Self::IndexedStore { .. }
        )
    }

    pub(crate) fn is_branch(&self) -> bool {
        matches!(self, Self::Jump { .. } | Self::BrNz { .. })
    }

    pub(crate) fn is_return(&self) -> bool {
        matches!(self, Self::Ret | Self::Rets { .. })
    }

    pub(crate) fn worst_case_size(&self) -> u32 {
        match self {
            // A destructive operation may need MOV + OP.
            Self::TwoOp { .. } | Self::ShiftImm { .. } | Self::Addi7 { .. } => 4,
            // The initial arbitrary-constant strategy reserves enough room for
            // a small synthesized sequence. Literal-island lowering can shrink
            // this later.
            Self::LoadConst32 { .. } => 10,
            // A two-target branch may need BNZ plus B before long-branch
            // relaxation is considered.
            Self::BrNz { .. } => 4,
            Self::Args { .. } | Self::Rets { .. } | Self::DummyUse { .. } => 0,
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{F32, I8, I16, I128};
    use crate::isa::sia32::regs::mach_reg;

    fn w(n: u8) -> Writable<Reg> {
        Writable::from_reg(mach_reg(n))
    }

    #[test]
    fn scalar_and_i64_register_representation_is_32_bit_native() {
        for ty in [I8, I16, I32] {
            let (rcs, tys) = rc_for_type(&ty).unwrap();
            assert_eq!(rcs, &[RegClass::Int]);
            assert_eq!(tys, &[I32]);
        }
        let (rcs, tys) = rc_for_type(&I64).unwrap();
        assert_eq!(rcs, &[RegClass::Int, RegClass::Int]);
        assert_eq!(tys, &[I32, I32]);
    }

    #[test]
    fn unsupported_value_classes_fail_explicitly() {
        assert!(rc_for_type(&F32).is_err());
        assert!(rc_for_type(&I128).is_err());
    }

    #[test]
    fn move_and_memory_classification_is_exact() {
        let mov = Inst::Mov {
            dst: w(1),
            src: mach_reg(2),
        };
        assert_eq!(mov.is_move(), Some((w(1), mach_reg(2))));
        assert!(!mov.is_memory_access());

        let load = Inst::Load {
            op: LoadOp::I32,
            dst: w(1),
            base: mach_reg(2),
        };
        assert!(load.is_memory_access());
    }

    #[test]
    fn pseudos_report_conservative_sizes() {
        let op = Inst::TwoOp {
            op: TwoOp::Sub,
            dst: w(1),
            lhs: mach_reg(2),
            rhs: mach_reg(3),
        };
        assert_eq!(op.worst_case_size(), 4);

        let constant = Inst::LoadConst32 {
            dst: w(1),
            value: 0xdead_beef,
        };
        assert_eq!(constant.worst_case_size(), 10);
    }
}
