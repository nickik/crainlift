# SIA32 Cranelift ABI

Status: **implemented ABI contract** on `sia32-backend`.

This document is normative for the ABI currently implemented by `isa/sia32/abi.rs`, `inst.rs`, and `regs.rs`. It describes what the backend does today, not planned behavior. A change to any item explicitly identified as ABI-visible below requires a deliberate Forge/Cosmic compatibility decision.

## 1. Machine model

SIA32 is a 32-bit, little-endian target.

- architectural integer registers: `r0..r15`
- machine word: 32 bits / 4 bytes
- pointer width: 32 bits
- stack grows downward
- call-boundary stack alignment: 8 bytes
- red zone: none
- shadow space: none
- frame pointer: not required by the current ABI
- implemented register classes: integer only

Cranelift's maximum encoded stack argument/return area accepted by this backend is 128 MiB (`STACK_ARG_RET_SIZE_LIMIT = 128 * 1024 * 1024`). This is an implementation limit, not an application-visible reserved stack area.

## 2. Register roles

| Register | ABI role | Allocatable | Preservation |
| --- | --- | --- | --- |
| `r0` | architectural zero | no | fixed |
| `r1..r6` | argument registers | yes | caller-saved |
| `r7..r8` | ordinary temporaries | yes | caller-saved |
| `r9..r11` | ordinary temporaries | yes | callee-saved |
| `r12` | backend scratch / return-value temporary | no | caller-clobbered |
| `r13` | stack pointer (`sp`) | no | special |
| `r14` | link register (`lr`) | no | caller-clobbered by calls |
| `r15` | ordinary allocatable integer register; possible future FP role | yes | callee-saved |

The regalloc environment is exactly:

- preferred: `r1..r8`
- non-preferred: `r9..r11`, `r15`
- fixed/non-allocatable: `r0`, `r12`, `r13`, `r14`

A call clobbers `r1..r8`, `r12`, and `r14`. A callee that writes any of `r9`, `r10`, `r11`, or `r15` must preserve that register.

`r12` is permanently unavailable to register allocation. The emitter relies on this for large immediates, address expansion, literal loads, direct long calls, and the generic ABI return-value temporary.

## 3. Supported value representation

The current backend implements only the integer register class.

- `I8`, `I16`, `I32`, pointers, and references occupy one 32-bit integer register part.
- `I64` occupies two 32-bit integer register parts.
- an `I64` is represented low word first, high word second.
- floating point, vectors, and `I128` are not part of the implemented ABI.

The upper unused bits of narrow integer register values are governed by the CLIF signature's `ArgumentExtension`; the ABI itself does not impose an additional default extension. `TargetIsa::default_argument_extension()` is `None`.

## 4. Calling-convention identity

The backend currently accepts only Cranelift `SystemV` as its compiler calling-convention identifier.

This does **not** mean the SIA32 ABI is the ABI of another System-V architecture. `SystemV` is only the Cranelift identifier used to select the SIA32-specific rules in this document. Other Cranelift calling conventions are rejected explicitly.

## 5. Arguments

Arguments are assigned left-to-right.

### 5.1 Scalar arguments

Normal scalar arguments use `r1`, `r2`, `r3`, `r4`, `r5`, `r6` in that order.

If an argument cannot fit in the remaining argument-register space, that argument goes to the stack and **all subsequent arguments are stack arguments**. Register allocation never resumes after the first stack argument.

### 5.2 I64 arguments

An `I64` requires a complete odd/even register pair:

- `r1:r2`
- `r3:r4`
- `r5:r6`

The low 32 bits are in the lower-numbered register. The high 32 bits are in the higher-numbered register.

If the next free argument register is even, it is skipped before placing an `I64`. An `I64` is never split between a register and the stack.

Examples:

- `(I32, I64)` -> `r1`, then `r3:r4`; `r2` is unused.
- `(I32, I32, I64)` -> `r1`, `r2`, then `r3:r4`.
- if an `I64` cannot fit completely before `r6`, it and every later argument are stack-passed.

### 5.3 Stack arguments

Scalar stack arguments:

- alignment: 4 bytes
- occupied size: 4 bytes

`I64` stack arguments:

- alignment: 8 bytes
- occupied size: 8 bytes
- low word at the lower address
- high word at the next 4-byte address

The complete stack argument area is rounded up to 8 bytes.

### 5.4 Narrow integer extension

`get_ext_mode()` preserves the `ArgumentExtension` specified by CLIF for both register and stack locations. The SIA32 ABI does not silently substitute sign- or zero-extension.

## 6. Hidden return-area pointer and aggregates

When Cranelift requests an implicit stack return area (`add_ret_area_ptr`):

- the hidden return-area pointer is physically passed in `r1`;
- it is represented as a non-formal ABI argument in Cranelift metadata;
- formal user arguments begin allocation at `r2`.

`StructArgument` is rejected by the current backend. By-value aggregate classification is therefore not defined. Forge/Cosmic must lower aggregates explicitly by address, or use the supported return-area mechanism where applicable.

The backend does not synthesize aggregate `memcpy` operations.

## 7. Return values

The complete register return area is `r1:r2`.

- one scalar return -> `r1`
- two scalar returns -> `r1`, then `r2`
- one `I64` return -> `r1:r2`, low word in `r1`, high word in `r2`

