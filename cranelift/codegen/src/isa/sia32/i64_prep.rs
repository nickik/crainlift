//! Frozen SIA32 I64 representation and lowering-independent ABI policy.
//!
//! This module deliberately does not make I64 a supported CLIF type. It is a
//! contract for the later lowering implementation.

use super::regs::Reg;

/// One 64-bit integer as two little-endian 32-bit machine words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct I64Words {
    pub(crate) low: u32,
    pub(crate) high: u32,
}

impl I64Words {
    pub(crate) const fn from_u64(value: u64) -> Self {
        Self { low: value as u32, high: (value >> 32) as u32 }
    }
    pub(crate) const fn to_u64(self) -> u64 { (self.low as u64) | ((self.high as u64) << 32) }
}

/// Allocatable architectural register pair for one I64 value.
///
/// The low word is always in the lower, odd-numbered register and the high word
/// is in the following register. This matches the call ABI and prevents a value
/// from ever including r0, backend scratch r12, SP, LR, or FP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct I64RegPair {
    pub(crate) low: Reg,
    pub(crate) high: Reg,
}

impl I64RegPair {
    pub(crate) const fn new(low: Reg, high: Reg) -> Option<Self> {
        if low.index() % 2 == 1
            && high.index() == low.index() + 1
            && low.normally_allocatable()
            && high.normally_allocatable()
        {
            Some(Self { low, high })
        } else {
            None
        }
    }
}

/// The only register pairs the future I64 allocator/lowering may use.
pub(crate) const I64_ALLOCATABLE_PAIRS: [I64RegPair; 5] = [
    I64RegPair { low: Reg::new(1).unwrap(), high: Reg::new(2).unwrap() },
    I64RegPair { low: Reg::new(3).unwrap(), high: Reg::new(4).unwrap() },
    I64RegPair { low: Reg::new(5).unwrap(), high: Reg::new(6).unwrap() },
    I64RegPair { low: Reg::new(7).unwrap(), high: Reg::new(8).unwrap() },
    I64RegPair { low: Reg::new(9).unwrap(), high: Reg::new(10).unwrap() },
];

pub(crate) const I64_STACK_SIZE: u32 = 8;
pub(crate) const I64_STACK_ALIGN: u32 = 8;
pub(crate) const I64_LOW_WORD_OFFSET: u32 = 0;
pub(crate) const I64_HIGH_WORD_OFFSET: u32 = 4;

/// Abstract instruction sequence required for exact 64-bit add/sub expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PairArithmeticStep {
    AddLow,
    CarryIfLowUnsignedLessThanLhs,
    AddHigh,
    AddCarryToHigh,
    SubLow,
    BorrowIfLhsLowUnsignedLessThanRhs,
    SubHigh,
    SubBorrowFromHigh,
}

pub(crate) const ADD64_SEQUENCE: [PairArithmeticStep; 4] = [
    PairArithmeticStep::AddLow,
    PairArithmeticStep::CarryIfLowUnsignedLessThanLhs,
    PairArithmeticStep::AddHigh,
    PairArithmeticStep::AddCarryToHigh,
];

