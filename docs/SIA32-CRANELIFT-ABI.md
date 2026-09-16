# SIA32 Cranelift ABI

Status: implemented compiler ABI for the `sia32` backend on branch `sia32-backend`.

This document describes the ABI actually implemented by `cranelift/codegen/src/isa/sia32/abi.rs`, `inst.rs`, and `regs.rs`. It is the contract that Forge and Cosmic should target until deliberately versioned.

## Machine model

- XLEN / pointer width: 32 bits.
- Endianness: little-endian.
- Architectural GPRs: `r0..r15`.
- `r0`: fixed zero, never allocatable.
- `r13`: stack pointer (`sp`), fixed, never allocatable.
- `r14`: link register (`lr`), fixed, never allocatable.
- `r12`: reserved compiler scratch register, never allocatable. It is available to emitter/prologue/relaxation expansion and must not carry an SSA value across an instruction expansion.
- `r15`: allocatable, callee-saved. It is the ABI's optional frame-pointer register when a frame pointer is introduced later.
- No architectural flags/condition-code register is part of the ABI.

## Register classes

The initial backend exposes one Cranelift register class only: integer.

- `I8`, `I16`, `I32`, pointers and references occupy one 32-bit integer GPR.
- `I64` occupies two integer GPR parts, low 32 bits first, high 32 bits second.
- Floating-point and vector SSA classes are unsupported in the initial backend.
- `I128` is unsupported.

## Register allocation set

Allocatable registers are:

- preferred / caller-saved: `r1..r8`
- non-preferred / callee-saved: `r9..r11`, `r15`

Fixed / unavailable to regalloc:

- `r0` zero
- `r12` compiler scratch
- `r13` `sp`
- `r14` `lr`

## Calling convention

The stable public calling convention is Cranelift `SystemV` as an identifier only; the actual register and stack rules are SIA32-specific and are defined here. Other Cranelift calling conventions are rejected rather than inheriting another target's convention.

### Scalar arguments

Scalar integer/pointer arguments are assigned left-to-right to:

`r1, r2, r3, r4, r5, r6`

After the argument-register area is exhausted, arguments are passed on the stack.

Narrow integer values use a full 32-bit register/slot. The generic ABI does not promise sign or zero extension beyond the argument type's explicit lowering requirements.

### I64 arguments

`I64` consumes an aligned pair of argument locations and is represented low word first.

Register pairs therefore begin on an odd-numbered architectural argument register and use consecutive registers. For example:

- first `I64`: `r1:r2`
- after one scalar in `r1`, the next `I64` skips to `r3:r4`
- a pair that cannot fit completely in the remaining argument-register area is placed on the stack rather than split between registers and stack

Stack `I64` values are 8-byte aligned and occupy 8 bytes, low word at the lower address.

### Hidden structure return pointer

A hidden structure-return (`sret`) pointer is an ordinary first ABI argument for placement purposes and therefore consumes `r1` when available. User arguments then begin at the next location.

The initial backend does not synthesize arbitrary by-value aggregate calling conventions. Aggregates are passed indirectly or returned through `sret` until a future ABI revision explicitly adds aggregate classification rules.

## Return values

- one scalar integer/pointer result: `r1`
- one `I64` result: `r1:r2`, low word in `r1`, high word in `r2`
- additional return values are assigned by the same ABI machinery to subsequent return locations where representable; excessive returns require Cranelift's stack-return-area mechanism

When Cranelift uses an implicit stack return area, the pointer is carried as a hidden argument according to the same argument-placement rules.

## Volatility

Caller-saved registers:

`r1, r2, r3, r4, r5, r6, r7, r8`

Callee-saved registers:

`r9, r10, r11, r15`

Special registers:

- `r12` is scratch and may be clobbered by backend-generated expansion code.
- `r13` is `sp` and must be restored to its incoming value on normal return.
- `r14` is `lr`; a non-leaf function must preserve the incoming return address before a nested call and restore it before return.
- `r0` is immutable zero.

## Stack

- stack grows downward
- public call-boundary alignment: 8 bytes
- scalar stack slot: 4 bytes
- `I64` stack slot: 8 bytes, aligned to 8 bytes
- no red zone
- no shadow space
- no callee-pop convention; callers and callees obey normal frame ownership
- fixed stack slots and spills are rounded according to the value representation above

The initial backend reserves no mandatory frame record. A frame pointer is optional; when used it is `r15` and must be preserved because `r15` is callee-saved.

## Prologue / epilogue contract

The backend's generic ABI layer computes a frame containing, as needed:

1. outgoing/fixed stack requirements supplied by Cranelift,
2. spill slots,
3. save slots for used callee-saved registers,
4. an `lr` save slot when the function may make a call / otherwise needs the incoming link value preserved.

The prologue:

1. decrements `sp` by the aligned frame size,
2. saves required callee-saved GPRs,
3. saves `lr` when required,
4. performs any enabled stack-limit check using reserved `r12` as the temporary rather than an allocatable register.

The epilogue performs the inverse sequence and returns through the restored `lr`.

Large stack adjustments and out-of-range stack addresses are emitter pseudos and may expand through `r12`; they are not allowed to alter the ABI-visible value of another GPR.

## Calls

- direct and indirect calls use `r14` as the architectural link register.
- call operands/returns follow the register assignments above.
- call clobbers include the caller-saved register set and backend-reserved scratch behavior.
- external/direct-call relocation encoding is part of the emitter/object work and is not yet a completed ABI mechanism at the time this document was written.
- tail calls are not part of the initial ABI.

## Unsupported / deliberately deferred ABI features

The current ABI does not define:

- floating-point register arguments or returns
- vector arguments or returns
- by-value aggregate classification beyond indirect/sret handling
- variadic argument register-save areas
- tail-call ABI
- red zone
- stack probing policy beyond the backend's generic stack-limit hook
- TLS ABI
- unwind/DWARF register numbering contract
- exception ABI

Unsupported cases must fail explicitly; another ISA's ABI must never be used as fallback behavior.

## Stability rule

Changes to any of the following are ABI changes and require deliberate versioning plus Forge/Cosmic conformance updates:

- argument registers
- return registers
- caller/callee-save sets
- `r12/r13/r14/r15` special roles
- stack alignment
- I64 pair ordering/alignment
- hidden `sret` placement
- aggregate classification

Emitter implementation details such as literal-island placement, branch veneers, and the exact instruction sequence used for large immediates are not ABI-visible so long as they preserve this contract.