A return value is never partially placed in registers.

If the return set does not fit in `r1:r2`:

- with Cranelift multi-return implicit-sret enabled, excess values use Cranelift's stack-return-area mechanism;
- otherwise ABI construction fails explicitly.

The current exception-payload compiler hook exposes `r1` and `r2`. This is an implementation hook and is **not** a separately frozen Cosmic exception ABI.

## 8. Stack-frame layout

Cranelift tracks the following regions independently:

1. incoming arguments
2. tail-argument area
3. SIA setup area
4. callee-save clobber area
5. fixed frame storage
6. stack slots
7. outgoing arguments

Fixed frame storage and outgoing-argument storage are rounded to 8 bytes.

### 8.1 Link-register setup area

A function classified by Cranelift as making regular calls receives an 8-byte setup area.

The prologue performs:

```text
sp = sp - 8
[sp + 0] = lr
```

Bytes `[sp + 4 .. sp + 7]` are padding. They are not currently assigned another ABI purpose.

The epilogue performs:

```text
lr = [sp + 0]
sp = sp + 8
```

A leaf function that does not require regular-call frame setup does not allocate this area solely for `lr`.

### 8.2 Callee-saved register area

Only callee-saved registers actually written by the function are saved.

The set is selected from:

```text
r9, r10, r11, r15
```

Saved registers are sorted by register number. The area size is:

```text
align_up(4 * number_of_saved_registers, 8)
```

The save sequence decrements `sp` by the complete clobber-area size and stores registers at offsets `0, 4, 8, ...`. Restore loads the same offsets and then increments `sp` by the area size.

### 8.3 Spill slots

An integer regalloc spill consumes one Cranelift spill slot. Scalar accesses are 1, 2, or 4 bytes according to the logical operation; the ABI frame accounting remains word-oriented.

`I64` SSA representation is two 32-bit register parts. Full I64 lowering/spill arithmetic policy is completed in the dedicated 64-bit lowering milestone rather than pretending SIA has 64-bit registers.

## 9. Calls and the link register

Architectural calls use `r14` as `lr`.

### 9.1 Direct/external calls

Direct calls are currently emitted as an address-independent long-call sequence using reserved `r12`:

```text
    LDPC.W r12, target_literal
    CALLR   r12
    B       after_literal
    .align  4
target_literal:
    .word   target        ; Reloc::Abs4
after_literal:
```

The target word carries an absolute 32-bit `Reloc::Abs4` relocation against the external name. The inline literal is placed immediately with the call, so the `LDPC.W` displacement remains in range.

`CALLR` writes the architectural return address to `r14`. When the callee returns, execution resumes at the branch following `CALLR`, which skips the embedded target word.

This sequence deliberately avoids making external-call correctness depend on the short `BL` displacement range.

### 9.2 Indirect calls

Indirect calls emit:

```text
CALLR target_register
```

The target register is an ordinary call use. The normal call argument/return fixed-register constraints and clobber set still apply.

### 9.3 Cranelift call metadata

Both direct and indirect calls are marked as regular calls and safepoints. The backend records ordinary or try-call sites as appropriate and forwards user stack-map metadata at the return address.

Patchable call sites are not currently implemented. `callee_pop_size` must be zero; SIA32 SystemV uses caller-owned stack argument areas.

Tail calls are not implemented by the current ABI.

## 10. Backend scratch and large-address expansion

The following are emitter implementation details, but they explain why `r12` is ABI-reserved:

- large `AddImm`/`SpAdjust` values are materialized through `r12` when necessary;
- non-zero base+offset memory operations may form the effective address in `r12`;
- arbitrary 32-bit constants can be built from signed 7-bit radix-128 digits or loaded from an inline `LDPC.W` literal island;
- direct external calls load their relocated absolute target through `r12`.

No user or regalloc value may remain live in `r12` across these expansions.

## 11. Deliberately unsupported ABI behavior

The implemented ABI deliberately does not provide:

- floating-point or vector arguments/returns
- `I128` values
- by-value aggregate classification
- automatic aggregate `memcpy`
- variadic register-save areas
- tail-call ABI
- patchable calls
- stack probing (`gen_probestack` and inline probing fail if requested)
- TLS ABI
- unwind/DWARF register numbering
- a separately frozen exception ABI

`r12` is also returned by the generic stack-limit-register hook, but stack probing/check generation is not implemented. This must not be interpreted as an active architectural stack-limit ABI.

## 12. ABI stability boundary

The following are ABI-visible and require an explicit compatibility change if modified:

- argument registers `r1..r6`
- return area `r1:r2`
- caller/callee-save sets
- special roles of `r12`, `r13`, `r14`, and current preservation rule for `r15`
- 8-byte call-boundary stack alignment
- scalar and I64 stack-slot alignment/order
- I64 low-word/high-word ordering
- hidden return-area pointer placement in `r1`
- link-register semantics

The following are **not** ABI-visible provided observable behavior remains identical:

- balanced-immediate constant-building sequences
- exact literal-island placement
- branch veneers and relaxation strategy
- choice of long-call sequence
- internal `MachInst` pseudo forms

Those emitter mechanisms may evolve without changing Forge/Cosmic binaries that obey this ABI.
