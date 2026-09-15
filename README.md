# Cranelift for Forge

This repository is a reduced Cranelift fork used by the Forge compiler and, later, the SIA target backend.

The fork intentionally excludes the Wasmtime runtime, WASI, Winch, component-model tooling, Wasmtime CLI, fuzz harnesses, examples, and Pulley. It retains the Cranelift implementation plus the small `wasmtime-internal-core` utility crate that current Cranelift sources still depend on.

## Forge integration

Forge owns language semantics through FIR. CLIF is a code-generation IR, not a Forge semantic compiler layer:

```text
Forge source -> AST -> HIR -> Typed HIR -> FIR -> CLIF -> Cranelift target
                                                   |-> AArch64
                                                   |-> RISC-V64
                                                   `-> SIA (later)
```

The initial supported Forge targets are AArch64 and RISC-V64. SIA support will be added here as a normal Cranelift ISA backend after the FIR-to-CLIF lowering is mature.

## Scope

Changes in this fork should remain Cranelift/backend changes. Forge-specific semantics belong in the Forge repository and must not migrate below the FIR boundary.
