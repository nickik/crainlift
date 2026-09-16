# SIA32 M7 backlog

M7 freezes ISA facilities independently of CLIF/ISLE lowering.

## M7 ISA-facilities phase

- Architectural representation for BSET/BCLR/BINV/BEXT, MUL/MULH/MULHU/MULHSU/MULO,
  DIV/DIVU/REM/REMU and REV8.
- Exact encoding and decoding.
- Register-field validation through the architectural 16-register `Reg` type.
- Explicit no-immediate contract for these R2F instructions.
- Stable textual disassembly.
- Pure reference execution semantics including multiply-high, divide-by-zero and signed-overflow edges.
- Golden encoding/decoding, constraint, disassembly and execution tests.

## Deferred to final lowering phase

Do not implement this while M5 ISLE lowering remains parked.

- CLIF `imul` lowering to SIA32 `MUL`.
- Signed/unsigned division and remainder lowering to `DIV`/`DIVU`/`REM`/`REMU`.
- High-half/widening multiply mapping to `MULH`, `MULHU`, and `MULHSU`.
- Decide whether/how `MULO` participates in CLIF overflow-producing operations.
- Copies/temporary-register constraints required by the two-address `rd <- f(rd, rs)` form.
- ISLE rules only after the base M5 ISLE environment is fixed and green.
- End-to-end CLIF -> ISLE -> MachInst -> bytes -> execution tests.

M7 completion does not depend on CLIF lowering.
