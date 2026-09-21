//! Initial Cranelift ABI implementation for SIA32.
//!
//! This implements the frozen integer ABI and deliberately rejects or leaves
//! unreachable features that are outside the initial contract (by-value struct
//! ABI synthesis, stack probing, tail calls, floating point and vectors).

use super::inst::Inst;
use super::regs;
use super::settings::Flags as SiaFlags;
use crate::ir::types::I32;
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
            alloc::format!("SIA32 initially supports only the SystemV compiler ABI, not {call_conv}")
        ))
    }
}

fn preg_real(index: u8) -> RealReg {
    RealReg::from(regs::preg(index))
}

fn slot_extension(param: &ir::AbiParam, location: ABIArgLocation) -> ir::ArgumentExtension {
    Sia32MachineDeps::get_ext_mode(isa::CallConv::SystemV, param.extension, location)
}

impl ABIMachineSpec for Sia32MachineDeps {
    type I = Inst;
    type F = SiaFlags;

    const STACK_ARG_RET_SIZE_LIMIT: u32 = 128 * 1024 * 1024;

    fn word_bits() -> u32 { 32 }

    fn stack_align(call_conv: isa::CallConv) -> u32 {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        8
    }

    fn compute_arg_locs(
        call_conv: isa::CallConv,
        flags: &settings::Flags,
        params: &[ir::AbiParam],
        args_or_rets: ArgsOrRets,
        add_ret_area_ptr: bool,
        mut args: ArgsAccumulator,
    ) -> CodegenResult<(u32, Option<usize>)> {
        ensure_call_conv(call_conv)?;
        if add_ret_area_ptr && args_or_rets != ArgsOrRets::Args {
            return Err(CodegenError::Unsupported(
                "SIA32 return-area pointer is valid only for arguments".to_owned(),
            ));
        }

        let reg_last = match args_or_rets {
            ArgsOrRets::Args => 6u8,
            ArgsOrRets::Rets => 2u8,
        };
        let mut next_reg = if add_ret_area_ptr { 2u8 } else { 1u8 };
        let mut stack_only = false;
        let mut next_stack = 0u32;

        let ret_area_ptr = add_ret_area_ptr.then(|| {
            ABIArg::reg(
                preg_real(1),
                I32,
                ir::ArgumentExtension::None,
                ir::ArgumentPurpose::Normal,
            )
        });

        for param in params {
            if matches!(param.purpose, ir::ArgumentPurpose::StructArgument(_)) {
                return Err(CodegenError::Unsupported(
                    "SIA32 passes aggregates by address; lower StructArgument to an explicit pointer"
                        .to_owned(),
                ));
            }

            let (rcs, reg_tys) = Inst::rc_for_type(&param.value_type)?;
            debug_assert!(rcs.iter().all(|rc| *rc == RegClass::Int));
            let parts = reg_tys.len() as u8;
            debug_assert!(parts == 1 || parts == 2);

            if parts == 2 && next_reg % 2 == 0 {
                next_reg = next_reg.saturating_add(1);
            }

            let fits_regs = !stack_only && next_reg <= reg_last && next_reg + parts - 1 <= reg_last;
            let mut slots = ABIArgSlotVec::new();

            if fits_regs {
                for (part, ty) in reg_tys.iter().enumerate() {
                    slots.push(ABIArgSlot::Reg {
                        reg: preg_real(next_reg + part as u8),
                        ty: *ty,
                        extension: if parts == 1 {
                            slot_extension(param, ABIArgLocation::Reg)
                        } else {
                            ir::ArgumentExtension::None
                        },
                    });
                }
                next_reg += parts;
            } else {
                if args_or_rets == ArgsOrRets::Rets && !flags.enable_multi_ret_implicit_sret() {
                    return Err(CodegenError::Unsupported(
                        "SIA32 return values exceed r1:r2; enable implicit sret or use StructReturn"
                            .to_owned(),
                    ));
                }
                if args_or_rets == ArgsOrRets::Args {
                    stack_only = true;
                }

                let value_align = if parts == 2 { 8 } else { 4 };
                next_stack = align_to(next_stack, value_align);
                for (part, ty) in reg_tys.iter().enumerate() {
                    slots.push(ABIArgSlot::Stack {
                        offset: i64::from(next_stack + (part as u32 * 4)),
                        ty: *ty,
                        extension: if parts == 1 {
                            slot_extension(param, ABIArgLocation::Stack)
                        } else {
                            ir::ArgumentExtension::None
                        },
                    });
                }
                next_stack += if parts == 2 { 8 } else { 4 };
            }

            args.push(ABIArg::Slots { slots, purpose: param.purpose });
        }

        let ret_area_pos = if let Some(ret_area_ptr) = ret_area_ptr {
            args.push_non_formal(ret_area_ptr);
            Some(args.args().len() - 1)
        } else {
            None
        };

        Ok((align_to(next_stack, 8), ret_area_pos))
    }

