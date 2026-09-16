//! Initial Cranelift ABI implementation for SIA32.
//!
//! This implements the frozen integer ABI and deliberately rejects or leaves
//! unreachable features that are outside the initial contract (by-value struct
//! ABI synthesis, stack probing, tail calls, floating point and vectors).

use super::inst::{Inst, LoadOp, StoreOp};
use super::regs;
use super::settings::Flags as SiaFlags;
use crate::ir::types::{I8, I16, I32, I64};
use crate::ir::{self, Signature, Type};
use crate::isa;
use crate::machinst::{
    ABIArg, ABIArgLocation, ABIArgSlot, ABIArgSlotVec, ABIMachineSpec, ArgPair,
    ArgsAccumulator, ArgsOrRets, FrameLayout, FunctionCalls, IsaFlags, RealReg, Reg, RetPair,
    SmallInstVec, StackAMode, Writable,
};
use crate::settings;
use crate::{CodegenError, CodegenResult};
use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use regalloc2::{MachineEnv, PRegSet, RegClass};
use smallvec::{SmallVec, smallvec};

pub(crate) type Sia32Callee = crate::machinst::Callee<Sia32MachineDeps>;

#[derive(Clone, Debug)]
pub(crate) struct Sia32MachineDeps;

impl IsaFlags for SiaFlags {}

