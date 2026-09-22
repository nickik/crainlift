//! SIA32 machine-instruction vocabulary used by the MachInst backend.
//!
//! Semantic instructions remain independent of the final 16-bit encoding. ABI
//! pseudos are expanded here after register allocation, using r12 as the one
//! reserved compiler scratch register.

use super::abi::Sia32MachineDeps;
use super::label::LabelUse;
use super::{encode, regs};
use crate::binemit::{CodeOffset, Reloc};
use crate::ir::condcodes::IntCC;
use crate::ir::types::{I8, I16, I32, I64};
use crate::ir::{self, ExternalName, Type};
use crate::isa::FunctionAlignment;
use crate::machinst::reg::OperandVisitorImpl;
use crate::machinst::{
    ArgPair, CallArgPair, CallInfo, CallRetPair, CallType, Callee, FrameLayout, FunctionCalls,
    MachBuffer, MachInst, MachInstEmit, MachInstEmitState, MachLabel, MachTerminator,
    OperandVisitor, Reg, RetLocation, RetPair, StackAMode, Writable,
};
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
    Mul,
    Div,
    DivU,
    Rem,
    RemU,
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
pub(crate) enum PrivilegedOp {
    SoftwareTrap { code: u8 },
    SRead { selector: u8 },
    SWrite { selector: u8 },
    SSwapScratch,
    SRet,
    SRetCtx,
    TlbFence,
    TlbFenceVa,
    TlbFenceAsid,
    Wfi,
    SyncI,
    Fence,
}

#[derive(Clone, Debug)]
pub(crate) enum Inst {
    Args {
        args: Vec<ArgPair>,
    },
    Rets {
        rets: Vec<RetPair>,
    },
    DummyUse {
        reg: Reg,
    },
    Nop,
    SoftwareTrap {
        code: u8,
    },
    TrapIfNz {
        test: Reg,
        code: ir::TrapCode,
    },
    TrapIfZ {
        test: Reg,
        code: ir::TrapCode,
    },
    Mov {
        dst: Writable<Reg>,
        src: Reg,
    },
    Add {
        dst: Writable<Reg>,
        lhs: Reg,
        rhs: Reg,
    },
    Unary {
        op: UnaryOp,
        dst: Writable<Reg>,
        src: Reg,
    },
    TwoOp {
        op: TwoOp,
        dst: Writable<Reg>,
        lhs: Reg,
        rhs: Reg,
    },
    Icmp {
        dst: Writable<Reg>,
        cc: IntCC,
        lhs: Reg,
        rhs: Reg,
    },
    ShiftImm {
        op: TwoOp,
        dst: Writable<Reg>,
        src: Reg,
        amount: u8,
    },
    Li7 {
        dst: Writable<Reg>,
        imm: i8,
    },
    Addi7 {
        dst: Writable<Reg>,
        src: Reg,
        imm: i8,
    },
    LoadConst32 {
        dst: Writable<Reg>,
        value: u32,
    },
    LoadExtName {
        dst: Writable<Reg>,
        name: ExternalName,
        offset: i64,
    },
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
    Fence,
    SRead {
        dst: Writable<Reg>,
        selector: u8,
    },
    ReadFixedGpr {
        dst: Writable<Reg>,
        index: u8,
    },
    WriteFixedGpr {
        src: Reg,
        index: u8,
    },
    SWrite {
        src: Reg,
        selector: u8,
    },
    SSwapScratch {
        dst: Writable<Reg>,
        src: Reg,
    },
    SRet,
    SRetCtx {
        src: Reg,
    },
    TlbFence,
    TlbFenceVa {
        src: Reg,
    },
    TlbFenceAsid {
        src: Reg,
    },
    Wfi,
    SyncI,

    LoadStack {
        dst: Writable<Reg>,
        mem: StackAMode,
        ty: Type,
    },
    StoreStack {
        src: Reg,
        mem: StackAMode,
        ty: Type,
    },
    StackAddr {
        dst: Writable<Reg>,
        mem: StackAMode,
    },
    LoadBaseOffset {
        dst: Writable<Reg>,
        base: Reg,
        offset: i32,
        ty: Type,
    },
    StoreBaseOffset {
        src: Reg,
        base: Reg,
        offset: i32,
        ty: Type,
    },
    Extend {
        dst: Writable<Reg>,
        src: Reg,
        signed: bool,
        from_bits: u8,
        to_bits: u8,
    },
    AddImm {
        dst: Writable<Reg>,
        src: Reg,
        imm: i32,
    },
    SpAdjust {
        amount: i32,
    },
    StackLowerBoundTrap {
        limit: Reg,
    },

