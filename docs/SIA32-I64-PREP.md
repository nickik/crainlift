# SIA32 M6-prep: I64 contract

This milestone freezes I64 infrastructure without enabling I64 CLIF lowering.
The SIA32 backend still rejects compilation at its existing lowering boundary.

## Value and register representation

An I64 is two 32-bit words: low word first, high word second. In registers the
low word occupies the lower odd-numbered register and the high word the next
register. Internal allocation is restricted to r1:r2, r3:r4, r5:r6, r7:r8 and
r9:r10. This deliberately excludes r0, r11:r12 (r12 is backend scratch), SP,
LR and FP from pair formation.

The call ABI uses the same ordering. I64 arguments use aligned pairs r1:r2,
r3:r4 and r5:r6. A pair that does not fit moves wholly to the stack; it never
straddles the register/stack boundary. One I64 return uses r1:r2.

## Stack representation

I64 stack slots are 8 bytes, aligned to 8 bytes. At the slot address, bytes
0..3 contain the low word and bytes 4..7 the high word. Spill/reload must use
two 32-bit stores/loads at offsets +0 and +4 and preserve this ordering.

## Multiword arithmetic

64-bit addition is frozen as low ADD, unsigned carry test (`result_low < lhs_low`),
high ADD, then add carry. Subtraction is low SUB, unsigned borrow test
(`lhs_low < rhs_low`), high SUB, then subtract borrow. This requires no hidden
condition-code state and maps onto existing SIA integer operations.

## Expensive operations

Initial lowering will use runtime helpers for operations where a multiword
inline sequence would be large or subtle:

- `__sia32_i64_mul`
- `__sia32_i64_sdiv`
- `__sia32_i64_udiv`
- `__sia32_i64_srem`
- `__sia32_i64_urem`

Each helper receives lhs in r1:r2 and rhs in r3:r4 and returns r1:r2. A later
optimization milestone may replace helpers with inline sequences without
changing the ABI.

## Explicitly deferred

M6-prep does not add ISLE rules, does not lower any I64 CLIF operation, and does
not claim native I64 code generation. Actual I64 CLIF support remains blocked
until the base M5 lowering environment is fixed and the I64 lowering milestone
is implemented and tested end-to-end.
