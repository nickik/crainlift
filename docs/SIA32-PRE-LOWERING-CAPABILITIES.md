# SIA32 capabilities before CLIF lowering

This document defines the supported SIA32 backend surface before the M5 CLIF/ISLE lowering milestone. It prevents registration and lowering-independent infrastructure from being mistaken for a complete Cranelift target.

## Available now

- `cranelift-codegen` can be built with `--no-default-features --features std,sia32`; SIA32 does not require `riscv64` or `arm64`.
- `sia32` is registered in `isa::lookup`, `isa::lookup_by_name`, and `ALL_ARCHITECTURES`.
- A SIA32 `TargetIsa` can be constructed and reports a 32-bit pointer type and the SIA32 target triple.
- The architectural register model, SIA32 register-allocation environment, ABI contract, I64 pre-lowering representation, exact instruction encoding facilities, M7 arithmetic facilities, labels and relocation contracts are available to lowering-independent tests and consumers.
- SIA32 is included in `all-native-arch` so ordinary architecture-aggregation builds cannot silently omit the scaffold.

## Deliberately unavailable

- General CLIF-to-SIA32 compilation. `compile_function` returns `CodegenError::Unsupported` until M5 lowering is complete.
- I64 CLIF lowering. M6-prep freezes representation and ABI policy only.
- A completed generic text-section builder driven by lowered MachInst functions.
- Claims that SIA32 is a production-complete Cranelift target.

## Isolation requirements

The SIA32 backend must not borrow RISC-V or AArch64 ABI/register identities. SIA32 owns its architectural register definitions (`r0`-`r15`), argument/return assignments, caller/callee-save sets, stack/frame policy and I64 register-pair policy. CI therefore builds SIA32 alone and separately retains ARM64/RISC-V regression checks.

## CI contract

The bounded SIA32 gate checks target-lexicon compatibility, an independent `std,sia32` build, registration/lookup and isolation tests, focused SIA32 unit/encoder tests, an aggregate `all-native-arch` build, and retained `std,arm64,riscv64` compilation. This gate intentionally does not attempt general SIA32 CLIF lowering before M5.
