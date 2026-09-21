//! Top-level lib.rs for `cranelift_object`.
//!
//! This re-exports `object` so you don't have to explicitly keep the versions in sync.

#![allow(elided_lifetimes_in_paths, explicit_outlives_requirements)]
#![deny(missing_docs)]

mod backend;
#[cfg(feature = "unwind")]
mod unwind;

pub use crate::backend::{ObjectBuilder, ObjectModule, ObjectProduct};

/// Version number of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use object;
