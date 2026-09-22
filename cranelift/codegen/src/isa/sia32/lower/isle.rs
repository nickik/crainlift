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
    CallArgList, CallInfo, CallRetList, InstOutput, Lower, MachLabel, Reg, StackAMode,
    VCodeConstant, VCodeConstantData, VCodeInst,
};
use alloc::boxed::Box;
type BoxCallInfo = Box<CallInfo<ExternalName>>;
type BoxCallIndInfo = Box<CallInfo<Reg>>;
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
        MInst::StackAddr { dst, mem } => MachineInst::StackAddr {
            dst: *dst,
            mem: *mem,
        },
        MInst::LoadConst32 { dst, value } => MachineInst::LoadConst32 {
            dst: *dst,
            value: *value,
        },
        MInst::LoadExtName { dst, name, offset } => MachineInst::LoadExtName {
            dst: *dst,
            name: name.as_ref().clone(),
            offset: *offset,
        },
        MInst::Add { dst, lhs, rhs } => MachineInst::Add {
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::CmpEq { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::CmpEq,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::CmpLt { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::CmpLt,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::CmpLtu { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::CmpLtu,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Mul { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Mul,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Div { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Div,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::DivU { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::DivU,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::Rem { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::Rem,
            dst: *dst,
            lhs: *lhs,
            rhs: *rhs,
        },
        MInst::RemU { dst, lhs, rhs } => MachineInst::TwoOp {
            op: TwoOp::RemU,
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
        MInst::Icmp { dst, cc, lhs, rhs } => MachineInst::Icmp {
            dst: *dst,
            cc: *cc,
            lhs: *lhs,
            rhs: *rhs,
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
        MInst::TrapIfNz { test, code } => MachineInst::TrapIfNz {
            test: *test,
            code: *code,
        },
        MInst::TrapIfZ { test, code } => MachineInst::TrapIfZ {
            test: *test,
            code: *code,
        },
        MInst::Call { info } => MachineInst::Call { info: info.clone() },
        MInst::CallInd { info } => MachineInst::CallInd { info: info.clone() },
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
        MInst::Fence {} => MachineInst::Fence,
        MInst::SRead { dst, selector } => MachineInst::SRead {
            dst: *dst,
            selector: *selector,
        },
        MInst::ReadFixedGpr { dst, index } => MachineInst::ReadFixedGpr {
            dst: *dst,
            index: *index,
        },
        MInst::WriteFixedGpr { src, index } => MachineInst::WriteFixedGpr {
            src: *src,
            index: *index,
        },
        MInst::SWrite { src, selector } => MachineInst::SWrite {
            src: *src,
            selector: *selector,
        },
        MInst::SRet {} => MachineInst::SRet,
        MInst::TlbFence {} => MachineInst::TlbFence,
        MInst::TlbFenceVa { src } => MachineInst::TlbFenceVa { src: *src },
        MInst::TlbFenceAsid { src } => MachineInst::TlbFenceAsid { src: *src },
        MInst::Wfi {} => MachineInst::Wfi,
        MInst::SyncI {} => MachineInst::SyncI,
        MInst::SoftwareTrap { code } => MachineInst::SoftwareTrap { code: *code },
    }
}

impl generated_code::Context for Sia32IsleContext<'_, '_> {
    isle_lower_prelude_methods!();

    fn emit(&mut self, inst: &MInst) -> Unit {
        self.lower_ctx.emit(into_machine_inst(inst));
    }

    fn gen_stack_addr(&mut self, slot: StackSlot, offset: Offset32) -> Reg {
        let result = self.temp_writable_reg(I32);
        let inst =
            self.lower_ctx
                .abi()
                .sized_stackslot_addr(slot, i64::from(offset) as u32, result);
        self.lower_ctx.emit(inst);
        result.to_reg()
    }

    fn sia_load_const(&mut self, dst: WritableReg, value: u64) -> MInst {
        let value = u32::try_from(value).expect("SIA32 word iconst must fit 32 bits");
        MInst::LoadConst32 { dst, value }
    }

    fn sia_load_ext_name(
        &mut self,
        dst: WritableReg,
        name: BoxExternalName,
        offset: i64,
    ) -> MInst {
        MInst::LoadExtName { dst, name, offset }
    }

    fn sia_add(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Add { dst, lhs, rhs }
    }

    fn sia_cmpeq(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::CmpEq { dst, lhs, rhs }
    }

    fn sia_cmplt(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::CmpLt { dst, lhs, rhs }
    }

    fn sia_cmpltu(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::CmpLtu { dst, lhs, rhs }
    }

    fn sia_mul(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Mul { dst, lhs, rhs }
    }
    fn sia_div(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Div { dst, lhs, rhs }
    }
    fn sia_divu(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::DivU { dst, lhs, rhs }
    }
    fn sia_rem(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Rem { dst, lhs, rhs }
    }
    fn sia_remu(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::RemU { dst, lhs, rhs }
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

    fn sia_icmp(&mut self, dst: WritableReg, cc: &IntCC, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Icmp {
            dst,
            cc: *cc,
            lhs,
            rhs,
        }
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

    fn gen_call_info(
        &mut self,
        sig: Sig,
        dest: ExternalName,
        uses: CallArgList,
        defs: CallRetList,
        try_call_info: OptionTryCallInfo,
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

    fn gen_call_ind_info(
        &mut self,
        sig: Sig,
        dest: Reg,
        uses: CallArgList,
        defs: CallRetList,
        try_call_info: OptionTryCallInfo,
    ) -> BoxCallIndInfo {
        let stack_ret_space = self.lower_ctx.sigs()[sig].sized_stack_ret_space();
        let stack_arg_space = self.lower_ctx.sigs()[sig].sized_stack_arg_space();
        self.lower_ctx
            .abi_mut()
            .accumulate_outgoing_args_size(stack_ret_space + stack_arg_space);
        Box::new(
            self.lower_ctx
                .gen_call_info(sig, dest, uses, defs, try_call_info, false),
        )
    }

    fn sia_m_gpr_read(&mut self, dst: WritableReg, register: u8) -> MInst {
        assert!(
            matches!(register, 1..=11 | 13 | 15),
            "fixed GPR read must name an ABI GPR or architectural SP"
        );
        MInst::ReadFixedGpr {
            dst,
            index: register,
        }
    }
    fn sia_m_gpr_write(&mut self, src: Reg, register: u8) -> MInst {
        assert!(
            matches!(register, 1..=11 | 13 | 15),
            "fixed GPR write must name an ABI GPR or architectural SP"
        );
        MInst::WriteFixedGpr {
            src,
            index: register,
        }
    }
    fn sia_m_sread(&mut self, dst: WritableReg, selector: u8) -> MInst {
        MInst::SRead { dst, selector }
    }
    fn sia_m_swrite(&mut self, src: Reg, selector: u8) -> MInst {
        MInst::SWrite { src, selector }
    }
    fn sia_m_sret(&mut self) -> MInst {
        MInst::SRet {}
    }
    fn sia_m_tlbfence(&mut self) -> MInst {
        MInst::TlbFence {}
    }
    fn sia_m_tlbfence_va(&mut self, src: Reg) -> MInst {
        MInst::TlbFenceVa { src }
    }
    fn sia_m_tlbfence_asid(&mut self, src: Reg) -> MInst {
        MInst::TlbFenceAsid { src }
    }
    fn sia_m_wfi(&mut self) -> MInst {
        MInst::Wfi {}
    }
    fn sia_m_sync_i(&mut self) -> MInst {
        MInst::SyncI {}
    }
    fn sia_m_trap(&mut self, code: u8) -> MInst {
        MInst::SoftwareTrap { code }
    }

    fn sia_fence(&mut self) -> MInst {
        MInst::Fence
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