    fn gen_load_stack(mem: StackAMode, into_reg: Writable<Reg>, ty: Type) -> Inst {
        Inst::LoadStack { dst: into_reg, mem, ty }
    }

    fn gen_store_stack(mem: StackAMode, from_reg: Reg, ty: Type) -> Inst {
        Inst::StoreStack { src: from_reg, mem, ty }
    }

    fn gen_move(to_reg: Writable<Reg>, from_reg: Reg, ty: Type) -> Inst {
        Inst::gen_move(to_reg, from_reg, ty)
    }

    fn gen_extend(to_reg: Writable<Reg>, from_reg: Reg, signed: bool, from_bits: u8, to_bits: u8) -> Inst {
        Inst::Extend { dst: to_reg, src: from_reg, signed, from_bits, to_bits }
    }

    fn gen_args(args: Vec<ArgPair>) -> Inst { Inst::Args { args } }
    fn gen_rets(rets: Vec<RetPair>) -> Inst { Inst::Rets { rets } }

    fn gen_add_imm(call_conv: isa::CallConv, into_reg: Writable<Reg>, from_reg: Reg, imm: u32) -> SmallInstVec<Inst> {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        smallvec![Inst::AddImm {
            dst: into_reg,
            src: from_reg,
            imm: i32::try_from(imm).expect("SIA32 ABI add immediate must fit i32"),
        }]
    }

    fn gen_stack_lower_bound_trap(limit_reg: Reg) -> SmallInstVec<Inst> {
        smallvec![Inst::StackLowerBoundTrap { limit: limit_reg }]
    }

    fn gen_get_stack_addr(mem: StackAMode, into_reg: Writable<Reg>) -> Inst {
        Inst::StackAddr { dst: into_reg, mem }
    }

    fn get_stacklimit_reg(call_conv: isa::CallConv) -> Reg {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        regs::stacklimit_reg()
    }

    fn gen_load_base_offset(into_reg: Writable<Reg>, base: Reg, offset: i32, ty: Type) -> Inst {
        Inst::LoadBaseOffset { dst: into_reg, base, offset, ty }
    }

    fn gen_store_base_offset(base: Reg, offset: i32, from_reg: Reg, ty: Type) -> Inst {
        Inst::StoreBaseOffset { src: from_reg, base, offset, ty }
    }

    fn gen_sp_reg_adjust(amount: i32) -> SmallInstVec<Inst> {
        if amount == 0 { SmallInstVec::new() } else { smallvec![Inst::SpAdjust { amount }] }
    }

    fn compute_frame_layout(
        call_conv: isa::CallConv,
        _flags: &settings::Flags,
        _sig: &Signature,
        regs_written: &[Writable<RealReg>],
        function_calls: FunctionCalls,
        incoming_args_size: u32,
        tail_args_size: u32,
        stackslots_size: u32,
        fixed_frame_storage_size: u32,
        outgoing_args_size: u32,
    ) -> FrameLayout {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        let mut clobbered_callee_saves = regs_written
            .iter().copied()
            .filter(|reg| matches!(reg.to_reg().hw_enc(), 9..=11 | 15))
            .collect::<Vec<_>>();
        clobbered_callee_saves.sort_by_key(|reg| reg.to_reg().hw_enc());
        clobbered_callee_saves.dedup_by_key(|reg| reg.to_reg().hw_enc());

        let setup_area_size = if function_calls == FunctionCalls::Regular { 8 } else { 0 };
        let clobber_size = if clobbered_callee_saves.is_empty() { 0 } else { align_to(clobbered_callee_saves.len() as u32 * 4, 8) };

        FrameLayout {
            word_bytes: 4,
            incoming_args_size,
            tail_args_size,
            setup_area_size,
            clobber_size,
            fixed_frame_storage_size: align_to(fixed_frame_storage_size, 8),
            stackslots_size,
            outgoing_args_size: align_to(outgoing_args_size, 8),
            clobbered_callee_saves,
            function_calls,
        }
    }

