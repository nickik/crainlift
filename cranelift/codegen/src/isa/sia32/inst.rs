//! SIA32 machine-instruction vocabulary used by the MachInst backend.
//!
//! This layer separates semantic machine operations from final 16-bit
//! encodings. Some semantic operations (for example destructive two-operand
//! SIA instructions whose destination differs from the CLIF lhs) may expand to
//! more than one architectural instruction during emission.

use super::abi::Sia32MachineDeps;
use super::label::LabelUse;
use super::{encode, regs};
use crate::binemit::CodeOffset;
use crate::ir::types::{I32, I64};
use crate::ir::{self, Type};
use crate::isa::FunctionAlignment;
use crate::machinst::{
    ArgPair, CallType, Callee, FrameLayout, MachBuffer, MachInst, MachInstEmit,
    MachInstEmitState, MachLabel, MachTerminator, OperandVisitor, Reg, RetPair, StackAMode,
    Writable,
};
use crate::machinst::reg::OperandVisitorImpl;
use crate::{CodegenError, CodegenResult};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use cranelift_control::ControlPlane;
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
    Args { args: Vec<ArgPair> },
    Rets { rets: Vec<RetPair> },
    DummyUse { reg: Reg },
    Nop,
    Trap { code: u8 },
    Mov { dst: Writable<Reg>, src: Reg },
    Add { dst: Writable<Reg>, lhs: Reg, rhs: Reg },
    Unary { op: UnaryOp, dst: Writable<Reg>, src: Reg },
    TwoOp { op: TwoOp, dst: Writable<Reg>, lhs: Reg, rhs: Reg },
    ShiftImm { op: TwoOp, dst: Writable<Reg>, src: Reg, amount: u8 },
    Li7 { dst: Writable<Reg>, imm: i8 },
    Addi7 { dst: Writable<Reg>, src: Reg, imm: i8 },
    LoadConst32 { dst: Writable<Reg>, value: u32 },
    Load { op: LoadOp, dst: Writable<Reg>, base: Reg },
    Store { op: StoreOp, src: Reg, base: Reg },
    IndexedLoad { dst: Writable<Reg>, base: Reg, index: Reg },
    IndexedStore { src: Reg, base: Reg, index: Reg },

    // ABI pseudos retain frame-relative intent until final emission.
    LoadStack { dst: Writable<Reg>, mem: StackAMode, ty: Type },
    StoreStack { src: Reg, mem: StackAMode, ty: Type },
    StackAddr { dst: Writable<Reg>, mem: StackAMode },
    LoadBaseOffset { dst: Writable<Reg>, base: Reg, offset: i32, ty: Type },
    StoreBaseOffset { src: Reg, base: Reg, offset: i32, ty: Type },
    Extend { dst: Writable<Reg>, src: Reg, signed: bool, from_bits: u8, to_bits: u8 },
    AddImm { dst: Writable<Reg>, src: Reg, imm: i32 },
    SpAdjust { amount: i32 },
    StackLowerBoundTrap { limit: Reg },

    Jump { target: MachLabel },
    BrNz { test: Reg, taken: MachLabel, not_taken: MachLabel },
    Ret,

    // Deliberately not a real call until M4 installs CallInfo + relocations.
    CallPlaceholder { target: Box<str> },
}

const INT1_RCS: [RegClass; 1] = [RegClass::Int];
const INT1_TYS: [Type; 1] = [I32];
const INT2_RCS: [RegClass; 2] = [RegClass::Int, RegClass::Int];
const INT2_TYS: [Type; 2] = [I32, I32];

pub(crate) fn rc_for_type(ty: &Type) -> CodegenResult<(&'static [RegClass], &'static [Type])> {
    match *ty {
        t if t.is_int() && t.bits() <= 32 => Ok((&INT1_RCS, &INT1_TYS)),
        I64 => Ok((&INT2_RCS, &INT2_TYS)),
        _ => Err(CodegenError::Unsupported(format!(
            "SIA32 does not yet support SSA value type {ty}"
        ))),
    }
}

