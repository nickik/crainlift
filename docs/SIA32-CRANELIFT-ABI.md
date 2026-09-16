# SIA32 Cranelift ABI

Status: implemented ABI contract on `sia32-backend`. This file documents the behavior in `isa/sia32/abi.rs`, `inst.rs`, and `regs.rs`; it does not describe planned behavior.

## Machine and registers

SIA32 is a 32-bit little-endian target with sixteen integer registers. `r0` is fixed zero. `r13` is the stack pointer, `r14` is the link register, and `r12` is reserved to the backend as a non-allocatable scratch/return temporary. `r15` remains allocatable and is callee-saved; the current ABI does not require a frame pointer.

The regalloc integer set is exactly:

- preferred/caller-saved: `r1..r8`
- non-preferred/callee-saved: `r9..r11`, `r15`
- fixed/non-allocatable: `r0`, `r12`, `r13`, `r14`

Calls clobber `r1..r8`, `r12`, and `r14`. Used `r9..r11` and `r15` are preserved by the callee.

Only the integer register class is implemented. `I8`, `I16`, `I32`, pointers and references use one 32-bit register part. `I64` uses two I32 register parts in low-word/high-word order. Floating point, vectors and I128 are not implemented.

## Calling-convention identity

The backend currently accepts only Cranelift `SystemV`. `SystemV` is used as the compiler calling-convention identifier; the actual placement rules below are SIA32-specific. Other Cranelift conventions are rejected rather than mapped to another architecture's ABI.

## Arguments

Normal scalar arguments are allocated left-to-right to `r1` through `r6`. Once an argument cannot be placed in the remaining argument registers, that argument and all subsequent normal arguments use the stack.

An `I64` requires a complete aligned register pair. Register pairs are `r1:r2`, `r3:r4`, and `r5:r6`; the low 32 bits are in the lower-numbered register. If the next available register is even, it is skipped before allocating an I64 pair. An I64 is never split between registers and stack.

Stack scalar argument slots are 4-byte aligned and occupy 4 bytes. Stack I64 values are 8-byte aligned and occupy 8 bytes, low word at the lower address. The complete stack argument area is rounded to 8 bytes.

Narrow integer extension is signature-driven: `get_ext_mode()` preserves the `ArgumentExtension` requested by CLIF for both register and stack locations.

### Hidden return-area pointer

When Cranelift requests an implicit stack return area (`add_ret_area_ptr`), the hidden pointer is physically assigned to `r1` and formal user-argument allocation starts at `r2`. It is represented as a non-formal ABI argument in Cranelift's metadata.

`StructArgument` is rejected. Aggregates must currently be lowered explicitly by address. The backend does not synthesize aggregate memcpy.

## Returns

The register return area is exactly `r1:r2`.

- one scalar return: `r1`
- two scalar returns: `r1`, then `r2`
- one I64 return: `r1:r2`, low word in `r1`

A return that does not fit completely in `r1:r2` cannot use further registers. If Cranelift's multi-return implicit-sret support is enabled, excess return values use its stack-return-area mechanism; otherwise ABI construction fails explicitly.

The current exception-payload compiler hook names `r1` and `r2`; this is not a separately frozen Cosmic exception ABI.

## Stack and frame layout

The stack grows downward. Call-boundary stack alignment is 8 bytes. There is no red zone or shadow space in the implemented ABI.

Cranelift's generic ABI tracks these frame regions separately: incoming arguments, tail arguments, setup area, callee-save clobber area, fixed frame storage, stack slots, and outgoing arguments. Fixed frame storage and outgoing-argument storage are rounded to 8 bytes.

For a function that makes regular calls, the SIA setup area is exactly 8 bytes. The prologue emits `sp -= 8` and saves incoming `lr` (`r14`) at `[sp + 0]`; the other four bytes are padding. The epilogue reloads `lr` from `[sp + 0]` and emits `sp += 8`.

Used callee-saved registers among `r9`, `r10`, `r11`, `r15` are sorted by register number and saved in a separate clobber area. Its size is `4 * count`, rounded to 8 bytes. Clobber save decrements `sp` by that area and stores registers at offsets `0,4,...`; clobber restore performs the inverse and restores `sp`.

Integer regalloc spills consume one Cranelift spill slot. Stack-address and large-offset details are emitter implementation details and must preserve the layout above.

## Calls and link register

Architectural calls use `r14` as the link register. The ABI marks `r14` clobbered by a call; a function that itself makes regular calls therefore preserves its incoming `lr` in the 8-byte setup area described above.

Direct/external call relocation and long-call encoding are emitter/object-format work and are not yet complete. Tail calls are not implemented by this initial ABI.

## Deliberately unsupported behavior

The implemented ABI deliberately does not provide:

- floating-point or vector arguments/returns
- I128 values
- by-value aggregate classification
- automatic aggregate memcpy
- variadic register-save areas
- tail-call ABI
- stack probing (`gen_probestack` and inline probing currently fail if requested)
- TLS ABI
- unwind/DWARF register numbering
- a separately frozen exception ABI

`r12` is exposed to the backend as the return-value temporary and is reserved from regalloc. Although a stack-limit register hook currently returns `r12`, stack probing/check generation is not implemented and this must not be interpreted as an active stack-limit ABI.

## ABI stability boundary

Changing argument registers, the `r1:r2` return area, caller/callee-save sets, special roles of `r12..r15`, stack alignment, I64 pair ordering/alignment, or hidden return-area-pointer placement is an ABI change and requires a deliberate Forge/Cosmic compatibility update.

Literal-pool placement, branch veneers, constant-building sequences, and other emitter choices are not ABI-visible provided they preserve this contract.