    fn gen_prologue_frame_setup(call_conv: isa::CallConv, _flags: &settings::Flags, _isa_flags: &SiaFlags, frame_layout: &FrameLayout) -> SmallInstVec<Inst> {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        let mut out = SmallInstVec::new();
        if frame_layout.setup_area_size != 0 {
            debug_assert_eq!(frame_layout.setup_area_size, 8);
            out.push(Inst::SpAdjust { amount: -8 });
            out.push(Inst::StoreBaseOffset { src: regs::link_reg(), base: regs::stack_reg(), offset: 0, ty: I32 });
        }
        out
    }

    fn gen_epilogue_frame_restore(call_conv: isa::CallConv, _flags: &settings::Flags, _isa_flags: &SiaFlags, frame_layout: &FrameLayout) -> SmallInstVec<Inst> {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        let mut out = SmallInstVec::new();
        if frame_layout.setup_area_size != 0 {
            out.push(Inst::LoadBaseOffset { dst: regs::writable_link_reg(), base: regs::stack_reg(), offset: 0, ty: I32 });
            out.push(Inst::SpAdjust { amount: 8 });
        }
        out
    }

    fn gen_return(call_conv: isa::CallConv, _isa_flags: &SiaFlags, _frame_layout: &FrameLayout) -> SmallInstVec<Inst> {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        smallvec![Inst::Ret]
    }

    fn gen_probestack(_insts: &mut SmallInstVec<Inst>, _frame_size: u32) {
        panic!("SIA32 stack probing is not part of the initial ABI")
    }

    fn gen_inline_probestack(_insts: &mut SmallInstVec<Inst>, _call_conv: isa::CallConv, _frame_size: u32, _guard_size: u32) {
        panic!("SIA32 inline stack probing is not part of the initial ABI")
    }

    fn gen_clobber_save(call_conv: isa::CallConv, _flags: &settings::Flags, frame_layout: &FrameLayout) -> SmallVec<[Inst; 16]> {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        let mut out = SmallVec::new();
        let stack_size = frame_layout.clobber_size
            + frame_layout.fixed_frame_storage_size
            + frame_layout.outgoing_args_size;
        if stack_size == 0 { return out; }
        out.push(Inst::SpAdjust { amount: -(stack_size as i32) });
        // Keep callee saves above the fixed/outgoing frame so StackAMode::Slot
        // offsets remain based at the current SP and never address above the caller SP.
        let save_base = frame_layout.fixed_frame_storage_size + frame_layout.outgoing_args_size;
        for (i, reg) in frame_layout.clobbered_callee_saves.iter().enumerate() {
            out.push(Inst::StoreBaseOffset { src: Reg::from(reg.to_reg()), base: regs::stack_reg(), offset: (save_base as i32) + (i as i32) * 4, ty: I32 });
        }
        out
    }

    fn gen_clobber_restore(call_conv: isa::CallConv, _flags: &settings::Flags, frame_layout: &FrameLayout) -> SmallVec<[Inst; 16]> {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        let mut out = SmallVec::new();
        let stack_size = frame_layout.clobber_size
            + frame_layout.fixed_frame_storage_size
            + frame_layout.outgoing_args_size;
        if stack_size == 0 { return out; }
        let save_base = frame_layout.fixed_frame_storage_size + frame_layout.outgoing_args_size;
        for (i, reg) in frame_layout.clobbered_callee_saves.iter().enumerate() {
            out.push(Inst::LoadBaseOffset { dst: Writable::from_reg(Reg::from(reg.to_reg())), base: regs::stack_reg(), offset: (save_base as i32) + (i as i32) * 4, ty: I32 });
        }
        out.push(Inst::SpAdjust { amount: stack_size as i32 });
        out
    }