    Jump {
        target: MachLabel,
    },
    BrNz {
        test: Reg,
        taken: MachLabel,
        not_taken: MachLabel,
    },
    Call {
        info: Box<CallInfo<ExternalName>>,
    },
    CallInd {
        info: Box<CallInfo<Reg>>,
    },
    Ret,
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

fn load_op(ty: Type) -> LoadOp {
    match ty {
        I8 => LoadOp::U8,
        I16 => LoadOp::U16,
        I32 => LoadOp::I32,
        _ => panic!("SIA32 scalar load pseudo has unsupported type {ty}"),
    }
}

fn store_op(ty: Type) -> StoreOp {
    match ty {
        I8 => StoreOp::I8,
        I16 => StoreOp::I16,
        I32 => StoreOp::I32,
        _ => panic!("SIA32 scalar store pseudo has unsupported type {ty}"),
    }
}

fn const_digits(value: u32) -> Vec<i8> {
    let mut n = i64::from(value as i32);
    let mut low_to_high = Vec::new();
    while n < -64 || n > 63 {
        let mut digit = n.rem_euclid(128);
        if digit > 63 {
            digit -= 128;
        }
        low_to_high.push(digit as i8);
        n = (n - digit) / 128;
    }
    low_to_high.push(n as i8);
    low_to_high.reverse();
    low_to_high
}

fn emit_const32(code: &mut MachBuffer<Inst>, dst: regs::Reg, value: u32) {
    let digits = const_digits(value);
    put_word(
        code,
        encode::li(dst, i32::from(digits[0])).expect("balanced top digit fits imm7"),
    );
    for &digit in &digits[1..] {
        put_word(code, encode::shli(dst, 7).unwrap());
        if digit != 0 {
            put_word(
                code,
                encode::addi(dst, i32::from(digit)).expect("balanced digit fits imm7"),
            );
        }
    }
}

fn emit_literal32(code: &mut MachBuffer<Inst>, state: &mut EmitState, dst: regs::Reg, value: u32) {
    let literal = code.get_label();
    let done = code.get_label();

    let load_at = code.cur_offset();
    code.use_label_at_offset(load_at, literal, LabelUse::Literal8);
    put_word(code, encode::ldpc_w(dst, 0).unwrap());

    let branch_at = code.cur_offset();
    code.use_label_at_offset(branch_at, done, LabelUse::Branch11);
    code.add_uncond_branch(branch_at, branch_at + 2, done);
    put_word(code, encode::b(0).unwrap());

    code.align_to(4);
    code.bind_label(literal, state.ctrl_plane_mut());
    code.put4(value);
    code.bind_label(done, state.ctrl_plane_mut());
}

fn emit_add_imm32(code: &mut MachBuffer<Inst>, dst: regs::Reg, src: regs::Reg, imm: i32) {
    if (-64..=63).contains(&imm) {
        if dst != src {
            put_word(code, encode::mov(dst, src));
        }
        if imm != 0 {
            put_word(code, encode::addi(dst, imm).unwrap());
        }
        return;
    }
    let scratch = regs::Reg::SCRATCH;
    if dst == scratch {
        assert!(
            src != scratch,
            "SIA32 cannot expand an arbitrary in-place add on reserved r12"
        );
        emit_const32(code, scratch, imm as u32);
        put_word(code, encode::add(scratch, scratch, src));
    } else {
        if dst != src {
            put_word(code, encode::mov(dst, src));
        }
        emit_const32(code, scratch, imm as u32);
        put_word(code, encode::add(dst, dst, scratch));
    }
}

fn frame_stack_offset(mem: &StackAMode, frame: &FrameLayout) -> i32 {
    let offset = match *mem {
        StackAMode::OutgoingArg(offset) => offset,
        StackAMode::Slot(offset) => offset + i64::from(frame.outgoing_args_size),
        StackAMode::IncomingArg(offset, stack_args_size) => {
            let current_sp_delta = frame.tail_args_size
                + frame.setup_area_size
                + frame.clobber_size
                + frame.fixed_frame_storage_size
                + frame.stackslots_size
                + frame.outgoing_args_size
                + if frame.function_calls == FunctionCalls::Regular {
                    8
                } else {
                    0
                };
            i64::from(current_sp_delta) - (i64::from(stack_args_size) - offset)
        }
    };
    i32::try_from(offset).expect("SIA32 frame offset must fit signed 32 bits")
}

fn emit_load_zero_offset(code: &mut MachBuffer<Inst>, op: LoadOp, dst: regs::Reg, base: regs::Reg) {
    let word = match op {
        LoadOp::I8 => encode::lb(dst, base),
        LoadOp::U8 => encode::lbu(dst, base),
        LoadOp::I16 => encode::lh(dst, base),
        LoadOp::U16 => encode::lhu(dst, base),
        LoadOp::I32 => encode::lw(dst, base),
    };
    put_word(code, word);
}

fn emit_store_zero_offset(
    code: &mut MachBuffer<Inst>,
    op: StoreOp,
    src: regs::Reg,
    base: regs::Reg,
) {
    let word = match op {
        StoreOp::I8 => encode::sb(src, base),
        StoreOp::I16 => encode::sh(src, base),
        StoreOp::I32 => encode::sw(src, base),
    };
    put_word(code, word);
}

fn emit_load_base_offset(
    code: &mut MachBuffer<Inst>,
    op: LoadOp,
    dst: regs::Reg,
    base: regs::Reg,
    offset: i32,
) {
    if offset == 0 {
        emit_load_zero_offset(code, op, dst, base);
        return;
    }
    if dst != base {
        emit_add_imm32(code, dst, base, offset);
        emit_load_zero_offset(code, op, dst, dst);
    } else {
        let scratch = regs::Reg::SCRATCH;
        assert!(
            base != scratch,
            "load r12,[r12+large] needs a second architectural scratch"
        );
        emit_add_imm32(code, scratch, base, offset);
        emit_load_zero_offset(code, op, dst, scratch);
    }
}

fn emit_store_base_offset(
    code: &mut MachBuffer<Inst>,
    op: StoreOp,
    src: regs::Reg,
    base: regs::Reg,
    offset: i32,
) {
    if offset == 0 {
        emit_store_zero_offset(code, op, src, base);
        return;
    }
    let scratch = regs::Reg::SCRATCH;
    assert!(
        src != scratch && base != scratch,
        "nonzero-offset store involving reserved r12 requires a second scratch and must be lowered earlier"
    );
    emit_add_imm32(code, scratch, base, offset);
    emit_store_zero_offset(code, op, src, scratch);
}

fn collect_call_operands<T>(info: &mut CallInfo<T>, collector: &mut impl OperandVisitor) {
    for CallArgPair { vreg, preg } in &mut info.uses {
        collector.reg_fixed_use(vreg, *preg);
    }
    // Register returns are definitions of ABI return registers, but those
    // registers are also present in the call clobber set. regalloc2 rejects an
    // instruction that simultaneously fixed-defs and clobbers the same preg.
    // Remove explicit return registers from this call's clobber set before
    // reporting the remaining clobbers.
    let mut clobbers = info.clobbers;
    for CallRetPair { vreg, location } in &mut info.defs {
        match location {
            RetLocation::Reg(preg, ..) => {
                collector.reg_fixed_def(vreg, *preg);
                clobbers.remove(
                    preg.to_real_reg()
                        .expect("SIA32 ABI return register must be physical")
                        .into(),
                );
            }
            RetLocation::Stack(..) => collector.any_def(vreg),
        }
    }
    collector.reg_clobbers(clobbers);
    if let Some(try_call_info) = &mut info.try_call_info {
        try_call_info.collect_operands(collector);
    }
}

fn record_call<T>(code: &mut MachBuffer<Inst>, state: &mut EmitState, info: &CallInfo<T>) {
    assert_eq!(
        info.callee_pop_size, 0,
        "SIA32 SystemV calls never use callee-pop stack arguments"
    );
    assert!(
        !info.patchable,
        "SIA32 patchable call sites are not implemented yet"
    );

    if let Some(try_call) = &info.try_call_info {
        code.add_try_call_site(
            Some(state.frame_layout.sp_to_fp()),
            try_call.exception_handlers(&state.frame_layout),
        );
    } else {
        code.add_call_site();
    }

    let ret_addr = code.cur_offset();
    if let Some(stack_map) = state.user_stack_map.take() {
        code.push_user_stack_map(state, ret_addr, stack_map);
    }
}

fn emit_direct_call(
    code: &mut MachBuffer<Inst>,
    state: &mut EmitState,
    info: &CallInfo<ExternalName>,
) {
    let literal = code.get_label();
    let done = code.get_label();

    let load_at = code.cur_offset();
    code.use_label_at_offset(load_at, literal, LabelUse::Literal8);
    put_word(code, encode::ldpc_w(regs::Reg::SCRATCH, 0).unwrap());
    put_word(code, encode::callr(regs::Reg::SCRATCH));
    record_call(code, state, info);

    // LR points here. On normal return, skip the relocated target word.
    let branch_at = code.cur_offset();
    code.use_label_at_offset(branch_at, done, LabelUse::Branch11);
    code.add_uncond_branch(branch_at, branch_at + 2, done);
    put_word(code, encode::b(0).unwrap());

    code.align_to(4);
    code.bind_label(literal, state.ctrl_plane_mut());
    code.add_reloc(Reloc::Abs4, &info.dest, 0);
    code.put4(0);
    code.bind_label(done, state.ctrl_plane_mut());
}

impl Inst {
    pub(crate) fn encoded_worst_case_size(&self) -> u32 {
        match self {
            Self::TwoOp { .. } | Self::ShiftImm { .. } | Self::Addi7 { .. } => 4,
            Self::Icmp { .. } => 10,
            Self::LoadConst32 { .. } => 18,
            Self::LoadExtName { .. } => 10,
            Self::BrNz { .. } => 4,
            Self::Extend { .. } => 6,
            Self::AddImm { .. } | Self::SpAdjust { .. } | Self::StackAddr { .. } => 22,
            Self::LoadStack { .. } | Self::LoadBaseOffset { .. } => 24,
            Self::StoreStack { .. } | Self::StoreBaseOffset { .. } => 24,
            Self::Call { .. } => 12,
            Self::CallInd { .. } => 2,
            Self::StackLowerBoundTrap { .. } => 12,
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
            Self::Nop
            | Self::SoftwareTrap { .. }
            | Self::Fence
            | Self::SRet
            | Self::TlbFence
            | Self::Wfi
            | Self::SyncI
            | Self::Jump { .. }
            | Self::Ret => {}
            Self::SRead { dst, .. } => collector.reg_def(dst),
            Self::ReadFixedGpr { dst, index } => {
                collector.reg_clobbers(regalloc2::PRegSet::empty().with(regs::preg(*index)));
                collector.reg_def(dst);
            }
            Self::WriteFixedGpr { src, index } => {
                collector.reg_use(src);
                collector.reg_clobbers(regalloc2::PRegSet::empty().with(regs::preg(*index)));
            }
            Self::SWrite { src, .. }
            | Self::SRetCtx { src }
            | Self::TlbFenceVa { src }
            | Self::TlbFenceAsid { src } => collector.reg_use(src),
            Self::SSwapScratch { dst, src } => {
                collector.reg_use(src);
                collector.reg_def(dst);
            }
            Self::TrapIfNz { test, .. } | Self::TrapIfZ { test, .. } => collector.reg_use(test),
            Self::Mov { dst, src } => {
                collector.reg_use(src);
                collector.reg_def(dst);
            }
            Self::Add { dst, lhs, rhs }
            | Self::TwoOp { dst, lhs, rhs, .. }
            | Self::Icmp { dst, lhs, rhs, .. } => {
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
            Self::Li7 { dst, .. }
            | Self::LoadConst32 { dst, .. }
            | Self::LoadExtName { dst, .. }
            | Self::StackAddr { dst, .. } => collector.reg_def(dst),
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
            Self::Call { info } => collect_call_operands(&mut **info, collector),
            Self::CallInd { info } => {
                collector.reg_use(&mut info.dest);
                collect_call_operands(&mut **info, collector);
            }
        }
    }

    fn is_move(&self) -> Option<(Writable<Reg>, Reg)> {
        if let Self::Mov { dst, src } = *self {
            Some((dst, src))
        } else {
            None
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
        false
    }
    fn is_args(&self) -> bool {
        matches!(self, Self::Args { .. })
    }
    fn call_type(&self) -> CallType {
        if matches!(self, Self::Call { .. } | Self::CallInd { .. }) {
            CallType::Regular
        } else {
            CallType::None
        }
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
        Self::Mov {
            dst: to_reg,
            src: from_reg,
        }
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
        u32::try_from(value)
            .ok()
            .map(|value| Self::LoadConst32 { dst, value })
    }
    fn gen_nop(_preferred_size: usize) -> Self {
        Self::Nop
    }
    fn gen_nop_units() -> Vec<Vec<u8>> {
        vec![encode::NOP.to_le_bytes().to_vec()]
    }
    fn worst_case_size() -> CodeOffset {
        24
    }
    fn worst_case_island_growth() -> CodeOffset {
        34
    }
    fn is_safepoint(&self) -> bool {
        matches!(
            self,
            Self::SoftwareTrap { .. } | Self::Call { .. } | Self::CallInd { .. }
        )
    }
    fn function_alignment() -> FunctionAlignment {
        FunctionAlignment {
            minimum: 4,
            preferred: 4,
        }
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

    fn emit(&self, code: &mut MachBuffer<Inst>, _info: &Self::Info, state: &mut EmitState) {
        let start = code.cur_offset();
        match self {
            Self::Args { .. } | Self::Rets { .. } | Self::DummyUse { .. } => {}
            Self::Nop => put_word(code, encode::NOP),
            Self::SRead { dst, selector } => put_word(
                code,
                encode::sread(arch_reg(dst.to_reg()), *selector).expect("validated SREAD selector"),
            ),
            Self::ReadFixedGpr { dst, index } => {
                let dst = arch_reg(dst.to_reg());
                let fixed = regs::Reg::new(*index).expect("validated fixed GPR index");
                if dst != fixed {
                    put_word(code, encode::mov(dst, fixed));
                }
            }
            Self::WriteFixedGpr { src, index } => {
                let src = arch_reg(*src);
                let fixed = regs::Reg::new(*index).expect("validated fixed GPR index");
                if src != fixed {
                    put_word(code, encode::mov(fixed, src));
                }
            }
            Self::SWrite { src, selector } => put_word(
                code,
                encode::swrite(arch_reg(*src), *selector).expect("validated SWRITE selector"),
            ),
            Self::SSwapScratch { dst, src } => {
                let dst = arch_reg(dst.to_reg());
                let src = arch_reg(*src);
                if dst != src {
                    put_word(code, encode::mov(dst, src));
                }
                put_word(code, encode::sswap_scratch(dst));
            }
            Self::SRet => put_word(code, encode::sret()),
            Self::SRetCtx { src } => put_word(code, encode::sretctx(arch_reg(*src))),
            Self::TlbFence => put_word(code, encode::tlbfence()),
            Self::TlbFenceVa { src } => put_word(code, encode::tlbfence_va(arch_reg(*src))),
            Self::TlbFenceAsid { src } => put_word(code, encode::tlbfence_asid(arch_reg(*src))),
            Self::Wfi => put_word(code, encode::wfi()),
            Self::SyncI => put_word(code, encode::sync_i()),
            Self::SoftwareTrap { code: trap_code } => put_word(
                code,
                encode::trap(*trap_code).expect("backend trap code must be encodable"),
            ),
            Self::TrapIfNz { test, code: _ } => {
                let trap = code.get_label();
                let done = code.get_label();
                let cond_at = code.cur_offset();
                code.use_label_at_offset(cond_at, trap, LabelUse::Cond7);
                put_word(code, encode::bnz(arch_reg(*test), 0).unwrap());
                let skip_at = code.cur_offset();
                code.use_label_at_offset(skip_at, done, LabelUse::Branch11);
                code.add_uncond_branch(skip_at, skip_at + 2, done);
                put_word(code, encode::b(0).unwrap());
                code.bind_label(trap, state.ctrl_plane_mut());
                put_word(code, encode::trap(0).unwrap());
                code.bind_label(done, state.ctrl_plane_mut());
            }
            Self::TrapIfZ { test, code: _ } => {
                let done = code.get_label();
                let branch_at = code.cur_offset();
                code.use_label_at_offset(branch_at, done, LabelUse::Cond7);
                put_word(code, encode::bnz(arch_reg(*test), 0).unwrap());
                put_word(code, encode::trap(0).unwrap());
                code.bind_label(done, state.ctrl_plane_mut());
            }
            Self::Mov { dst, src } => {
                put_word(code, encode::mov(arch_reg(dst.to_reg()), arch_reg(*src)))
            }
            Self::Add { dst, lhs, rhs } => put_word(
                code,
                encode::add(arch_reg(dst.to_reg()), arch_reg(*lhs), arch_reg(*rhs)),
            ),
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
                if d != l {
                    put_word(code, encode::mov(d, l));
                }
                let word = match op {
                    TwoOp::Sub => encode::sub(d, r),
                    TwoOp::Addo => encode::addo(d, r),
                    TwoOp::Subo => encode::subo(d, r),
                    TwoOp::CmpEq => encode::cmpeq(d, r),
                    TwoOp::CmpLt => encode::cmplt(d, r),
                    TwoOp::CmpLtu => encode::cmpltu(d, r),
                    TwoOp::Min => encode::min(d, r),
                    TwoOp::MinU => encode::minu(d, r),
                    TwoOp::Max => encode::max(d, r),
                    TwoOp::MaxU => encode::maxu(d, r),
                    TwoOp::And => encode::and(d, r),
                    TwoOp::Or => encode::or(d, r),
                    TwoOp::Xor => encode::xor(d, r),
                    TwoOp::Shl => encode::shl(d, r),
                    TwoOp::Shr => encode::shr(d, r),
                    TwoOp::Sar => encode::sar(d, r),
                    TwoOp::BSet => encode::bset(d, r),
                    TwoOp::BClr => encode::bclr(d, r),
                    TwoOp::BInv => encode::binv(d, r),
                    TwoOp::BExt => encode::bext(d, r),
                    TwoOp::Mul => encode::mul(d, r),
                    TwoOp::Div => encode::div(d, r),
                    TwoOp::DivU => encode::divu(d, r),
                    TwoOp::Rem => encode::rem(d, r),
                    TwoOp::RemU => encode::remu(d, r),
                    TwoOp::Rev8 => encode::rev8(d, r),
                };
                put_word(code, word);
            }
            Self::Icmp { dst, cc, lhs, rhs } => {
                let d = arch_reg(dst.to_reg());
                let l = arch_reg(*lhs);
                let r = arch_reg(*rhs);
                if d != l {
                    put_word(code, encode::mov(d, l));
                }
                match cc {
                    IntCC::Equal => put_word(code, encode::cmpeq(d, r)),
                    IntCC::NotEqual => {
                        put_word(code, encode::cmpeq(d, r));
                        put_word(
                            code,
                            encode::li(crate::isa::sia32::regs::Reg::SCRATCH, 1)
                                .expect("one fits LI"),
                        );
                        put_word(code, encode::xor(d, crate::isa::sia32::regs::Reg::SCRATCH));
                    }
                    IntCC::SignedLessThan => put_word(code, encode::cmplt(d, r)),
                    IntCC::UnsignedLessThan => put_word(code, encode::cmpltu(d, r)),
                    IntCC::SignedGreaterThan => {
                        let lhs = if d == l && d != r {
                            put_word(code, encode::mov(crate::isa::sia32::regs::Reg::SCRATCH, l));
                            crate::isa::sia32::regs::Reg::SCRATCH
                        } else {
                            l
                        };
                        if d != r {
                            put_word(code, encode::mov(d, r));
                        }
                        put_word(code, encode::cmplt(d, lhs));
                    }
                    IntCC::UnsignedGreaterThan => {
                        let lhs = if d == l && d != r {
                            put_word(code, encode::mov(crate::isa::sia32::regs::Reg::SCRATCH, l));
                            crate::isa::sia32::regs::Reg::SCRATCH
                        } else {
                            l
                        };
                        if d != r {
                            put_word(code, encode::mov(d, r));
                        }
                        put_word(code, encode::cmpltu(d, lhs));
                    }
                    IntCC::SignedLessThanOrEqual => {
                        let lhs = if d == l && d != r {
                            put_word(code, encode::mov(crate::isa::sia32::regs::Reg::SCRATCH, l));
                            crate::isa::sia32::regs::Reg::SCRATCH
                        } else {
                            l
                        };
                        if d != r {
                            put_word(code, encode::mov(d, r));
                        }
                        put_word(code, encode::cmplt(d, lhs));
                        put_word(
                            code,
                            encode::li(crate::isa::sia32::regs::Reg::SCRATCH, 1)
                                .expect("one fits LI"),
                        );
                        put_word(code, encode::xor(d, crate::isa::sia32::regs::Reg::SCRATCH));
                    }
                    IntCC::UnsignedLessThanOrEqual => {
                        let lhs = if d == l && d != r {
                            put_word(code, encode::mov(crate::isa::sia32::regs::Reg::SCRATCH, l));
                            crate::isa::sia32::regs::Reg::SCRATCH
                        } else {
                            l
                        };
                        if d != r {
                            put_word(code, encode::mov(d, r));
                        }
                        put_word(code, encode::cmpltu(d, lhs));
                        put_word(
                            code,
                            encode::li(crate::isa::sia32::regs::Reg::SCRATCH, 1)
                                .expect("one fits LI"),
                        );
                        put_word(code, encode::xor(d, crate::isa::sia32::regs::Reg::SCRATCH));
                    }
                    IntCC::SignedGreaterThanOrEqual => {
                        put_word(code, encode::cmplt(d, r));
                        put_word(
                            code,
                            encode::li(crate::isa::sia32::regs::Reg::SCRATCH, 1)
                                .expect("one fits LI"),
                        );
                        put_word(code, encode::xor(d, crate::isa::sia32::regs::Reg::SCRATCH));
                    }
                    IntCC::UnsignedGreaterThanOrEqual => {
                        put_word(code, encode::cmpltu(d, r));
                        put_word(
                            code,
                            encode::li(crate::isa::sia32::regs::Reg::SCRATCH, 1)
                                .expect("one fits LI"),
                        );
                        put_word(code, encode::xor(d, crate::isa::sia32::regs::Reg::SCRATCH));
                    }
                }
            }
            Self::ShiftImm {
                op,
                dst,
                src,
                amount,
            } => {
                let d = arch_reg(dst.to_reg());
                let s = arch_reg(*src);
                if d != s {
                    put_word(code, encode::mov(d, s));
                }
                let word = match op {
                    TwoOp::Shl => encode::shli(d, *amount),
                    TwoOp::Shr => encode::shri(d, *amount),
                    TwoOp::Sar => encode::sari(d, *amount),
                    _ => panic!("invalid SIA32 immediate shift"),
                }
                .expect("shift amount validated");
                put_word(code, word);
            }
            Self::Li7 { dst, imm } => put_word(
                code,
                encode::li(arch_reg(dst.to_reg()), i32::from(*imm)).unwrap(),
            ),
            Self::Addi7 { dst, src, imm } => {
                let d = arch_reg(dst.to_reg());
                let s = arch_reg(*src);
                if d != s {
                    put_word(code, encode::mov(d, s));
                }
                put_word(code, encode::addi(d, i32::from(*imm)).unwrap());
            }
            Self::LoadConst32 { dst, value } => {
                if const_digits(*value).len() <= 2 {
                    emit_const32(code, arch_reg(dst.to_reg()), *value);
                } else {
                    emit_literal32(code, state, arch_reg(dst.to_reg()), *value);
                }
            }
            Self::LoadExtName { dst, name, offset } => {
                emit_ext_name32(code, state, arch_reg(dst.to_reg()), name, *offset)
            }
            Self::Load { op, dst, base } => {
                emit_load_zero_offset(code, *op, arch_reg(dst.to_reg()), arch_reg(*base))
            }
            Self::Store { op, src, base } => {
                emit_store_zero_offset(code, *op, arch_reg(*src), arch_reg(*base))
            }
            Self::IndexedLoad { dst, base, index } => put_word(
                code,
                encode::lda_w(arch_reg(dst.to_reg()), arch_reg(*base), arch_reg(*index)),
            ),
            Self::IndexedStore { src, base, index } => put_word(
                code,
                encode::sta_w(arch_reg(*src), arch_reg(*base), arch_reg(*index)),
            ),
            Self::Fence => put_word(code, encode::fence()),

            Self::LoadStack { dst, mem, ty } => {
                let off = frame_stack_offset(mem, &state.frame_layout);
                emit_load_base_offset(
                    code,
                    load_op(*ty),
                    arch_reg(dst.to_reg()),
                    regs::Reg::FP,
                    off,
                );
            }
            Self::StoreStack { src, mem, ty } => {
                let off = frame_stack_offset(mem, &state.frame_layout);
                emit_store_base_offset(code, store_op(*ty), arch_reg(*src), regs::Reg::FP, off);
            }
            Self::StackAddr { dst, mem } => {
                let off = frame_stack_offset(mem, &state.frame_layout);
                emit_add_imm32(code, arch_reg(dst.to_reg()), regs::Reg::FP, off);
            }
            Self::LoadBaseOffset {
                dst,
                base,
                offset,
                ty,
            } => emit_load_base_offset(
                code,
                load_op(*ty),
                arch_reg(dst.to_reg()),
                arch_reg(*base),
                *offset,
            ),
            Self::StoreBaseOffset {
                src,
                base,
                offset,
                ty,
            } => emit_store_base_offset(
                code,
                store_op(*ty),
                arch_reg(*src),
                arch_reg(*base),
                *offset,
            ),
            Self::Extend {
                dst,
                src,
                signed,
                from_bits,
                to_bits,
            } => {
                assert!(*from_bits <= *to_bits && *to_bits <= 32 && *from_bits > 0);
                let d = arch_reg(dst.to_reg());
                let s = arch_reg(*src);
                if d != s {
                    put_word(code, encode::mov(d, s));
                }
                if from_bits < to_bits {
                    let shift = 32 - *from_bits;
                    if shift != 0 {
                        put_word(code, encode::shli(d, shift).unwrap());
                        put_word(
                            code,
                            if *signed {
                                encode::sari(d, shift).unwrap()
                            } else {
                                encode::shri(d, shift).unwrap()
                            },
                        );
                    }
                }
            }
            Self::AddImm { dst, src, imm } => {
                emit_add_imm32(code, arch_reg(dst.to_reg()), arch_reg(*src), *imm)
            }
            Self::SpAdjust { amount } => {
                emit_add_imm32(code, regs::Reg::SP, regs::Reg::SP, *amount)
            }
            Self::StackLowerBoundTrap { .. } => {
                panic!("SIA32 stack probing is not implemented by the frozen ABI")
            }

            Self::Jump { target } => {
                code.use_label_at_offset(start, *target, LabelUse::Branch11);
                code.add_uncond_branch(start, start + 2, *target);
                put_word(code, encode::b(0).unwrap());
            }
            Self::BrNz {
                test,
                taken,
                not_taken,
            } => {
                code.use_label_at_offset(start, *taken, LabelUse::Cond7);
                put_word(code, encode::bnz(arch_reg(*test), 0).unwrap());
                let second = code.cur_offset();
                code.use_label_at_offset(second, *not_taken, LabelUse::Branch11);
                code.add_uncond_branch(second, second + 2, *not_taken);
                put_word(code, encode::b(0).unwrap());
            }
            Self::Call { info } => emit_direct_call(code, state, info),
            Self::CallInd { info } => {
                put_word(code, encode::callr(arch_reg(info.dest)));
                record_call(code, state, info);
            }
            Self::Ret => put_word(code, encode::ret()),
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
    use crate::ir::types::{F32, I128};
    use crate::isa::{CallConv, sia32::regs::mach_reg};

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
        assert_eq!(MachInst::is_move(&mov), Some((w(1), mach_reg(2))));
        assert!(!mov.is_mem_access());
        let load = Inst::Load {
            op: LoadOp::I32,
            dst: w(1),
            base: mach_reg(2),
        };
        assert!(load.is_mem_access());
    }
    #[test]
    fn arbitrary_i32_constants_have_bounded_exact_reconstruction() {
        for value in [
            0u32,
            1,
            63,
            64,
            127,
            128,
            0x7fff_ffff,
            0x8000_0000,
            0xffff_ffff,
            0xdead_beef,
        ] {
            let digits = const_digits(value);
            assert!(!digits.is_empty());
            assert!(digits.len() <= 5);
            assert!(digits.iter().all(|d| (-64..=63).contains(d)));
            let mut got = digits[0] as i32 as u32;
            for &d in &digits[1..] {
                got = got.wrapping_shl(7).wrapping_add(d as i32 as u32);
            }
            assert_eq!(got, value, "digits={digits:?}");
        }
    }
    #[test]
    fn literal_pool_threshold_keeps_small_values_inline() {
        assert!(const_digits(63).len() <= 2);
        assert!(const_digits(128).len() <= 2);
        assert!(const_digits(0xdead_beef).len() > 2);
    }
    #[test]
    fn function_alignment_preserves_ldpc_word_phase() {
        let a = Inst::function_alignment();
        assert_eq!(a.minimum, 4);
        assert_eq!(a.preferred, 4);
    }
    #[test]
    fn indirect_call_is_a_real_regular_call() {
        let info = CallInfo::empty(mach_reg(3), CallConv::SystemV);
        let call = Inst::CallInd {
            info: Box::new(info),
        };
        assert_eq!(call.call_type(), CallType::Regular);
        assert!(call.is_safepoint());
        assert_eq!(call.encoded_worst_case_size(), 2);
    }
    #[test]
    fn pseudos_report_conservative_sizes() {
        let op = Inst::TwoOp {
            op: TwoOp::Sub,
            dst: w(1),
            lhs: mach_reg(2),
            rhs: mach_reg(3),
        };
        assert_eq!(op.encoded_worst_case_size(), 4);
        let c = Inst::LoadConst32 {
            dst: w(1),
            value: 0xdead_beef,
        };
        assert_eq!(c.encoded_worst_case_size(), 18);
    }
    #[test]
    fn trap_and_nop_bytes_are_exact() {
        assert_eq!(Inst::TRAP_OPCODE, &[0x0f, 0xc0]);
        assert_eq!(Inst::gen_nop_units(), vec![vec![0xff, 0xcf]]);
    }
}
