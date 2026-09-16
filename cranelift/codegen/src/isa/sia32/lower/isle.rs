//! ISLE integration glue for the SIA32 integer lowering rules.

pub mod generated_code;

use crate::isa::sia32::Sia32Backend;
use crate::isa::sia32::inst::{Inst as MachineInst, TwoOp, UnaryOp};
use crate::machinst::isle::*;
use crate::machinst::{Lower, MachLabel, Reg, VCodeInst};

// `inst.isle` declares MInst as a primitive. Generated ISLE code resolves that
// primitive through this type alias rather than defining a second instruction
// enum, so the Rust emitter remains authoritative.
type MInst = MachineInst;

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
    pub lower_ctx: &'a mut Lower<'b, MInst>,
    pub backend: &'a Sia32Backend,
}

impl<'a, 'b> Sia32IsleContext<'a, 'b> {
    fn new(lower_ctx: &'a mut Lower<'b, MInst>, backend: &'a Sia32Backend) -> Self {
        Self { lower_ctx, backend }
    }
}

impl generated_code::Context for Sia32IsleContext<'_, '_> {
    isle_lower_prelude_methods!();

    fn sia_load_const(&mut self, dst: WritableReg, value: u64) -> MInst {
        let value = u32::try_from(value).expect("SIA32 word iconst must fit 32 bits");
        MInst::LoadConst32 { dst, value }
    }

    fn sia_add(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::Add { dst, lhs, rhs }
    }

    fn sia_sub(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::Sub, dst, lhs, rhs }
    }

    fn sia_and(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::And, dst, lhs, rhs }
    }

    fn sia_or(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::Or, dst, lhs, rhs }
    }

    fn sia_xor(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::Xor, dst, lhs, rhs }
    }

    fn sia_shl(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::Shl, dst, lhs, rhs }
    }

    fn sia_shr(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::Shr, dst, lhs, rhs }
    }

    fn sia_sar(&mut self, dst: WritableReg, lhs: Reg, rhs: Reg) -> MInst {
        MInst::TwoOp { op: TwoOp::Sar, dst, lhs, rhs }
    }

    fn sia_clz(&mut self, dst: WritableReg, src: Reg) -> MInst {
        MInst::Unary { op: UnaryOp::Clz, dst, src }
    }

    fn sia_ctz(&mut self, dst: WritableReg, src: Reg) -> MInst {
        MInst::Unary { op: UnaryOp::Ctz, dst, src }
    }

    fn sia_popcnt(&mut self, dst: WritableReg, src: Reg) -> MInst {
        MInst::Unary { op: UnaryOp::Cpop, dst, src }
    }

    fn sia_extend(
        &mut self,
        dst: WritableReg,
        src: Reg,
        signed: bool,
        from_bits: u8,
        to_bits: u8,
    ) -> MInst {
        MInst::Extend { dst, src, signed, from_bits, to_bits }
    }

    fn sia_load_offset(&mut self, dst: WritableReg, base: Reg, offset: i32, ty: Type) -> MInst {
        MInst::LoadBaseOffset { dst, base, offset, ty }
    }

    fn sia_store_offset(&mut self, src: Reg, base: Reg, offset: i32, ty: Type) -> MInst {
        MInst::StoreBaseOffset { src, base, offset, ty }
    }

    fn sia_jump(&mut self, target: MachLabel) -> MInst {
        MInst::Jump { target }
    }

    fn sia_brnz(&mut self, test: Reg, taken: MachLabel, not_taken: MachLabel) -> MInst {
        MInst::BrNz { test, taken, not_taken }
    }
}

pub(crate) fn lower(
    lower_ctx: &mut Lower<MInst>,
    backend: &Sia32Backend,
    inst: Inst,
) -> Option<InstOutput> {
    let mut isle_ctx = Sia32IsleContext::new(lower_ctx, backend);
    match generated_code::constructor_lower(&mut isle_ctx, inst) {
        Ok(output) => output,
        Err(_) => panic!("SIA32 ISLE lower constructor returned an internal error for {inst:?}"),
    }
}

pub(crate) fn lower_branch(
    lower_ctx: &mut Lower<MInst>,
    backend: &Sia32Backend,
    branch: Inst,
    targets: &[MachLabel],
) -> Option<()> {
    let mut isle_ctx = Sia32IsleContext::new(lower_ctx, backend);
    match generated_code::constructor_lower_branch(&mut isle_ctx, branch, targets) {
        Ok(output) => output,
        Err(_) => panic!("SIA32 ISLE branch constructor returned an internal error for {branch:?}"),
    }
}
