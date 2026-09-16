//! Executable model of the frozen SIA32 compiler ABI.
//!
//! This module is intentionally independent from Cranelift's generic ABI
//! machinery. It is the small, easily-tested placement oracle that the real
//! `ABIMachineSpec` implementation must match once calls and stack frames are
//! wired into the backend.

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ValueKind {
    I8,
    I16,
    I32,
    Ptr32,
    I64,
}

impl ValueKind {
    const fn stack_size(self) -> u32 {
        match self {
            Self::I8 | Self::I16 | Self::I32 | Self::Ptr32 => 4,
            Self::I64 => 8,
        }
    }

    const fn stack_align(self) -> u32 {
        self.stack_size()
    }

    const fn register_words(self) -> u8 {
        match self {
            Self::I8 | Self::I16 | Self::I32 | Self::Ptr32 => 1,
            Self::I64 => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Location {
    Reg(u8),
    RegPair { low: u8, high: u8 },
    Stack { offset: u32, size: u32, align: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) values: Vec<Location>,
    pub(crate) stack_size: u32,
}

const ARG_FIRST: u8 = 1;
const ARG_LAST: u8 = 6;
const STACK_ALIGN: u32 = 8;

const fn align_to(value: u32, align: u32) -> u32 {
    debug_assert!(align.is_power_of_two());
    (value + (align - 1)) & !(align - 1)
}

/// Place ordinary call arguments according to the frozen SIA32 ABI.
///
/// If `hidden_sret` is true then r1 is consumed by the hidden return-area
/// pointer before user arguments are assigned. Once one argument has to move to
/// the stack, all following arguments are stack-assigned as well. I64 values
/// use even-starting ABI pairs r1:r2, r3:r4 or r5:r6 and never straddle the
/// register/stack boundary.
pub(crate) fn place_args(kinds: &[ValueKind], hidden_sret: bool) -> Placement {
    let mut values = Vec::with_capacity(kinds.len());
    let mut next_reg = if hidden_sret { 2 } else { ARG_FIRST };
    let mut stack_only = false;
    let mut next_stack = 0u32;

    for kind in kinds.iter().copied() {
        let words = kind.register_words();
        let location = if !stack_only {
            match words {
                1 if next_reg <= ARG_LAST => {
                    let reg = next_reg;
                    next_reg += 1;
                    Some(Location::Reg(reg))
                }
                2 => {
                    // 64-bit values begin on an odd-numbered architectural
                    // register, yielding ABI pairs 1:2, 3:4, 5:6. If the
                    // current scalar cursor points at an even register, leave
                    // that register unused rather than changing the pair ABI.
                    if next_reg % 2 == 0 {
                        next_reg += 1;
                    }
                    if next_reg + 1 <= ARG_LAST {
                        let low = next_reg;
                        let high = next_reg + 1;
                        next_reg += 2;
                        Some(Location::RegPair { low, high })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        if let Some(location) = location {
            values.push(location);
            continue;
        }

        stack_only = true;
        let align = kind.stack_align();
        next_stack = align_to(next_stack, align);
        let size = kind.stack_size();
        values.push(Location::Stack {
            offset: next_stack,
            size,
            align,
        });
        next_stack += size;
    }

    Placement {
        values,
        stack_size: align_to(next_stack, STACK_ALIGN),
    }
}

/// Place return values for the first backend milestone.
///
/// One <=32-bit value returns in r1. One I64 returns in r1:r2. Two <=32-bit
/// values return in r1 and r2. Anything larger must be converted by the caller
/// into the hidden-sret form rather than silently inventing another convention.
pub(crate) fn place_rets(kinds: &[ValueKind]) -> Option<Placement> {
    let values = match kinds {
        [] => Vec::new(),
        [ValueKind::I8 | ValueKind::I16 | ValueKind::I32 | ValueKind::Ptr32] => {
            alloc::vec![Location::Reg(1)]
        }
        [ValueKind::I64] => alloc::vec![Location::RegPair { low: 1, high: 2 }],
        [
            ValueKind::I8 | ValueKind::I16 | ValueKind::I32 | ValueKind::Ptr32,
            ValueKind::I8 | ValueKind::I16 | ValueKind::I32 | ValueKind::Ptr32,
        ] => alloc::vec![Location::Reg(1), Location::Reg(2)],
        _ => return None,
    };

    Some(Placement {
        values,
        stack_size: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn six_scalar_args_fill_r1_through_r6() {
        let p = place_args(&[ValueKind::I32; 6], false);
        assert_eq!(
            p.values,
            vec![
                Location::Reg(1),
                Location::Reg(2),
                Location::Reg(3),
                Location::Reg(4),
                Location::Reg(5),
                Location::Reg(6),
            ]
        );
        assert_eq!(p.stack_size, 0);
    }

    #[test]
    fn first_stack_arg_forces_following_args_to_stack() {
        let p = place_args(&[ValueKind::I32; 8], false);
        assert_eq!(p.values[5], Location::Reg(6));
        assert_eq!(
            p.values[6],
            Location::Stack {
                offset: 0,
                size: 4,
                align: 4,
            }
        );
        assert_eq!(
            p.values[7],
            Location::Stack {
                offset: 4,
                size: 4,
                align: 4,
            }
        );
        assert_eq!(p.stack_size, 8);
    }

    #[test]
    fn i64_uses_aligned_register_pairs() {
        let p = place_args(
            &[ValueKind::I32, ValueKind::I64, ValueKind::I32, ValueKind::I64],
            false,
        );
        assert_eq!(p.values[0], Location::Reg(1));
        assert_eq!(p.values[1], Location::RegPair { low: 3, high: 4 });
        assert_eq!(p.values[2], Location::Reg(5));
        assert_eq!(
            p.values[3],
            Location::Stack {
                offset: 0,
                size: 8,
                align: 8,
            }
        );
        assert_eq!(p.stack_size, 8);
    }

    #[test]
    fn hidden_sret_consumes_r1() {
        let p = place_args(&[ValueKind::Ptr32, ValueKind::I32], true);
        assert_eq!(p.values, vec![Location::Reg(2), Location::Reg(3)]);
    }

    #[test]
    fn stack_i64_is_eight_byte_aligned() {
        let p = place_args(
            &[
                ValueKind::I32,
                ValueKind::I32,
                ValueKind::I32,
                ValueKind::I32,
                ValueKind::I32,
                ValueKind::I32,
                ValueKind::I32,
                ValueKind::I64,
            ],
            false,
        );
        assert_eq!(
            p.values[6],
            Location::Stack {
                offset: 0,
                size: 4,
                align: 4,
            }
        );
        assert_eq!(
            p.values[7],
            Location::Stack {
                offset: 8,
                size: 8,
                align: 8,
            }
        );
        assert_eq!(p.stack_size, 16);
    }

    #[test]
    fn returns_are_bounded_and_explicit() {
        assert_eq!(
            place_rets(&[ValueKind::I32]).unwrap().values,
            vec![Location::Reg(1)]
        );
        assert_eq!(
            place_rets(&[ValueKind::I64]).unwrap().values,
            vec![Location::RegPair { low: 1, high: 2 }]
        );
        assert_eq!(
            place_rets(&[ValueKind::Ptr32, ValueKind::I16])
                .unwrap()
                .values,
            vec![Location::Reg(1), Location::Reg(2)]
        );
        assert!(place_rets(&[ValueKind::I32, ValueKind::I32, ValueKind::I32]).is_none());
    }

    #[test]
    fn narrow_values_are_one_abi_word() {
        let p = place_args(&[ValueKind::I8, ValueKind::I16, ValueKind::Ptr32], false);
        assert_eq!(
            p.values,
            vec![Location::Reg(1), Location::Reg(2), Location::Reg(3)]
        );
    }
}
