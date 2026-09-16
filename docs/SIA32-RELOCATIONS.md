# SIA32 relocation and object-code contract

Status: M8 relocation contract for the SIA32 Cranelift backend.

This document defines the relocation records emitted by Cranelift. It is intentionally independent of the M5 ISLE lowering work.

## 1. Object boundary

The first SIA32 integration uses Forge's existing object-emission abstraction rather than pretending SIA32 is another architecture supported by `cranelift-object`.

The pinned `cranelift-object` backend does not have an `object::Architecture` / ELF machine mapping for `Architecture::Sia32`. Until a real SIA ELF identity is added to the external `object` crate and agreed by the Forge/Cosmic linker toolchain, `cranelift-object` is not the SIA32 container contract.

Cranelift therefore emits ordinary `MachReloc` records. Forge (or a later Cosmic object writer/linker) consumes those records and chooses the final container representation.

No SIA-specific variant is added to `cranelift_codegen::binemit::Reloc` merely to rename an existing relocation. The generic `Reloc::Abs4` already means an absolute generic 32-bit word and is the canonical SIA32 `ABS32` relocation.

## 2. ABS32

SIA32 `ABS32` is represented by:

```text
Reloc::Abs4
```

The relocation applies to one four-byte, little-endian, four-byte-aligned word.

Its mathematical value is:

```text
S + A
```

where:

- `S` is the final absolute symbol address;
- `A` is the signed relocation addend.

For SIA32, `S + A` must be in the inclusive range `0x00000000 .. 0xffffffff`. The object writer/linker must report an overflow when it is outside that range. Silent low-32-bit truncation is forbidden.

Backend APIs that create SIA32 absolute-address words use an `i32` addend. Conversion to Cranelift's generic `i64` `MachReloc` addend is explicit. Host `usize` is not part of the target relocation contract.

## 3. External function calls

The existing direct/external call sequence is the canonical long-call form:

```text
    LDPC.W r12, target_literal
    CALLR   r12
    B       after_literal
    .align  4
target_literal:
    .word   0              ; ABS32 against the callee, addend 0
after_literal:
```

The `.word` carries `Reloc::Abs4` against the external function name.

This sequence is independent of the short architectural `BL` range. The literal is emitted beside the call, so the `LDPC.W` reference is a local label fixup whose range is known to the backend. The branch after `CALLR` prevents execution from falling into the relocated word after the callee returns.

No external-call PC-relative relocation is required by M8.

## 4. External data and symbol addresses

An absolute address of an external function, global, or data symbol uses the same `ABS32` record. The backend provides an aligned relocated-word primitive for this purpose:

```text
    .align 4
symbol_literal:
    .word 0                ; ABS32(symbol, signed i32 addend)
```

Code that places such a word inline in executable text must ensure that normal instruction flow cannot enter the word. A future M5 global-address lowering rule may use `LDPC.W` plus a branch-around literal in the same manner as direct calls; M8 does not depend on that ISLE rule existing.

Data sections may use the relocated word directly without an executable branch-around sequence.

## 5. Local PC-relative references

M8 defines no object-level PC-relative SIA relocation.

The following references are internal to a compiled machine-code buffer and are resolved by SIA `LabelUse` fixups before the object boundary:

- `BNZ` / conditional branch targets;
- `B` targets;
- local call/branch veneers;
- `LDPC.W` references to inline literals and constant islands.

If a future ABI or object format introduces a PC-relative reference whose target can be outside the current emitted buffer, that is a new relocation kind and requires its own range, addend, overflow, and linker semantics. It must not be encoded as `ABS32` by convenience.

## 6. Relocation offsets and contents

The relocation offset is the byte offset of the first byte of the four-byte address word, not the offset of the `LDPC.W`, call, branch, or symbol definition.

The emitted four-byte field is initially zero. The object writer/linker replaces that field according to `S + A`.

SIA32 relocation words are four-byte aligned even though ordinary instructions are two-byte aligned.

For the canonical direct-call sequence emitted at a four-byte-aligned function start, the sequence occupies 12 bytes and its `ABS32` relocation is at byte offset 8.

## 7. Range and overflow policy

There are two distinct range mechanisms:

1. Local instruction reach is a Cranelift/SIA emitter problem. `LabelUse` checks the architectural displacement range and uses veneers/islands where supported.
2. `ABS32` symbol value range is an object writer/linker problem. The final `S + A` must fit an unsigned 32-bit target address.

Neither mechanism may silently wrap or truncate an out-of-range value.

## 8. Object-writer requirements

A Forge/Cosmic object writer consuming SIA32 Cranelift output must:

- preserve the exact `MachReloc` byte offset;
- map SIA32 `Reloc::Abs4` to the chosen container's absolute-32 relocation representation;
- preserve the signed addend exactly;
- preserve the referenced symbol identity;
- reject an unsupported relocation kind rather than guessing;
- reject final `ABS32` underflow/overflow rather than truncating;
- keep target arithmetic independent of host pointer width;
- support both function and data/global symbol targets.

The current Forge writer is still an ELF64 writer for its existing AArch64/RISC-V targets. Adding the SIA32 container identity and cross-object link tests is the Forge-side integration step of M8; it must follow this contract rather than introducing a second relocation model.
