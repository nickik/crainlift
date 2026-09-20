#![cfg(feature = "sia32")]

use cranelift_codegen::isa::{self, ALL_ARCHITECTURES, LookupError};
use cranelift_codegen::settings;
use target_lexicon::Triple;

#[test]
fn sia32_is_registered_and_lookup_by_name_builds_the_sia_backend() {
    assert!(ALL_ARCHITECTURES.contains(&"sia32"));
    let builder = isa::lookup_by_name("sia32").expect("sia32 feature must register target lookup");
    let isa = builder
        .finish(settings::Flags::new(settings::builder()))
        .expect("sia32 backend must construct before CLIF lowering");
    assert_eq!(isa.name(), "sia32");
    assert_eq!(isa.triple().architecture.to_string(), "sia32");
    assert_eq!(isa.pointer_type(), cranelift_codegen::ir::types::I32);
}

#[test]
fn sia32_triple_lookup_does_not_alias_an_existing_backend() {
    let triple: Triple = "sia32-unknown-none".parse().expect("valid SIA32 triple");
    let builder = isa::lookup(triple).expect("sia32 lookup must be enabled");
    let target = builder
        .finish(settings::Flags::new(settings::builder()))
        .expect("sia32 backend must construct");
    assert_eq!(target.name(), "sia32");
    assert_ne!(target.name(), "riscv64");
    assert_ne!(target.name(), "aarch64");
}

#[cfg(not(feature = "riscv64"))]
#[test]
fn independent_sia32_build_does_not_enable_riscv64() {
    let triple: Triple = "riscv64-unknown-none".parse().unwrap();
    assert!(matches!(
        isa::lookup(triple),
        Err(LookupError::SupportDisabled)
    ));
}

#[cfg(not(feature = "arm64"))]
#[test]
fn independent_sia32_build_does_not_enable_arm64() {
    let triple: Triple = "aarch64-unknown-none".parse().unwrap();
    assert!(matches!(
        isa::lookup(triple),
        Err(LookupError::SupportDisabled)
    ));
}
