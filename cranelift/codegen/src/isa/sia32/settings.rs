//! SIA32 settings.

use crate::settings::{self, Builder, Value, detail};
use core::fmt;

// Generated from cranelift/codegen/meta/src/isa/sia32.rs.
include!(concat!(env!("OUT_DIR"), "/settings-sia32.rs"));
