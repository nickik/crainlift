//! SIA32 architectural integer-register identities and ABI roles.
//!
//! The architectural `Reg` type is used by the exact encoder. The helpers at
//! the bottom of this module map those same register numbers onto Cranelift's
//! real-register representation and define the initial regalloc2 environment.

use crate::machinst::{Reg as MachReg, Writable};
use alloc::vec;
use core::fmt;
use regalloc2::{MachineEnv, PReg, PRegSet, RegClass};

/// One of SIA32's sixteen architectural integer registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reg(u8);

impl Reg {
    pub const ZERO: Self = Self(0);
    pub const SP: Self = Self(13);
    pub const LR: Self = Self(14);
    pub const FP: Self = Self(15);
    pub const SCRATCH: Self = Self(12);

    /// Construct an architectural register. Values outside r0..r15 are rejected.
    pub const fn new(index: u8) -> Option<Self> {
        if index < 16 { Some(Self(index)) } else { None }
    }

    pub const fn index(self) -> u8 { self.0 }

    pub const fn is_zero(self) -> bool { self.0 == 0 }
    pub const fn is_sp(self) -> bool { self.0 == 13 }
    pub const fn is_lr(self) -> bool { self.0 == 14 }
    pub const fn is_fp(self) -> bool { self.0 == 15 }
    pub const fn is_backend_scratch(self) -> bool { self.0 == 12 }

    /// Registers available to ordinary integer register allocation under the
    /// frozen initial ABI. r15 remains allocatable unless a function elects to
    /// use it as a frame pointer.
    pub const fn normally_allocatable(self) -> bool {
        matches!(self.0, 1..=11 | 15)
    }

    pub const fn caller_saved(self) -> bool { matches!(self.0, 1..=8) }
    pub const fn callee_saved(self) -> bool { matches!(self.0, 9..=11 | 15) }
    pub const fn argument_index(self) -> Option<u8> {
        match self.0 {
            1..=6 => Some(self.0 - 1),
            _ => None,
        }
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            13 => f.write_str("sp"),
            14 => f.write_str("lr"),
            15 => f.write_str("r15"),
            n => write!(f, "r{n}"),
        }
    }
}

pub const RETURN_LOW: Reg = Reg(1);
pub const RETURN_HIGH: Reg = Reg(2);
pub const ARG_REGS: [Reg; 6] = [Reg(1), Reg(2), Reg(3), Reg(4), Reg(5), Reg(6)];
pub const CALLER_SAVED: [Reg; 8] = [
    Reg(1), Reg(2), Reg(3), Reg(4), Reg(5), Reg(6), Reg(7), Reg(8),
];
pub const CALLEE_SAVED: [Reg; 4] = [Reg(9), Reg(10), Reg(11), Reg(15)];

/// Convert an architectural SIA register number into a regalloc2 physical
/// integer register. The hardware encoding is exactly the architectural number.
pub const fn preg(index: u8) -> PReg {
    assert!(index < 16);
    PReg::new(index as usize, RegClass::Int)
}

/// Convert an architectural SIA register number into Cranelift's real register.
pub const fn mach_reg(index: u8) -> MachReg {
    MachReg::from_real_reg(preg(index))
}

pub const fn zero_reg() -> MachReg { mach_reg(Reg::ZERO.index()) }
pub const fn stack_reg() -> MachReg { mach_reg(Reg::SP.index()) }
pub const fn link_reg() -> MachReg { mach_reg(Reg::LR.index()) }
pub const fn fp_reg() -> MachReg { mach_reg(Reg::FP.index()) }
pub const fn scratch_reg() -> MachReg { mach_reg(Reg::SCRATCH.index()) }

pub fn writable_zero_reg() -> Writable<MachReg> { Writable::from_reg(zero_reg()) }
pub fn writable_stack_reg() -> Writable<MachReg> { Writable::from_reg(stack_reg()) }
pub fn writable_link_reg() -> Writable<MachReg> { Writable::from_reg(link_reg()) }
pub fn writable_fp_reg() -> Writable<MachReg> { Writable::from_reg(fp_reg()) }
pub fn writable_scratch_reg() -> Writable<MachReg> { Writable::from_reg(scratch_reg()) }

/// Initial SIA32 register-allocation environment.
///
/// Caller-saved r1-r8 are preferred so leaf functions avoid save/restore work.
/// Callee-saved r9-r11/r15 are available as the second tier. Architectural zero
/// r0, backend scratch r12, stack pointer r13, and link register r14 are fixed
/// and therefore deliberately absent from both allocatable sets.
pub fn create_reg_environment() -> MachineEnv {
    let preferred_regs_by_class = [
        PRegSet::empty()
            .with(preg(1))
            .with(preg(2))
            .with(preg(3))
            .with(preg(4))
            .with(preg(5))
            .with(preg(6))
            .with(preg(7))
            .with(preg(8)),
        PRegSet::empty(),
        PRegSet::empty(),
    ];

    let non_preferred_regs_by_class = [
        PRegSet::empty()
            .with(preg(9))
            .with(preg(10))
            .with(preg(11))
            .with(preg(15)),
        PRegSet::empty(),
        PRegSet::empty(),
    ];

    MachineEnv {
        preferred_regs_by_class,
        non_preferred_regs_by_class,
        fixed_stack_slots: vec![],
        // r12 is reserved for backend-controlled late expansion; it is not a
        // regalloc scratch register because branch/literal veneers must be able
        // to rely on it independently of regalloc move resolution.
        scratch_by_class: [None, None, None],
    }
}