fn arch_reg(reg: Reg) -> regs::Reg {
    let real = reg
        .to_real_reg()
        .expect("SIA32 emission requires allocated real registers");
    assert_eq!(real.class(), RegClass::Int);
    regs::Reg::new(real.hw_enc()).expect("SIA32 real register encoding must be r0..r15")
}

fn put_word(code: &mut MachBuffer<Inst>, word: u16) {
    code.put2(word);
}

impl Inst {
    pub(crate) fn encoded_worst_case_size(&self) -> u32 {
        match self {
            Self::TwoOp { .. } | Self::ShiftImm { .. } | Self::Addi7 { .. } => 4,
            Self::LoadConst32 { .. } => 10,
            Self::BrNz { .. } => 4,
            Self::LoadStack { .. }
            | Self::StoreStack { .. }
            | Self::StackAddr { .. }
            | Self::LoadBaseOffset { .. }
            | Self::StoreBaseOffset { .. }
            | Self::Extend { .. }
            | Self::AddImm { .. }
            | Self::SpAdjust { .. }
            | Self::StackLowerBoundTrap { .. } => 12,
            Self::Args { .. } | Self::Rets { .. } | Self::DummyUse { .. } => 0,
            _ => 2,
        }
    }
}

impl MachInst for Inst {
    type LabelUse = LabelUse;
    type ABIMachineSpec = Sia32MachineDeps;

    const TRAP_OPCODE: &'static [u8] = &[0x0f, 0xc0];

    fn get_operands(&mut self, collector: &mut impl OperandVisitor) {
        match self {
            Self::Args { args } => {
                for ArgPair { vreg, preg } in args {
                    collector.reg_fixed_def(vreg, *preg);
                }
            }
            Self::Rets { rets } => {
                for RetPair { vreg, preg } in rets {
                    collector.reg_fixed_use(vreg, *preg);
                }
            }
            Self::DummyUse { reg } => collector.reg_use(reg),
            Self::Nop | Self::Trap { .. } | Self::Jump { .. } | Self::Ret => {}
            Self::Mov { dst, src } => {
                collector.reg_use(src);
                collector.reg_def(dst);
            }
            Self::Add { dst, lhs, rhs } | Self::TwoOp { dst, lhs, rhs, .. } => {
                collector.reg_use(lhs);
                collector.reg_use(rhs);
                collector.reg_def(dst);
            }
            Self::Unary { dst, src, .. }
            | Self::ShiftImm { dst, src, .. }
            | Self::Addi7 { dst, src, .. }
            | Self::Extend { dst, src, .. }
            | Self::AddImm { dst, src, .. } => {
                collector.reg_use(src);
                collector.reg_def(dst);
            }
            Self::Li7 { dst, .. } | Self::LoadConst32 { dst, .. } | Self::StackAddr { dst, .. } => {
                collector.reg_def(dst);
            }
            Self::Load { dst, base, .. } | Self::LoadBaseOffset { dst, base, .. } => {
                collector.reg_use(base);
                collector.reg_def(dst);
            }
            Self::Store { src, base, .. } | Self::StoreBaseOffset { src, base, .. } => {
                collector.reg_use(src);
                collector.reg_use(base);
            }
            Self::IndexedLoad { dst, base, index } => {
                collector.reg_use(base);
                collector.reg_use(index);
                collector.reg_def(dst);
            }
            Self::IndexedStore { src, base, index } => {
                collector.reg_use(src);
                collector.reg_use(base);
                collector.reg_use(index);
            }
            Self::LoadStack { dst, .. } => collector.reg_def(dst),
            Self::StoreStack { src, .. } => collector.reg_use(src),
            Self::SpAdjust { .. } => {}
            Self::StackLowerBoundTrap { limit } => collector.reg_use(limit),
            Self::BrNz { test, .. } => collector.reg_use(test),
            Self::CallPlaceholder { .. } => {}
        }
    }

    fn is_move(&self) -> Option<(Writable<Reg>, Reg)> {
        match *self {
            Self::Mov { dst, src } => Some((dst, src)),
            _ => None,
        }
    }

