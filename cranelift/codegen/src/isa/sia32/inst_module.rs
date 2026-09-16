// Keep Cranelift's operand-visitor convenience methods in scope for the
// included SIA32 MachInst implementation. Declaration order does not affect
// trait method lookup within this module.
include!("inst.rs");
use crate::machinst::reg::OperandVisitorImpl;
