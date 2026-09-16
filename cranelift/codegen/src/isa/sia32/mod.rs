//! SIA32 backend support under construction.
//!
//! The exact encoder and architectural register definitions intentionally land
//! before the backend is wired into `isa::lookup()`. That keeps the executable
//! ISA contract independently testable while target-lexicon support is added.

pub mod encode;
pub mod regs;
