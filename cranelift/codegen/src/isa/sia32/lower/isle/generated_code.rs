// Generated at build time from sia32/{inst,lower}.isle.
//
// Keep attributes outside the include: inner attributes are not accepted from
// an include! whose path comes from OUT_DIR/ISLE_DIR.
#![expect(
    dead_code,
    unreachable_patterns,
    unused_imports,
    unused_variables,
    irrefutable_let_patterns,
    clippy::clone_on_copy,
    reason = "generated ISLE code"
)]

include!(concat!(env!("ISLE_DIR"), "/isle_sia32.rs"));