const fn align_to(value: u32, align: u32) -> u32 {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

fn ensure_call_conv(call_conv: isa::CallConv) -> CodegenResult<()> {
    if call_conv == isa::CallConv::SystemV {
        Ok(())
    } else {
        Err(CodegenError::Unsupported(
            format!("SIA32 currently supports only SystemV calling convention, got {call_conv:?}")
                .into(),
        ))
    }
}

fn abi_type(ty: Type) -> CodegenResult<(u32, u32)> {
    if ty == I64 {
        Ok((8, 8))
    } else if ty.is_int() && ty.bits() <= 32 {
        Ok((4, 4))
    } else {
        Err(CodegenError::Unsupported(
            format!("unsupported SIA32 ABI type {ty}").into(),
        ))
    }
}

fn phys_reg(n: u8) -> Reg {
    regs::mach_reg(n)
}

fn writable_phys_reg(n: u8) -> Writable<Reg> {
    Writable::from_reg(phys_reg(n))
}

fn arg_reg(index: usize) -> Option<Reg> {
    regs::ARG_REGS.get(index).map(|r| phys_reg(r.index()))
}

fn return_reg(index: usize) -> Option<Reg> {
    match index {
        0 => Some(phys_reg(regs::RETURN_LOW.index())),
        1 => Some(phys_reg(regs::RETURN_HIGH.index())),
        2 => Some(phys_reg(3)),
        3 => Some(phys_reg(4)),
        4 => Some(phys_reg(5)),
        5 => Some(phys_reg(6)),
        _ => None,
    }
}

impl ABIMachineSpec for Sia32MachineDeps {
    type I = Inst;
    type F = SiaFlags;

    const WORD_BITS: u32 = 32;
    const STACK_ALIGN: u32 = 8;
    const STACK_ARG_RET_SIZE_LIMIT: u32 = 128 * 1024;
    const TRAP_CODE_STACK_OVERFLOW: ir::TrapCode = ir::TrapCode::STACK_OVERFLOW;

    fn compute_arg_locs(
        call_conv: isa::CallConv,
        _flags: &settings::Flags,
        params: &[ir::AbiParam],
        args_or_rets: ArgsOrRets,
        add_ret_area_ptr: bool,
        mut args: ArgsAccumulator,
    ) -> CodegenResult<(u32, Option<usize>)> {
        ensure_call_conv(call_conv)?;

        let mut reg_index = 0usize;
        let mut stack_offset = 0u32;
        let max_regs = 6usize;

        for param in params {
            let ty = param.value_type;
            let (size, align) = abi_type(ty)?;
            let parts = if ty == I64 { 2 } else { 1 };

            if parts == 2 && reg_index % 2 != 0 {
                reg_index += 1;
            }

            let can_use_regs = reg_index + parts <= max_regs;
            if can_use_regs {
                let mut slots = ABIArgSlotVec::new();
                for part in 0..parts {
                    let reg = match args_or_rets {
                        ArgsOrRets::Args => arg_reg(reg_index + part),
                        ArgsOrRets::Rets => return_reg(reg_index + part),
                    }
                    .expect("register index checked against SIA32 ABI register count");
                    slots.push(ABIArgSlot::Reg {
                        reg,
                        ty: I32,
                        extension: param.extension,
                    });
                }
                args.push(ABIArg::Slots { slots, purpose: param.purpose });
                reg_index += parts;
            } else {
                stack_offset = align_to(stack_offset, align);
                args.push(ABIArg::Stack {
                    offset: stack_offset as i64,
                    ty,
                    extension: param.extension,
                    purpose: param.purpose,
                });
                stack_offset += size;
            }
        }

        let ret_area_arg = if add_ret_area_ptr {
            let reg = if reg_index < max_regs {
                let reg = arg_reg(reg_index).unwrap();
                reg_index += 1;
                let mut slots = ABIArgSlotVec::new();
                slots.push(ABIArgSlot::Reg {
                    reg,
                    ty: I32,
                    extension: ir::ArgumentExtension::None,
                });
                args.push(ABIArg::Slots {
                    slots,
                    purpose: ir::ArgumentPurpose::Normal,
                });
                args.len().checked_sub(1)
            } else {
                stack_offset = align_to(stack_offset, 4);
                args.push(ABIArg::Stack {
                    offset: stack_offset as i64,
                    ty: I32,
                    extension: ir::ArgumentExtension::None,
                    purpose: ir::ArgumentPurpose::Normal,
                });
                stack_offset += 4;
                args.len().checked_sub(1)
            };
            reg
        } else {
            None
        };

        Ok((align_to(stack_offset, Self::STACK_ALIGN), ret_area_arg))
    }

    fn gen_load_stack(mem: StackAMode, ty: Type, dst: Writable<Reg>) -> Inst {
        Inst::LoadStack { dst, mem, ty }
    }

    fn gen_store_stack(mem: StackAMode, ty: Type, src: Reg) -> Inst {
        Inst::StoreStack { src, mem, ty }
    }

    fn gen_move(dst: Writable<Reg>, src: Reg, ty: Type) -> Inst {
        Inst::gen_move(dst, src, ty)
    }

    fn gen_extend(
        dst: Writable<Reg>,
        src: Reg,
        from_bits: u8,
        to_bits: u8,
        signed: bool,
    ) -> Inst {
        Inst::Extend { dst, src, signed, from_bits, to_bits }
    }

    fn gen_args(args: Vec<ArgPair>) -> Inst {
        Inst::Args { args }
    }

    fn gen_rets(rets: Vec<RetPair>) -> Inst {
        Inst::Rets { rets }
    }

    fn gen_add_imm(dst: Writable<Reg>, src: Reg, imm: i64) -> SmallInstVec<Inst> {
        smallvec![Inst::AddImm {
            dst,
            src,
            imm: i32::try_from(imm).expect("SIA32 stack immediate must fit i32"),
        }]
    }

    fn get_stacklimit_reg(_call_conv: isa::CallConv) -> Reg {
        regs::stacklimit_reg()
    }

    fn get_stacklimit_reg_checked(call_conv: isa::CallConv) -> Option<Reg> {
        if call_conv == isa::CallConv::SystemV {
            Some(regs::stacklimit_reg())
        } else {
            None
        }
    }

    fn get_frame_pointer_reg() -> Reg {
        regs::fp_reg()
    }

    fn get_stack_pointer_reg() -> Reg {
        regs::stack_reg()
    }

    fn get_link_reg() -> Option<Reg> {
        Some(regs::link_reg())
    }

    fn get_pinned_reg() -> Option<Reg> {
        None
    }

    fn get_machine_env(_flags: &settings::Flags, _isa_flags: &Self::F) -> &MachineEnv {
        &regs::MACHINE_ENV
    }

    fn get_regs_clobbered_by_call(
        _call_conv_of_callee: isa::CallConv,
        _call_conv_of_caller: isa::CallConv,
    ) -> PRegSet {
        let mut set = PRegSet::empty();
        for r in regs::CALLER_SAVED {
            set.add(regs::preg(r.index()));
        }
        set.add(regs::preg(regs::Reg::SCRATCH.index()));
        set.add(regs::preg(regs::Reg::LR.index()));
        set
    }

    fn get_ext_mode(
        _call_conv: isa::CallConv,
        _specified: ir::ArgumentExtension,
    ) -> ir::ArgumentExtension {
        ir::ArgumentExtension::None
    }

    fn compute_frame_layout(
        call_conv: isa::CallConv,
        flags: &settings::Flags,
        sig: &Signature,
        regs: &[RealReg],
        function_calls: FunctionCalls,
        fixed_frame_storage_size: u32,
        outgoing_args_size: u32,
        clobber_size: u32,
        _isa_flags: &Self::F,
    ) -> FrameLayout {
        ensure_call_conv(call_conv).expect("call convention validated before frame-layout computation");

        let mut clobbered_callee_saves = Vec::new();
        let mut save_area = 0u32;
        for &real in regs {
            let hw = real.hw_enc();
            if matches!(hw, 9..=11 | 15) {
                save_area += 4;
                clobbered_callee_saves.push(real);
            }
        }

        let setup_area_size = if function_calls != FunctionCalls::None { 4 } else { 0 };
        let raw_frame = fixed_frame_storage_size
            .saturating_add(clobber_size)
            .saturating_add(save_area)
            .saturating_add(setup_area_size);
        let frame_size = align_to(raw_frame, Self::STACK_ALIGN);

        FrameLayout {
            word_bytes: 4,
            incoming_args_size: 0,
            setup_area_size,
            tail_args_size: 0,
            clobber_size: clobber_size + save_area,
            fixed_frame_storage_size,
            outgoing_args_size,
            stack_size: frame_size,
            stack_align: Self::STACK_ALIGN,
            clobbered_callee_saves,
        }
    }

    fn gen_prologue_frame_setup(
        flags: &settings::Flags,
        frame_layout: &FrameLayout,
    ) -> SmallInstVec<Inst> {
        let mut insts = SmallInstVec::new();
        if frame_layout.stack_size != 0 {
            insts.push(Inst::SpAdjust { amount: -(frame_layout.stack_size as i32) });
        }

        let mut offset = frame_layout.fixed_frame_storage_size + frame_layout.clobber_size;
        for real in &frame_layout.clobbered_callee_saves {
            offset = offset.saturating_sub(4);
            insts.push(Inst::StoreBaseOffset {
                src: Reg::from(*real),
                base: regs::stack_reg(),
                offset: offset as i32,
                ty: I32,
            });
        }

        if frame_layout.setup_area_size != 0 {
            insts.push(Inst::StoreBaseOffset {
                src: regs::link_reg(),
                base: regs::stack_reg(),
                offset: (frame_layout.stack_size - 4) as i32,
                ty: I32,
            });
        }

        if flags.enable_probestack() {
            insts.push(Inst::StackLowerBoundTrap { limit: regs::stacklimit_reg() });
        }

        insts
    }

    fn gen_epilogue_frame_restore(
        _flags: &settings::Flags,
        frame_layout: &FrameLayout,
    ) -> SmallInstVec<Inst> {
        let mut insts = SmallInstVec::new();

        if frame_layout.setup_area_size != 0 {
            insts.push(Inst::LoadBaseOffset {
                dst: regs::writable_link_reg(),
                base: regs::stack_reg(),
                offset: (frame_layout.stack_size - 4) as i32,
                ty: I32,
            });
        }

        let mut offset = frame_layout.fixed_frame_storage_size + frame_layout.clobber_size;
        for real in frame_layout.clobbered_callee_saves.iter().rev() {
            offset = offset.saturating_sub(4);
            insts.push(Inst::LoadBaseOffset {
                dst: Writable::from_reg(Reg::from(*real)),
                base: regs::stack_reg(),
                offset: offset as i32,
                ty: I32,
            });
        }

        if frame_layout.stack_size != 0 {
            insts.push(Inst::SpAdjust { amount: frame_layout.stack_size as i32 });
        }
        insts
    }

    fn gen_return(_call_conv: isa::CallConv) -> Inst {
        Inst::Ret
    }

    fn gen_stack_lower_bound_trap(limit: Reg) -> SmallInstVec<Inst> {
        smallvec![Inst::StackLowerBoundTrap { limit }]
    }

    fn gen_memcpy(
        _call_conv: isa::CallConv,
        _dst: Reg,
        _src: Reg,
        _size: usize,
    ) -> SmallInstVec<Inst> {
        panic!("SIA32 ABI memcpy synthesis is not implemented")
    }

    fn gen_inline_probestack(_frame_size: u32, _guard_size: u32) -> SmallInstVec<Inst> {
        panic!("SIA32 inline probestack is not implemented")
    }

    fn is_caller_save(reg: RealReg) -> bool {
        matches!(reg.hw_enc(), 1..=8)
    }

    fn retval_temp_reg() -> Writable<Reg> {
        writable_phys_reg(12)
    }

    fn spilltmp_reg() -> Writable<Reg> {
        writable_phys_reg(12)
    }
}

// Inherent helpers keep this backend independent of trait-import scope at call
// sites while still delegating to the Cranelift MachInst implementation.
impl Inst {
    pub(crate) fn sia_gen_move(dst: Writable<Reg>, src: Reg, ty: Type) -> Self {
        <Self as crate::machinst::MachInst>::gen_move(dst, src, ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(ty: Type) -> ir::AbiParam { ir::AbiParam::new(ty) }

    #[test]
    fn machine_env_excludes_fixed_registers() {
        let env = Sia32MachineDeps::get_machine_env(
            &settings::Flags::new(settings::builder()),
            &SiaFlags::new(&settings::Flags::new(settings::builder()), &settings::builder()),
        );
        for fixed in [0u8, 12, 13, 14] {
            let preg = regs::preg(fixed);
            assert!(!env.preferred_regs_by_class[0].contains(preg));
            assert!(!env.non_preferred_regs_by_class[0].contains(preg));
        }
        for alloc in [1u8, 8, 9, 11, 15] {
            let preg = regs::preg(alloc);
            assert!(
                env.preferred_regs_by_class[0].contains(preg)
                    || env.non_preferred_regs_by_class[0].contains(preg)
            );
        }
    }

    #[test]
    fn call_clobbers_match_caller_saved_and_reserved_scratch() {
        let set = Sia32MachineDeps::get_regs_clobbered_by_call(
            isa::CallConv::SystemV,
            isa::CallConv::SystemV,
        );
        for r in 1u8..=8 {
            assert!(set.contains(regs::preg(r)));
        }
        assert!(set.contains(regs::preg(12)));
        assert!(set.contains(regs::preg(14)));
        for r in [9u8, 10, 11, 15] {
            assert!(!set.contains(regs::preg(r)));
        }
    }

    #[test]
    fn scalar_and_pair_arguments_obey_register_contract() {
        let flags = settings::Flags::new(settings::builder());
        let mut storage = Vec::new();
        let params = [p(I32), p(I64), p(I32)];
        let (stack, _) = Sia32MachineDeps::compute_arg_locs(
            isa::CallConv::SystemV,
            &flags,
            &params,
            ArgsOrRets::Args,
            false,
            ArgsAccumulator::new(&mut storage),
        ).unwrap();
        assert_eq!(stack, 0);
    }
}
