# SIA32 compiler ABI for Cranelift

## Status

This document freezes the initial compiler-facing ABI used by the SIA32 Cranelift backend.

It does not redefine the SIA32 instruction set. The executable instruction contract remains the frozen SIA32-I/SIA32-P contract in LightingSimulation. This document only defines the conventions a compiler needs in order to generate independently linked functions.

The initial ABI is deliberately small and conventional. It is intended for Forge, Cosmic, firmware and freestanding code. Extensions may be added later, but existing argument, return, stack and preservation rules must remain compatible.

## Target identity

Canonical architecture spelling:

```text
sia32
```

Initial canonical target triple:

```text
sia32-unknown-none
```

A future Cosmic-specific OS component may be added to target-lexicon, but it must not change the ABI defined here.

Target properties:

```text
XLEN               32 bits
pointer width       32 bits
endianness          little
instruction width   16 bits fixed
minimum code align  2 bytes
preferred func align 4 bytes
base VM page size   2048 bytes (2 KiB)
```

Cranelift uses `CallConv::SystemV` as the identifier for the default SIA calling convention. This means “the platform System-V-style ABI for this architecture”; it does not imply reuse of another architecture's register assignment.

## Register roles

SIA has sixteen architectural integer registers.

| Register | ABI role | Preservation |
| --- | --- | --- |
| `r0` | architectural zero | fixed, never allocatable |
| `r1` | argument 0 / return low | caller-saved |
| `r2` | argument 1 / return high | caller-saved |
| `r3` | argument 2 | caller-saved |
| `r4` | argument 3 | caller-saved |
| `r5` | argument 4 | caller-saved |
| `r6` | argument 5 | caller-saved |
| `r7` | temporary | caller-saved |
| `r8` | temporary | caller-saved |
| `r9` | saved GPR | callee-saved |
| `r10` | saved GPR | callee-saved |
| `r11` | saved GPR | callee-saved |
| `r12` | backend scratch | reserved, not allocatable |
| `r13` | stack pointer `sp` | fixed |
| `r14` | link register `lr` | special |
| `r15` | saved GPR / optional frame pointer `fp` | callee-saved |

### Backend scratch register

`r12` is reserved for compiler-generated expansions including:

- long branches and calls;
- address materialization;
- literal-pool veneers;
- large stack-frame adjustment/access sequences;
- relocation/linker veneers.

Because it is never exposed to register allocation, a pseudo-instruction may safely expand late without discovering that its scratch register contains a live value.

## Arguments

Integer, pointer and integer-like scalar arguments of 32 bits or less use:

```text
r1 r2 r3 r4 r5 r6
```

in source order.

Values narrower than 32 bits are passed in a full register. The caller extends them according to the function signature:

- unsigned values: zero extension;
- signed values: sign extension;
- booleans: canonical `0` or `1` at an ABI boundary.

This canonical Boolean rule is independent of SIA comparison instructions, which internally produce `0xffffffff` for true.

### 64-bit arguments

A 64-bit integer occupies an aligned register pair:

```text
r1:r2
r3:r4
r5:r6
```

The lower 32 bits occupy the lower-numbered register and the upper 32 bits occupy the higher-numbered register.

A 64-bit value never straddles the register/stack boundary. If the next suitable pair is unavailable, the whole value is passed on the stack.

Once argument assignment has moved to the stack, later arguments are also assigned to the stack for the initial ABI. This keeps call lowering deterministic and avoids register holes that complicate varargs and debugging.

## Return values

A scalar value up to 32 bits returns in:

```text
r1
```

A 64-bit integer returns in:

```text
r1 = low 32 bits
r2 = high 32 bits
```

Small aggregates whose ABI representation is one or two 32-bit integer words may use the same return registers.

Larger aggregates use a hidden structure-return pointer in `r1`. User-visible arguments are shifted right by one argument register. The callee stores the result through that pointer and also returns the same pointer in `r1`.

## Caller- and callee-saved state

Caller-saved:

```text
r1-r8
```

Callee-saved:

```text
r9-r11, r15
```

Fixed/reserved:

```text
r0, r12, r13
```

`r14` is the architectural link register and follows the separate rules below.

There are no floating-point or vector register classes in the initial ABI.

## Link register and calls