    fn gen_memcpy<F: FnMut(Type) -> Writable<Reg>>(_call_conv: isa::CallConv, _dst: Reg, _src: Reg, _size: usize, _alloc_tmp: F) -> SmallVec<[Inst; 8]> {
        panic!("SIA32 initial ABI lowers aggregates by address and does not synthesize memcpy")
    }

    fn get_number_of_spillslots_for_value(rc: RegClass, _target_vector_bytes: u32, _isa_flags: &SiaFlags) -> u32 {
        match rc {
            RegClass::Int => 1,
            RegClass::Float | RegClass::Vector => unreachable!("SIA32 has no FP/vector register class"),
        }
    }

    fn get_machine_env(_flags: &settings::Flags, call_conv: isa::CallConv) -> &MachineEnv {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        &regs::MACHINE_ENV
    }

    fn get_regs_clobbered_by_call(call_conv_of_callee: isa::CallConv, _is_exception: bool) -> PRegSet {
        ensure_call_conv(call_conv_of_callee).expect("unsupported SIA32 calling convention");
        PRegSet::empty()
            .with(regs::preg(1)).with(regs::preg(2)).with(regs::preg(3)).with(regs::preg(4))
            .with(regs::preg(5)).with(regs::preg(6)).with(regs::preg(7)).with(regs::preg(8))
            .with(regs::preg(12)).with(regs::preg(14))
    }

    fn get_ext_mode(call_conv: isa::CallConv, specified: ir::ArgumentExtension, _location: ABIArgLocation) -> ir::ArgumentExtension {
        ensure_call_conv(call_conv).expect("unsupported SIA32 calling convention");
        specified
    }

    fn retval_temp_reg(call_conv_of_callee: isa::CallConv) -> Writable<Reg> {
        ensure_call_conv(call_conv_of_callee).expect("unsupported SIA32 calling convention");
        regs::writable_scratch_reg()
    }

    fn exception_payload_regs(callee_conv: isa::CallConv) -> &'static [Reg] {
        ensure_call_conv(callee_conv).expect("unsupported SIA32 calling convention");
        static REGS: [Reg; 2] = [regs::mach_reg(1), regs::mach_reg(2)];
        &REGS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_abi_has_expected_word_and_stack_size() {
        assert_eq!(Sia32MachineDeps::word_bits(), 32);
        assert_eq!(Sia32MachineDeps::word_bytes(), 4);
        assert_eq!(Sia32MachineDeps::stack_align(isa::CallConv::SystemV), 8);
    }

    #[test]
    fn call_clobbers_match_frozen_abi_plus_reserved_temporaries() {
        let set = Sia32MachineDeps::get_regs_clobbered_by_call(isa::CallConv::SystemV, false);
        for n in 1..=8 { assert!(set.contains(regs::preg(n))); }
        assert!(set.contains(regs::preg(12)));
        assert!(set.contains(regs::preg(14)));
        for n in [9, 10, 11, 15] { assert!(!set.contains(regs::preg(n))); }
    }

    #[test]
    fn machine_environment_excludes_fixed_registers() {
        let flags = settings::Flags::new(settings::builder());
        let env = Sia32MachineDeps::get_machine_env(&flags, isa::CallConv::SystemV);
        for n in [0, 12, 13, 14] {
            let preg = regs::preg(n);
            assert!(!env.preferred_regs_by_class[0].contains(preg));
            assert!(!env.non_preferred_regs_by_class[0].contains(preg));
        }
    }

    #[test]
    fn narrow_integer_abi_extensions_are_signature_driven() {
        assert_eq!(
            Sia32MachineDeps::get_ext_mode(isa::CallConv::SystemV, ir::ArgumentExtension::Uext, ABIArgLocation::Reg),
            ir::ArgumentExtension::Uext
        );
    }

    #[test]
    fn basic_memory_ops_are_integer_only() {
        let load = Sia32MachineDeps::gen_load_base_offset(
            Writable::from_reg(regs::mach_reg(1)), regs::mach_reg(2), 4, I32,
        );
        assert!(matches!(load, Inst::LoadBaseOffset { .. }));
    }
}