pub(crate) const SUB64_SEQUENCE: [PairArithmeticStep; 4] = [
    PairArithmeticStep::SubLow,
    PairArithmeticStep::BorrowIfLhsLowUnsignedLessThanRhs,
    PairArithmeticStep::SubHigh,
    PairArithmeticStep::SubBorrowFromHigh,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum I64ExpensiveOp { Mul, SignedDiv, UnsignedDiv, SignedRem, UnsignedRem }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HelperCallPolicy {
    pub(crate) symbol: &'static str,
    pub(crate) lhs_low: Reg,
    pub(crate) lhs_high: Reg,
    pub(crate) rhs_low: Reg,
    pub(crate) rhs_high: Reg,
    pub(crate) ret_low: Reg,
    pub(crate) ret_high: Reg,
}

/// Expensive I64 operations use stable runtime helpers initially rather than
/// duplicating complex multiword algorithms in ISLE. Arguments occupy r1:r2
/// and r3:r4; the result returns in r1:r2.
pub(crate) const fn helper_policy(op: I64ExpensiveOp) -> HelperCallPolicy {
    let symbol = match op {
        I64ExpensiveOp::Mul => "__sia32_i64_mul",
        I64ExpensiveOp::SignedDiv => "__sia32_i64_sdiv",
        I64ExpensiveOp::UnsignedDiv => "__sia32_i64_udiv",
        I64ExpensiveOp::SignedRem => "__sia32_i64_srem",
        I64ExpensiveOp::UnsignedRem => "__sia32_i64_urem",
    };
    HelperCallPolicy {
        symbol,
        lhs_low: Reg::new(1).unwrap(), lhs_high: Reg::new(2).unwrap(),
        rhs_low: Reg::new(3).unwrap(), rhs_high: Reg::new(4).unwrap(),
        ret_low: Reg::new(1).unwrap(), ret_high: Reg::new(2).unwrap(),
    }
}

/// Reference semantics for the frozen carry sequence.
pub(crate) const fn add_reference(lhs: I64Words, rhs: I64Words) -> I64Words {
    let (low, carry) = lhs.low.overflowing_add(rhs.low);
    let high = lhs.high.wrapping_add(rhs.high).wrapping_add(carry as u32);
    I64Words { low, high }
}

/// Reference semantics for the frozen borrow sequence.
pub(crate) const fn sub_reference(lhs: I64Words, rhs: I64Words) -> I64Words {
    let (low, borrow) = lhs.low.overflowing_sub(rhs.low);
    let high = lhs.high.wrapping_sub(rhs.high).wrapping_sub(borrow as u32);
    I64Words { low, high }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(n: u8) -> Reg { Reg::new(n).unwrap() }

    #[test]
    fn word_order_is_low_then_high_little_endian() {
        let words = I64Words::from_u64(0x1122_3344_5566_7788);
        assert_eq!(words, I64Words { low: 0x5566_7788, high: 0x1122_3344 });
        assert_eq!(words.to_u64(), 0x1122_3344_5566_7788);
        assert_eq!((I64_LOW_WORD_OFFSET, I64_HIGH_WORD_OFFSET), (0, 4));
        assert_eq!((I64_STACK_SIZE, I64_STACK_ALIGN), (8, 8));
    }

    #[test]
    fn only_frozen_odd_even_allocatable_pairs_are_valid() {
        for pair in I64_ALLOCATABLE_PAIRS { assert_eq!(I64RegPair::new(pair.low, pair.high), Some(pair)); }
        assert_eq!(I64RegPair::new(r(2), r(3)), None);
        assert_eq!(I64RegPair::new(r(11), r(12)), None);
        assert_eq!(I64RegPair::new(r(12), r(13)), None);
        assert_eq!(I64RegPair::new(r(15), r(0)), None);
    }

    #[test]
    fn carry_and_borrow_sequences_are_frozen() {
        assert_eq!(ADD64_SEQUENCE, [PairArithmeticStep::AddLow, PairArithmeticStep::CarryIfLowUnsignedLessThanLhs, PairArithmeticStep::AddHigh, PairArithmeticStep::AddCarryToHigh]);
        assert_eq!(SUB64_SEQUENCE, [PairArithmeticStep::SubLow, PairArithmeticStep::BorrowIfLhsLowUnsignedLessThanRhs, PairArithmeticStep::SubHigh, PairArithmeticStep::SubBorrowFromHigh]);
        let a = I64Words::from_u64(0x0000_0001_ffff_ffff);
        assert_eq!(add_reference(a, I64Words::from_u64(1)).to_u64(), 0x0000_0002_0000_0000);
        assert_eq!(sub_reference(I64Words::from_u64(0x0000_0002_0000_0000), I64Words::from_u64(1)).to_u64(), 0x0000_0001_ffff_ffff);
    }

    #[test]
    fn expensive_operations_have_stable_helpers_and_pair_abi() {
        let cases = [(I64ExpensiveOp::Mul,"__sia32_i64_mul"),(I64ExpensiveOp::SignedDiv,"__sia32_i64_sdiv"),(I64ExpensiveOp::UnsignedDiv,"__sia32_i64_udiv"),(I64ExpensiveOp::SignedRem,"__sia32_i64_srem"),(I64ExpensiveOp::UnsignedRem,"__sia32_i64_urem")];
        for (op, symbol) in cases {
            let p = helper_policy(op); assert_eq!(p.symbol, symbol);
            assert_eq!((p.lhs_low,p.lhs_high,p.rhs_low,p.rhs_high),(r(1),r(2),r(3),r(4)));
            assert_eq!((p.ret_low,p.ret_high),(r(1),r(2)));
        }
    }
}
