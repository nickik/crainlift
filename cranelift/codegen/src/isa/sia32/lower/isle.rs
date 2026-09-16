//! ISLE integration glue for the SIA32 integer lowering rules.

pub mod generated_code;

use self::generated_code::MInst;
use crate::ir::condcodes::{FloatCC, IntCC};
use crate::isa::sia32::Sia32Backend;
use crate::isa::sia32::inst::{Inst as MachineInst, TwoOp, UnaryOp};
use crate::machinst::isle::*;
use crate::machinst::{
    CallArgList, CallRetList, InstOutput, Lower, MachLabel, Reg, VCodeConstant,
    VCodeConstantData, VCodeInst,
};
use crate::{
    ir::{
        BlockCall, Inst, InstructionData, MemFlagsData, Opcode, TrapCode, Type, Value, ValueList,
        immediates::*, types::*,
    },
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use regalloc2::PReg;

// `increment_lowered_uses` in the pinned Cranelift revision is cfg-gated to the
// pre-existing native backends. The shared ISLE prelude only needs the semantic
// operation "mark this value used". This extension provides it for an SIA-only
// build without changing generic Cranelift yet; when another native backend is
// enabled the existing inherent method wins normal Rust method resolution.
trait SiaLowerUseExt {
    fn increment_lowered_uses(&mut self, val: Value);
}

impl<I: VCodeInst> SiaLowerUseExt for Lower<'_, I> {
    fn increment_lowered_uses(&mut self, val: Value) {
        let _ = self.put_value_in_regs(val);
    }
}

pub(crate) struct Sia32IsleContext<'a, 'b> {
    pub lower_ctx: &'a mut Lower<'b, MachineInst>,
    pub backend: &'a Sia32Backend,
}

impl<'a, 'b> Sia32IsleContext<'a, 'b> {
    fn new(lower_ctx: &'a mut Lower<'b, MachineInst>, backend: &'a Sia32Backend) -> Self {
        Self { lower_ctx, backend }
    }

    pub(crate) fn dfg(&self) -> &crate::ir::DataFlowGraph {
        &self.lower_ctx.f.dfg
    }
}

fn into_machine_inst(inst: MInst) -> MachineInst {
    match inst {
        MInst::LoadConst32 { dst, value } => MachineInst::LoadConst32 { dst, value },
        MInst::Add { dst, lhs, rhs } => MachineInst::Add { dst, lhs, rhs },
        MInst::Sub { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Sub,
            dst,
            lhs,
            rhs,
        },
        MInst::And { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::And,
            dst,
            lhs,
            rhs,
        },
        MInst::Or { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Or,
            dst,
            lhs,
            rhs,
        },
        MInst::Xor { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Xor,
            dst,
            lhs,
            rhs,
        },
        MInst::Shl { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Shl,
            dst,
            lhs,
            rhs,
        },
        MInst::Shr { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Shr,
            dst,
            lhs,
            rhs,
        },
        MInst::Sar { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Sar,
            dst,
            lhs,
            rhs,
        },
        MInst::Clz { dst, src } => MachineInst::Unary {
            op: UnaryOp::Clz,
            dst,
            src,
        },
        MInst::Ctz { dst, src } => MachineInst::Unary {
            op: UnaryOp::Ctz,
            dst,
            src,
        },
        MInst::Cpop { dst, src } => MachineInst::Unary {
            op: UnaryOp::Cpop,
            dst,
            src,
        },
        MInst::Extend {
            dst,
            src,
            signed,
            from_bits,
            to_bits,
        } => MachineInst::Extend {
            dst,
            src,
            signed,
            from_bits,
            to_bits,
        },
        MInst::LoadBaseOffset {
            dst,
            base,
            offset,
            ty,
        } => MachineInst::LoadBaseOffset {
            dst,
            base,
            offset,
            ty,
        },
        MInst::StoreBaseOffset {
            src,
            base,
            offset,
            ty,
        } => MachineInst::StoreBaseOffset {
            src,
            base,
            offset,
            ty,
        },
        MInst::Jump { target } => MachineInst::Jump { target },
        MInst::BrNz {
            test,
            taken,
            not_taken,
        } => MachineInst::BrNz {
            test,
            taken,
            not_taken,
        },
    }
}

impl generated_code::Context for Sia32IsleContext<'_, '_> {
    isle_lower_prelude_methods!();

    fn emit(&mut self, inst: MInst) -> Unit {
        self.lower_ctx.emit(into_machine_inst(inst));
    }

    fn sia_load_const(&mut self, dst: WritableReg, value: u64) -> MInst {
        let value = u32::try_from(value).expect("SIA32 word iconst must fit 32 bits");
        MInst::LoadConst32 { dst, value }
    }

    fn sia_add(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Add { dst, lhs, rhs }
    }

    fn sia_sub(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Sub { dst, lhs, rhs }
    }

    fn sia_and(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::And { dst, lhs, rhs }
    }

    fn sia_or(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Or { dst, lhs, rhs }
    }

    fn sia_xor(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Xor { dst, lhs, rhs }
    }

    fn sia_shl(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Shl { dst, lhs, rhs }
    }

    fn sia_shr(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Shr { dst, lhs, rhs }
    }

    fn sia_sar(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Sar { dst, lhs, rhs }
    }

    fn sia_clz(&mut self, dst: WritableReg, src: Reg) -> MInst {
        MInst::Clz { dst, src }
    }

    fn sia_ctz(&mut self, dst: WritableReg, src: Reg) -> MInst {
        MInst::Ctz { dst, src }
    }

    fn sia_popcnt(&mut self, dst: WritableReg, src: Reg) -> MInst {
        MInst::Cpop { dst, src }
    }

    fn sia_extend(
        &mut self,
        dst: WritableReg,
        src: Reg,
        signed: bool,
        from_bits: u8,
        to_bits: u8,
    ) -> MInst {
        MInst::Extend {
            dst,
            src,
            signed,
            from_bits,
            to_bits,
        }
    }

    fn sia_load_offset(&mut self, dst: WritableReg, base: Reg, offset: i32, ty: Type) -> MInst {
        MInst::LoadBaseOffset {
            dst,
            base,
            offset,
            ty,
        }
    }

    fn sia_store_offset(&mut self, src: Reg, base: Reg, offset: i32, ty: Type) -> MInst {
        MInst::StoreBaseOffset {
            src,
            base,
            offset,
            ty,
        }
    }

    fn sia_jump(&mut self, target: MachLabel) -> MInst {
        MInst::Jump { target }
    }

    fn sia_brnz(&mut self, test: Reg, taken: MachLabel, not_taken: MachLabel) -> MInst {
        MInst::BrNz {
            test,
            taken,
            not_taken,
        }
    }
}

pub(crate) fn lower(
    lower_ctx: &mut Lower<MachineInst>,
    backend: &Sia32Backend,
    inst: Inst,
) -> Option<InstOutput> {
    let mut isle_ctx = Sia32IsleContext::new(lower_ctx, backend);
    generated_code::constructor_lower(&mut isle_ctx, inst)
}

pub(crate) fn lower_branch(
    lower_ctx: &mut Lower<MachineInst>,
    backend: &Sia32Backend,
    branch: Inst,
    targets: &[MachLabel],
) -> Option<()> {
    let mut isle_ctx = Sia32IsleContext::new(lower_ctx, backend);
    generated_code::constructor_lower_branch(&mut isle_ctx, branch, targets)
}
