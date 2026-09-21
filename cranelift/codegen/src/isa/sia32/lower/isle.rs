//! ISLE integration glue for the SIA32 integer lowering rules.

pub mod generated_code;

use self::generated_code::MInst;
use crate::ir::condcodes::{FloatCC, IntCC};
use crate::ir::{
    BlockCall, Inst, InstructionData, MemFlagsData, Opcode, TrapCode, Type, Value, ValueList,
    immediates::*, types::*,
};
use crate::isa::sia32::Sia32Backend;
use crate::isa::sia32::inst::{Inst as MachineInst, TwoOp, UnaryOp};
use crate::machinst::isle::*;
use crate::machinst::{
    CallArgList, CallInfo, CallRetList, InstOutput, Lower, MachLabel, Reg, StackAMode, VCodeConstant,
    VCodeConstantData, VCodeInst,
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use regalloc2::PReg;

type BoxCallInfo = Box<CallInfo<crate::ir::ExternalName>>;

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

impl MInst {
    // Required by the shared lowering prelude. Keep `ty` in the signature to
    // match MachInst::gen_move even though SIA's scalar register move encoding
    // is type-independent for the R0 integer subset.
    fn gen_move(dst: WritableReg, src: Reg, _ty: Type) -> Self {
        Self::Mov { dst, src }
    }
}

impl From<MachineInst> for MInst {
    fn from(inst: MachineInst) -> Self {
        match inst {
            MachineInst::Mov { dst, src } => Self::Mov { dst, src },
            MachineInst::StackAddr { dst, mem } => Self::StackAddr { dst, mem },
            other => {
                panic!("SIA ISLE wrapper cannot represent shared-prelude instruction {other:?}")
            }
        }
    }
}

fn into_machine_inst(inst: &MInst) -> MachineInst {
    match inst {
        MInst::Mov { dst, src } => MachineInst::Mov {
            dst: *dst,
            src: *src,
        },
        MInst::Trap { code } => MachineInst::Trap { code: *code },
        MInst::StackAddr { dst, mem } => MachineInst::StackAddr {
            dst: *dst,
            mem: *mem,
        },
        MInst::LoadConst32 { dst, value } => MachineInst::LoadConst32 {
            dst: *dst,
            value: *value,
        },
        MInst::Add { dst, lhs, rhs } => MachineInst::Add {
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Sub { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Sub,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::And { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::And,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Or { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Or,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Xor { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Xor,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Shl { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Shl,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Shr { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Shr,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Sar { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Sar,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Clz { dst, src } => MachineInst::Unary {
            op: UnaryOp::Clz,
            dst: *dst,
            src: *src,
        },
        MInst::Ctz { dst, src } => MachineInst::Unary {
            op: UnaryOp::Ctz,
            dst: *dst,
            src: *src,
        },
        MInst::Cpop { dst, src } => MachineInst::Unary {
            op: UnaryOp::Cpop,
            dst: *dst,
            src: *src,
        },
        MInst::Extend {
            dst,
            src,
            signed,
            from_bits,
            to_bits,
        } => MachineInst::Extend {
            dst: *dst,
            src: *src,
            signed: *signed,
            from_bits: *from_bits,
            to_bits: *to_bits,
        },
        MInst::LoadBaseOffset {
            dst,
            base,
            offset,
            ty,
        } => MachineInst::LoadBaseOffset {
            dst: *dst,
            base: *base,
            offset: *offset,
            ty: *ty,
        },
        MInst::StoreBaseOffset {
            src,
            base,
            offset,
            ty,
        } => MachineInst::StoreBaseOffset {
            src: *src,
            base: *base,
            offset: *offset,
            ty: *ty,
        },
        MInst::Jump { target } => MachineInst::Jump { target: *target },
        MInst::BrNz {
            test,
            taken,
            not_taken,
        } => MachineInst::BrNz {
            test: *test,
            taken: *taken,
            not_taken: *not_taken,
        },
        MInst::Call { info } => MachineInst::Call { info: info.clone() },
    }
}

impl generated_code::Context for Sia32IsleContext<'_, '_> {
    isle_lower_prelude_methods!();

    fn emit(&mut self, inst: &MInst) -> Unit {
        self.lower_ctx.emit(into_machine_inst(inst));
    }

    fn gen_call_info(
        &mut self,
        sig: Sig,
        dest: crate::ir::ExternalName,
        uses: CallArgList,
        defs: CallRetList,
        try_call_info: Option<crate::machinst::TryCallInfo>,
        patchable: bool,
    ) -> BoxCallInfo {
        let stack_ret_space = self.lower_ctx.sigs()[sig].sized_stack_ret_space();
        let stack_arg_space = self.lower_ctx.sigs()[sig].sized_stack_arg_space();
        self.lower_ctx
            .abi_mut()
            .accumulate_outgoing_args_size(stack_ret_space + stack_arg_space);

        Box::new(
            self.lower_ctx
                .gen_call_info(sig, dest, uses, defs, try_call_info, patchable),
        )
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