`BL` and `CALLR` write the return PC to `r14`.

A leaf function may leave `r14` live and return directly with `RET`.

A function that can execute another call must preserve its incoming `r14` before the first call that could overwrite it. Normally it saves `r14` in its stack frame and restores it before `RET`.

This is a callee responsibility. Callers do not separately preserve `r14` around every call.

Tail calls are not part of the initial ABI/backend milestone.

## Stack

The stack grows toward lower addresses.

At every public function entry and immediately before a call:

```text
sp % 8 == 0
```

The caller owns outgoing stack arguments. The callee owns its local frame and callee-save area.

Stack argument rules:

- 32-bit and narrower scalar stack arguments occupy 4-byte slots;
- 64-bit values are 8-byte aligned and occupy 8 bytes;
- aggregates use their natural ABI alignment, capped at 8 bytes for the initial ABI;
- incoming stack arguments begin at the caller's call-boundary `sp`;
- the caller removes outgoing stack arguments after the call.

There is no red zone.

Natural SIA memory alignment remains mandatory. Compiler-generated stack accesses must therefore satisfy the natural alignment of the access width.

## Stack frames

There is no mandatory linked frame record.

`r15` is normally allocatable as a callee-saved GPR. The backend may reserve it as a frame pointer when a function requires stable frame-relative addressing, for example for dynamic stack allocation or other frame-layout constraints.

When used as a frame pointer, `r15` must preserve its caller value in the ordinary callee-save area.

A typical non-leaf frame is conceptually:

```text
higher addresses

incoming stack arguments
-------------------------  entry sp
saved lr / saved GPRs
local stack slots
spill slots
outgoing call area, if reserved
-------------------------  current sp

lower addresses
```

The exact order of saved registers and local slots is a backend implementation detail and is not an inter-module ABI.

## Large frames

The ABI does not impose an artificial frame-size limit. When an immediate/addressing form cannot reach a stack slot or adjustment, the backend must synthesize the address/adjustment using `r12`.

The first backend does not implement OS-specific stack probing. It must nevertheless fail explicitly rather than truncate an out-of-range displacement.

## Aggregates

Initial aggregate classification is intentionally conservative:

- 1 ABI word: pass/return like one 32-bit scalar;
- 2 ABI words: pass/return like a two-word integer when register alignment permits;
- larger or awkwardly aligned aggregates: pass by address;
- large returns: hidden sret pointer as defined above.

Forge may perform higher-level layout decisions before Cranelift; this ABI defines only the final machine-level convention.

## Variadic functions

C-style varargs are unsupported in the first SIA backend.

The compiler must reject a variadic SIA signature rather than silently using an unstable convention.

## TLS

Thread-local-storage relocations and a TLS ABI are not defined for the first backend. TLS references must be rejected until Cosmic defines the runtime/thread-pointer contract.

## Unwind and debugging

The first backend does not emit architectural unwind information.

DWARF register numbers are provisionally the architectural register numbers `0..15` if/when unwind/debug support is added, but this does not become normative until that support is implemented and tested.

## Floating point and vectors

The initial ABI is integer-only.

Native floating-point and vector arguments/returns are unsupported. A future soft-float ABI may pass bit representations in integer registers, but that is not implied by this document.

## Optional SIA multiply/divide extensions

Presence of SIA-Zmul/SIA-M does not change the calling convention. They are target features affecting instruction selection only.

## Compiler invariants

The backend must preserve these rules regardless of optimization level:

1. `r0` is never allocated.
2. `r12` is always available to late expansion.
3. `r13` is always the stack pointer.
4. non-leaf code preserves the incoming `r14` value.
5. `r9-r11` and `r15` survive a call.
6. the call-boundary stack is 8-byte aligned.
7. ABI booleans are `0`/`1`, even though native SIA compare masks use `0xffffffff`/`0`.
8. target-sized addresses and pointers are exactly 32 bits.
9. no relocation or frame calculation may silently truncate a host-width value to 32 bits.

## Initial unsupported features

The first useful native backend may explicitly reject:

- varargs;
- tail calls;
- TLS;
- unwind emission;
- native floating point;
- vectors/SIMD;
- atomics without a defined SIA architectural primitive.

These omissions do not block ordinary Forge user code or the first Cosmic kernel bring-up.