// Bring MachInst-provided helpers (`rc_for_type`, `gen_move`) into scope for
// the included SIA32 ABI implementation without duplicating the large file.
include!("abi.rs");
use crate::machinst::MachInst;