    fn is_term(&self) -> MachTerminator {
        match self {
            Self::Rets { .. } | Self::Ret => MachTerminator::Ret,
            Self::Jump { .. } | Self::BrNz { .. } => MachTerminator::Branch,
            _ => MachTerminator::None,
        }
    }

    fn is_trap(&self) -> bool {
        matches!(self, Self::Trap { .. })
    }

    fn is_args(&self) -> bool {
        matches!(self, Self::Args { .. })
    }

    fn call_type(&self) -> CallType {
        CallType::None
    }

    fn is_included_in_clobbers(&self) -> bool {
        !self.is_args()
    }

    fn is_mem_access(&self) -> bool {
        matches!(
            self,
            Self::Load { .. }
                | Self::Store { .. }
                | Self::IndexedLoad { .. }
                | Self::IndexedStore { .. }
                | Self::LoadStack { .. }
                | Self::StoreStack { .. }
                | Self::LoadBaseOffset { .. }
                | Self::StoreBaseOffset { .. }
        )
    }

    fn gen_move(to_reg: Writable<Reg>, from_reg: Reg, ty: Type) -> Self {
        debug_assert_eq!(ty, I32);
        Self::Mov { dst: to_reg, src: from_reg }
    }

    fn gen_dummy_use(reg: Reg) -> Self {
        Self::DummyUse { reg }
    }

    fn rc_for_type(ty: &Type) -> CodegenResult<(&[RegClass], &[Type])> {
        rc_for_type(ty)
    }

    fn canonical_type_for_rc(rc: RegClass) -> Type {
        match rc {
            RegClass::Int => I32,
            RegClass::Float | RegClass::Vector => unreachable!("SIA32 has only integer registers"),
        }
    }

    fn gen_jump(target: MachLabel) -> Self {
        Self::Jump { target }
    }

    fn gen_imm_u64(value: u64, dst: Writable<Reg>) -> Option<Self> {
        u32::try_from(value).ok().map(|value| Self::LoadConst32 { dst, value })
    }

    fn gen_nop(_preferred_size: usize) -> Self {
        Self::Nop
    }

    fn gen_nop_units() -> Vec<Vec<u8>> {
        vec![encode::NOP.to_le_bytes().to_vec()]
    }

    fn worst_case_size() -> CodeOffset {
        16
    }

    fn worst_case_island_growth() -> CodeOffset {
        32
    }

    fn is_safepoint(&self) -> bool {
        self.is_trap()
    }

