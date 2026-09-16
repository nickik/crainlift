//! SIA32 architectural integer-register identities and ABI roles.
//!
//! This is intentionally independent from regalloc2 for the first encoder
//! milestone. The MachInst register-class adapter will map these exact physical
//! registers into Cranelift's register allocator later.

use core::fmt;

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
