# SIA32 production-lowering completion plan

This checklist is driven by real Forge-on-Lighting firmware, not synthetic backend coverage alone.

## P0 - firmware execution blockers

- [x] Integer constants and I32 add/sub/and/or/xor.
- [x] Variable I32 shifts.
- [x] CLZ/CTZ/CPOP.
- [x] I8/I16 to I32 extension and I32 to I8/I16 reduction.
- [x] Scalar 8/16/32-bit loads and stores.
- [x] Stack-slot address generation (stack_addr).
- [x] Jump, brif, and return.
- [x] Architectural FENCE lowering for SIA-TSO.
- [x] Direct call lowering through shared Cranelift ABI machinery.
- [x] call_indirect lowering through shared Cranelift ABI machinery.
- [ ] Prove non-leaf LR save/restore with two nested Forge calls.
- [ ] Prove register arguments and I32 return values with Forge putc(console, value).
- [ ] Prove stack arguments beyond register capacity.
- [ ] Prove spills/reloads under register pressure.
- [ ] Prove direct-call relocation survives SIAO32/image emission and Lighting execution.
- [ ] Run firmware_functions.fg through Lighting and observe both diagnostic lines.

## P1 - likely next integer firmware blockers

- [ ] Complete signed/unsigned icmp lowering and canonical 0/1 vs SIA mask handling.
- [ ] Lower select to SIA CMOV where semantics permit; avoid branch diamonds.
- [ ] Add immediate-shift patterns and narrow-width shift semantics.
- [ ] Add integer multiply/divide/rem policy gated by SIA target features.
- [ ] Add explicit trap/unreachable lowering.
- [ ] Audit global/symbol address materialization.
- [ ] Exercise signed and unsigned byte/halfword loads.
- [ ] Exercise nonzero and large stack-slot offsets.
- [ ] Stress branch and LDPC literal reach/relaxation.

## P2 - linked firmware

- [ ] Emit all reachable Forge functions, not only the selected entry.
- [ ] Consume direct-call relocations in the SIAO32 image linker.
- [ ] Add rodata/string literals.
- [ ] Add writable data and zeroed bss.
- [ ] Add global and function-address relocations.
- [ ] Produce one Lighting ROM image from linked SIA sections.
- [ ] Preserve a symbol map for lighting-run diagnostics.

## P3 - systems firmware before Cosmic

- [ ] SIA32-P compiler intrinsics: SREAD/SWRITE/SSWAP.
- [ ] TLBFENCE variants.
- [ ] SYNC.I.
- [ ] Explicit FENCE intrinsic/barrier.
- [ ] WFI.
- [ ] Trap/interrupt entry and SRET/SRETCTX.
- [ ] PLIO MMIO diagnostic firmware.
- [ ] DMA/notification firmware diagnostics.
- [ ] Freestanding Forge core library subset.

## Policy

Do not add Forge-only or Lighting-only shortcuts for a missing CLIF operation. Every blocker reached by firmware should either receive correct production SIA32 lowering plus a focused backend regression, or be explicitly rejected here until its architecture/compiler contract is defined.

The acceptance gate is execution on LightingSimulation with observable results, not merely successful object emission.