    fn function_alignment() -> FunctionAlignment {
        FunctionAlignment { minimum: 2, preferred: 4 }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EmitState {
    ctrl_plane: ControlPlane,
    frame_layout: FrameLayout,
    user_stack_map: Option<ir::UserStackMap>,
}

impl MachInstEmitState<Inst> for EmitState {
    fn new(abi: &Callee<Sia32MachineDeps>, ctrl_plane: ControlPlane) -> Self {
        Self {
            ctrl_plane,
            frame_layout: abi.frame_layout().clone(),
            user_stack_map: None,
        }
    }

    fn pre_safepoint(&mut self, user_stack_map: Option<ir::UserStackMap>) {
        self.user_stack_map = user_stack_map;
    }

    fn ctrl_plane_mut(&mut self) -> &mut ControlPlane {
        &mut self.ctrl_plane
    }

    fn take_ctrl_plane(self) -> ControlPlane {
        self.ctrl_plane
    }

    fn frame_layout(&self) -> &FrameLayout {
        &self.frame_layout
    }
}

impl MachInstEmit for Inst {
    type State = EmitState;
    type Info = ();

    fn emit(&self, code: &mut MachBuffer<Inst>, _info: &Self::Info, _state: &mut EmitState) {
        let start = code.cur_offset();
        match self {
            Self::Args { .. } | Self::Rets { .. } | Self::DummyUse { .. } => {}
            Self::Nop => put_word(code, encode::NOP),
            Self::Trap { code: trap_code } => {
                put_word(code, encode::trap(*trap_code).expect("backend trap code must be encodable"));
            }
            Self::Mov { dst, src } => put_word(code, encode::mov(arch_reg(dst.to_reg()), arch_reg(*src))),
            Self::Add { dst, lhs, rhs } => put_word(code, encode::add(arch_reg(dst.to_reg()), arch_reg(*lhs), arch_reg(*rhs))),
            Self::Unary { op, dst, src } => {
                let word = match op {
                    UnaryOp::Clz => encode::clz(arch_reg(dst.to_reg()), arch_reg(*src)),
                    UnaryOp::Ctz => encode::ctz(arch_reg(dst.to_reg()), arch_reg(*src)),
                    UnaryOp::Cpop => encode::cpop(arch_reg(dst.to_reg()), arch_reg(*src)),
                };
                put_word(code, word);
            }
            Self::TwoOp { op, dst, lhs, rhs } => {
                let d = arch_reg(dst.to_reg());
                let l = arch_reg(*lhs);
                let r = arch_reg(*rhs);
                if d != l { put_word(code, encode::mov(d, l)); }
                let word = match op {
                    TwoOp::Sub => encode::sub(d, r), TwoOp::Addo => encode::addo(d, r),
                    TwoOp::Subo => encode::subo(d, r), TwoOp::CmpEq => encode::cmpeq(d, r),
                    TwoOp::CmpLt => encode::cmplt(d, r), TwoOp::CmpLtu => encode::cmpltu(d, r),
                    TwoOp::Min => encode::min(d, r), TwoOp::MinU => encode::minu(d, r),
                    TwoOp::Max => encode::max(d, r), TwoOp::MaxU => encode::maxu(d, r),
                    TwoOp::And => encode::and(d, r), TwoOp::Or => encode::or(d, r),
                    TwoOp::Xor => encode::xor(d, r), TwoOp::Shl => encode::shl(d, r),
                    TwoOp::Shr => encode::shr(d, r), TwoOp::Sar => encode::sar(d, r),
                    TwoOp::BSet => encode::bset(d, r), TwoOp::BClr => encode::bclr(d, r),
                    TwoOp::BInv => encode::binv(d, r), TwoOp::BExt => encode::bext(d, r),
                    TwoOp::Rev8 => encode::rev8(d, r),
                };
                put_word(code, word);
            }
            Self::ShiftImm { op, dst, src, amount } => {
                let d = arch_reg(dst.to_reg());
                let s = arch_reg(*src);
                if d != s { put_word(code, encode::mov(d, s)); }
                let word = match op {
                    TwoOp::Shl => encode::shli(d, *amount),
                    TwoOp::Shr => encode::shri(d, *amount),
                    TwoOp::Sar => encode::sari(d, *amount),
                    _ => panic!("invalid SIA32 immediate-shift operation"),
                }.expect("shift amount must be validated during lowering");
                put_word(code, word);
            }
            Self::Li7 { dst, imm } => put_word(code, encode::li(arch_reg(dst.to_reg()), i32::from(*imm)).expect("LI7 immediate must be in range")),
            Self::Addi7 { dst, src, imm } => {
                let d = arch_reg(dst.to_reg());
                let s = arch_reg(*src);
                if d != s { put_word(code, encode::mov(d, s)); }
                put_word(code, encode::addi(d, i32::from(*imm)).expect("ADDI7 immediate must be in range"));
            }
            Self::Load { op, dst, base } => {
                let d = arch_reg(dst.to_reg()); let b = arch_reg(*base);
                put_word(code, match op {
                    LoadOp::I8 => encode::lb(d,b), LoadOp::U8 => encode::lbu(d,b),
                    LoadOp::I16 => encode::lh(d,b), LoadOp::U16 => encode::lhu(d,b),
                    LoadOp::I32 => encode::lw(d,b),
                });
            }
            Self::Store { op, src, base } => {
                let s = arch_reg(*src); let b = arch_reg(*base);
                put_word(code, match op {
                    StoreOp::I8 => encode::sb(s,b), StoreOp::I16 => encode::sh(s,b),
                    StoreOp::I32 => encode::sw(s,b),
                });
            }
            Self::IndexedLoad { dst, base, index } => put_word(code, encode::lda_w(arch_reg(dst.to_reg()), arch_reg(*base), arch_reg(*index))),
            Self::IndexedStore { src, base, index } => put_word(code, encode::sta_w(arch_reg(*src), arch_reg(*base), arch_reg(*index))),
            Self::Jump { target } => {
                code.use_label_at_offset(start, *target, LabelUse::Branch11);
                code.add_uncond_branch(start, start + 2, *target);
                put_word(code, encode::b(0).unwrap());
            }
            Self::BrNz { test, taken, not_taken } => {
                code.use_label_at_offset(start, *taken, LabelUse::Cond7);
                put_word(code, encode::bnz(arch_reg(*test), 0).unwrap());
                let second = code.cur_offset();
                code.use_label_at_offset(second, *not_taken, LabelUse::Branch11);
                code.add_uncond_branch(second, second + 2, *not_taken);
                put_word(code, encode::b(0).unwrap());
            }
            Self::Ret => put_word(code, encode::ret()),
            Self::LoadConst32 { .. }
            | Self::LoadStack { .. }
            | Self::StoreStack { .. }
            | Self::StackAddr { .. }
            | Self::LoadBaseOffset { .. }
            | Self::StoreBaseOffset { .. }
            | Self::Extend { .. }
            | Self::AddImm { .. }
            | Self::SpAdjust { .. }
            | Self::StackLowerBoundTrap { .. }
            | Self::CallPlaceholder { .. } => {
                panic!("SIA32 pseudo reached emitter before expansion was implemented: {self:?}")
            }
        }
        debug_assert!(code.cur_offset() - start <= Self::worst_case_size());
    }

    fn pretty_print_inst(&self, _state: &mut Self::State) -> String {
        format!("{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{F32, I8, I16, I128};
    use crate::isa::sia32::regs::mach_reg;

    fn w(n: u8) -> Writable<Reg> { Writable::from_reg(mach_reg(n)) }

    #[test]
    fn scalar_and_i64_register_representation_is_32_bit_native() {
        for ty in [I8, I16, I32] {
            let (rcs, tys) = rc_for_type(&ty).unwrap();
            assert_eq!(rcs, &[RegClass::Int]); assert_eq!(tys, &[I32]);
        }
        let (rcs, tys) = rc_for_type(&I64).unwrap();
        assert_eq!(rcs, &[RegClass::Int, RegClass::Int]); assert_eq!(tys, &[I32, I32]);
    }

    #[test]
    fn unsupported_value_classes_fail_explicitly() {
        assert!(rc_for_type(&F32).is_err()); assert!(rc_for_type(&I128).is_err());
    }

    #[test]
    fn move_and_memory_classification_is_exact() {
        let mov = Inst::Mov { dst: w(1), src: mach_reg(2) };
        assert_eq!(MachInst::is_move(&mov), Some((w(1), mach_reg(2))));
        assert!(!mov.is_mem_access());
        let load = Inst::Load { op: LoadOp::I32, dst: w(1), base: mach_reg(2) };
        assert!(load.is_mem_access());
    }

    #[test]
    fn pseudos_report_conservative_sizes() {
        let op = Inst::TwoOp { op: TwoOp::Sub, dst: w(1), lhs: mach_reg(2), rhs: mach_reg(3) };
        assert_eq!(op.encoded_worst_case_size(), 4);
        let constant = Inst::LoadConst32 { dst: w(1), value: 0xdead_beef };
        assert_eq!(constant.encoded_worst_case_size(), 10);
    }

    #[test]
    fn trap_and_nop_bytes_are_exact() {
        assert_eq!(Inst::TRAP_OPCODE, &[0x0f, 0xc0]);
        assert_eq!(Inst::gen_nop_units(), vec![vec![0xff, 0xcf]]);
    }
}