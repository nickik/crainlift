# SIA32 Cranelift Backend TODO

## Goal

Add a native 32-bit SIA backend to this Cranelift fork, initially targeting the frozen SIA32-I integer ISA and the current Forge/Cosmic toolchain. The backend must emit real SIA machine code that executes unchanged in LightingSimulation and, later, on the hardware-derived Lighting CPU.

The first implementation is based on the Cranelift revision currently pinned by Forge (`fcd03035697e5f9b68bf674698ae0da128f4f5c7`). Rebase onto newer `crainlift/main` only after the first backend milestone is independently green.

SIA32 facts that drive the design:

- XLEN/address width: 32 bits
- little-endian
- fixed 16-bit instructions
- r0..r15, with r0 hard-wired zero
- ABI architectural conventions already reserved: r13=SP, r14=LR, r15 available as FP/general register
- natural byte/half/word alignment
- no condition-code register
- compare instructions produce `0xffffffff` for true and zero for false
- short branch and literal reaches make relaxation/islands a core backend requirement, not a later optimization

## M0 — Freeze compiler-facing SIA ABI and target identity

- [ ] Write `docs/SIA32-CRANELIFT-ABI.md` before implementing calls.
- [ ] Choose canonical target architecture spelling (`sia32` recommended).
- [ ] Add SIA32 to the forked/patched `target-lexicon` used by both Cranelift and Forge.
- [ ] Define SIA32 as 32-bit pointer width and little-endian.
- [ ] Decide target triple forms for bare-metal and Cosmic, e.g. `sia32-unknown-none` and/or a Cosmic OS component.
- [ ] Ensure Cranelift and Forge use exactly the same `target-lexicon` source so `Triple`/`Architecture` types cannot diverge.
- [ ] Decide whether the default Cranelift calling convention can remain `SystemV` as an identifier or whether a dedicated SIA calling-convention enum value is required.
- [ ] Freeze argument registers.
- [ ] Freeze integer return registers, including two-register 64-bit returns.
- [ ] Freeze caller-saved and callee-saved register sets.
- [ ] Freeze r13 as stack pointer.
- [ ] Freeze r14 link-register semantics across leaf/non-leaf functions.
- [ ] Decide whether r15 is always a frame pointer, optionally a frame pointer, or normally allocatable.
- [ ] Reserve any mandatory backend scratch register(s), especially for long branches, literal loads, stack-address materialization and external calls.
- [ ] Define stack growth direction, stack alignment at call boundaries, local-slot alignment and incoming-argument alignment.
- [ ] Define frame-record format if one exists.
- [ ] Define LR spill/restore rules.
- [ ] Define aggregate argument/return rules and hidden sret pointer rules.
- [ ] Define 64-bit scalar argument alignment and register-pair ordering.
- [ ] Define behavior when an argument spans the last argument register and stack.
- [ ] Decide varargs support; explicitly reject it initially if not required.
- [ ] Decide tail-call support; explicitly disable it initially unless ABI work is complete.
- [ ] Decide red-zone policy; normally none for an OS/kernel-capable SIA ABI.
- [ ] Decide stack probing / large-frame policy.
- [ ] Define function alignment: minimum is naturally 2 bytes; choose preferred alignment based on Lighting fetch behavior.
- [ ] Define the page-size value reported by `TargetIsa::page_size_align_log2` from the SIA/Lighting VM contract rather than copying another backend.
- [ ] Define TLS policy or explicitly leave TLS unsupported for the first backend.
- [ ] Define unwind/DWARF register numbering or explicitly specify no unwind information for the first milestone.

## M1 — Cranelift target plumbing and empty backend scaffold

- [ ] Add Cargo feature `sia32` to `cranelift-codegen`.
- [ ] Add `sia32` to `all-native-arch` only after it compiles reliably in the all-ISA build.
- [ ] Add `pub mod sia32` under the feature gate in `cranelift/codegen/src/isa/mod.rs`.
- [ ] Extend `isa::lookup()` for `Architecture::Sia32`.
- [ ] Add `sia32` to `ALL_ARCHITECTURES`.
- [ ] Add `Isa::Sia32` to `cranelift/codegen/meta/src/isa/mod.rs`.
- [ ] Extend `Isa::from_arch`, `Isa::all`, `Display`, and `define()`.
- [ ] Add `cranelift/codegen/meta/src/isa/sia32.rs` for SIA settings.
- [ ] Add `cranelift/codegen/src/isa/sia32/settings.rs`.
- [ ] Add a SIA ISLE compilation unit in `cranelift/codegen/meta/src/isle.rs`.
- [ ] Add the SIA backend directory and generated-ISLE include plumbing.
- [ ] Implement `Sia32Backend` holding triple, shared flags and SIA flags.
- [ ] Implement `isa_builder()` and constructor.
- [ ] Implement the complete `TargetIsa` trait surface even where the initial answer is deliberately unsupported/false/None.
- [ ] Report pointer type `I32` through the SIA target triple.
- [ ] Return correct endianness through the target triple.
- [ ] Implement `name() == "sia32"`.
- [ ] Implement text-section builder using `MachTextSectionBuilder`.
- [ ] Implement SIA function alignment and page-size reporting.
- [ ] Return false for native FMA/round/SIMD capabilities until actually supported.
- [ ] Make Capstone disassembly return unsupported rather than pretending SIA is another ISA.
- [ ] Implement backend register pretty-printing (`r0`..`r15`, with optional `sp/lr/fp` aliases in debug output).
- [ ] Add compile-only tests that `isa::lookup(sia32 triple)` constructs the backend and frontend config reports 32-bit pointers.

## Dynamic stack allocation / VLA support

- [x] Permit compiler-generated `sia_gpr_read.i32 13` to snapshot architectural SP; `sia_gpr_write ..., 13` already restores/updates SP.
- [x] Add production-pipeline regression proving SP read, runtime arithmetic, SP write and restoration lower together.
- [ ] Add a dedicated non-privileged CLIF stack-allocation abstraction so frontends do not need to name r13 directly.
- [ ] Make fixed stack-slot addressing stable while SP is dynamically displaced (frame-base strategy or equivalent).
- [ ] Define scope/return restoration rules and call interaction before Cosmic C enables general VLA lowering.

## M2 — Register file, machine instruction model and exact encoder

- [ ] Add `sia32/inst/regs.rs` with the physical integer register class.
- [ ] Model r0 as fixed/non-allocatable constant-zero register.
- [ ] Model r13 as fixed SP.
- [ ] Model r14 according to ABI LR allocation policy.
- [ ] Model r15 according to the final FP policy.
- [ ] Ensure allocatable register pressure matches actual hardware; do not expose pseudo-registers as physical registers.
- [ ] Define `Sia32MachineDeps` / register classes required by the generic MachInst pipeline.
- [ ] Define immediate types for signed imm7, branch halfword displacements, LDPC displacement, shifts and system immediates.
- [ ] Define address operands for base, base+index*4, update forms and stack-slot accesses.
- [ ] Define the SIA `Inst` enum for every machine operation the backend can initially emit.
- [ ] Represent pseudo-instructions separately from final encodings where expansion is required.
- [ ] Correctly declare register uses/defs/mods and multi-output instructions such as ADC/SBB.
- [ ] Encode r0 destination escape instructions CLZ/CTZ/CPOP correctly.
- [ ] Encode ADD/MOV/CMOV.
- [ ] Encode arithmetic and comparison operations.
- [ ] Encode logic and all register/immediate shifts.
- [ ] Encode LI/ADDI.
- [ ] Encode scalar LB/LBU/LH/LHU/LW/SB/SH/SW.
- [ ] Encode pre/post-update word forms where selected by the backend.
- [ ] Encode indexed LDA.W/STA.W.
- [ ] Encode LDP/STP/LD4/ST4 if/when used by prologue, epilogue, spills or optimization.
- [ ] Encode LDPC.W exactly.
- [ ] Encode BNZ/DBNZ/B/BL.
- [ ] Encode JALR/JR/CALLR/RET.
- [ ] Encode BSET/BCLR/BINV/BEXT/REV8.
- [ ] Encode ADC/SBB with the architectural `rd != rc` restriction enforced before emission.
- [ ] Encode TRAP/BREAK/NOP.
- [ ] Feature-gate optional SIA-Zmul/SIA-M encodings instead of assuming they are base ISA.
- [ ] Add byte-exact unit tests for each emitted instruction.
- [ ] Differentially check encoder output against LightingSimulation `src/isa.rs`, `siaasm`, and the frozen encoding golden tests.
- [ ] Add negative encoder tests for reserved modes, invalid register groups, invalid aliases and out-of-range immediates.

## M3 — MachInst emission, labels, branches and literal pools

- [ ] Implement `MachInst` for SIA instructions.
- [ ] Implement move recognition and move emission needed by register allocation.
- [ ] Implement spill/reload generation for integer values.
- [ ] Implement stack-slot address generation.
- [ ] Implement instruction sizing; base instructions are fixed 2 bytes but pseudos may expand.
- [ ] Implement final binary emission into `MachBuffer`.
- [ ] Implement local label uses/fixups for BNZ/DBNZ/B/BL.
- [ ] Define exact branch reach in the backend using halfword-scaled SIA displacements.
- [ ] Implement conditional branch inversion/expansion because SIA has BNZ but no base BZ.
- [ ] Implement long conditional branches when BNZ reach is exceeded.
- [ ] Implement long unconditional branches when B reach is exceeded.
- [ ] Implement long calls when BL reach is exceeded.
- [ ] Decide and implement branch veneers versus inline long-branch sequences.
- [ ] Ensure branch relaxation terminates even when adding veneers changes layout.
- [ ] Implement literal-pool / constant-island support for LDPC.W.
- [ ] Respect LDPC.W's short signed word displacement and aligned `(PC+4)` base rule.
- [ ] Ensure literal islands cannot be executed as instructions; branch around islands when needed.
- [ ] Re-run branch-range calculations after island insertion.
- [ ] Define constant materialization strategy for arbitrary 32-bit constants (LI/ADDI sequence versus LDPC literal).
- [ ] Define symbol-address materialization strategy.
- [ ] Add stress tests with functions intentionally larger than every short branch/literal reach.
- [ ] Add tests with multiple interacting islands and branches around them.
- [ ] Add tests at exact positive/negative displacement boundaries and one byte/halfword beyond each boundary.

## M4 — ABI, prologues, epilogues, calls and stack frames

- [ ] Implement the SIA ABI `MachineDeps` and callee machinery patterned structurally on the existing MachInst backends, not by copying RV64 ABI assumptions.
- [ ] Implement argument-location assignment for I8/I16/I32/pointers.
- [ ] Implement register-pair assignment for I64 once I64 policy is available.
- [ ] Implement stack arguments.
- [ ] Implement return-value assignment.
- [ ] Implement hidden sret handling if aggregates require it.
- [ ] Implement caller-saved clobber sets.
- [ ] Implement callee-save discovery, save and restore.
- [ ] Implement LR preservation for non-leaf functions.
- [ ] Implement optional frame-pointer setup/teardown if selected.
- [ ] Implement fixed and dynamic stack slots.
- [ ] Implement SP adjustment for small frames.
- [ ] Implement large-frame SP adjustment when immediate reach is insufficient.
- [ ] Implement accesses to stack slots outside direct immediate/addressing reach.
- [ ] Preserve required stack alignment at every call site.
- [ ] Implement direct local calls and external calls.
- [ ] Implement indirect calls through CALLR/JALR.
- [ ] Implement returns through RET/JALR according to ABI.
- [ ] Implement call-site stack cleanup according to the selected calling convention.
- [ ] Add nested-call, recursion, leaf/non-leaf, many-argument, many-return and spill-across-call tests.
- [ ] Add register-pressure tests specifically because SIA exposes far fewer allocatable GPRs than current native Cranelift targets.

## M5 — Integer CLIF lowering

- [ ] Add `sia32/inst.isle` constructors/extractors.
- [ ] Add `sia32/lower.isle` and generated-lowering integration.
- [ ] Lower integer constants efficiently.
- [ ] Lower iadd/isub with wrapping semantics.
- [ ] Do not use ADDO/SUBO for ordinary wrapping CLIF arithmetic.
- [ ] Lower explicit overflow/trap forms only when their semantics exactly match SIA ADDO/SUBO.
- [ ] Lower bitwise AND/OR/XOR.
- [ ] Lower shifts, including masking semantics for variable counts.
- [ ] Synthesize rotates if CLIF requires them and SIA has no direct rotate.
- [ ] Lower clz/ctz/popcnt to native escapes.
- [ ] Lower byte swap to REV8 where legal.
- [ ] Lower signed/unsigned integer comparisons.
- [ ] Handle the semantic mismatch between SIA's compare mask (`0xffffffff`) and CLIF values that require canonical 0/1 results.
- [ ] Exploit compare masks directly only where subsequent semantics permit it (e.g. control/select patterns), and normalize where observable.
- [ ] Lower select/conditional move safely using CMOV.
- [ ] Lower integer extension/truncation operations.
- [ ] Lower load/store of 8/16/32-bit values with correct signedness.
- [ ] Lower address arithmetic using indexed forms when profitable.
- [ ] Lower stack and global-address accesses.
- [ ] Lower brif/jump/block-parameter transfers.
- [ ] Lower calls, call_indirect and returns.
- [ ] Lower traps and explicit unreachable behavior.
- [ ] Lower memory fences only when SIA architectural semantics exist; otherwise reject rather than silently dropping them.
- [ ] Add CLIF filetests under `cranelift/filetests/filetests/isa/sia32` for every supported lowering family.

## M6 — 64-bit integers on a 32-bit machine

- [ ] Audit the generic Cranelift pipeline for assumptions that legal native integer values are pointer-sized or smaller.
- [ ] Decide representation of I64 as a two-GPR value for SIA.
- [ ] Define pair ordering and alignment constraints.
- [ ] Implement I64 copies/spills/reloads.
- [ ] Implement I64 load/store as two 32-bit words with little-endian ordering.
- [ ] Implement I64 add/sub using ADD plus ADC / SUB plus SBB or equivalent carry sequences.
- [ ] Implement I64 bitwise operations.
- [ ] Implement I64 equality and signed/unsigned comparisons.
- [ ] Implement I64 shifts including cross-word shifts and all boundary counts.
- [ ] Implement I64 rotates if required.
- [ ] Implement I64 multiply strategy (native pieces or libcall).
- [ ] Implement I64 divide/remainder strategy (normally libcalls initially).
- [ ] Implement I64 constants.
- [ ] Implement I64 arguments and returns in ABI register pairs/stack.
- [ ] Add randomized differential tests against host 64-bit arithmetic for all supported I64 operations.
- [ ] Decide I128 policy; normally explicitly unsupported/libcall-lowered unless Forge requires it.

## M7 — Multiply, divide, floating point and vector policy

- [ ] Treat SIA-Zmul/SIA-M as target features, not unconditional instructions.
- [ ] Add ISA flags for the optional multiply/divide extension(s).
- [ ] Lower MUL/MULH/MULHU/MULHSU when the feature is enabled.
- [ ] Lower DIV/DIVU/REM/REMU when enabled while matching architectural divide-by-zero/overflow behavior expected by CLIF.
- [ ] Provide libcalls or explicit unsupported diagnostics when the extension is absent.
- [ ] Decide whether the first SIA target is hard integer-only or soft-float capable.
- [ ] If soft-float is needed, define f32/f64 ABI representation and libcall lowering.
- [ ] Never advertise native FMA/round support without instructions.
- [ ] Explicitly reject unsupported vector/SIMD CLIF types in early milestones.
- [ ] Add negative tests proving unsupported FP/vector code fails deterministically instead of miscompiling.

## M8 — Relocations and object-code contract

- [ ] Define the SIA relocation model before Forge starts emitting linked binaries.
- [ ] Decide whether the first integration uses standard ELF with a project-specific SIA machine identity, a Cosmic object format, or Forge's existing object-emission abstraction.
- [ ] Add SIA relocation variants to `cranelift_codegen::binemit::Reloc` as needed.
- [ ] At minimum cover absolute 32-bit addresses if used.
- [ ] Cover external function calls / long call sequences.
- [ ] Cover PC-relative literal/symbol references if emitted across object boundaries.
- [ ] Define relocation addend and overflow rules.
- [ ] Extend relocation display/debug formatting.
- [ ] Extend Forge's Cranelift relocation translation/object writer for every SIA relocation.
- [ ] Ensure 32-bit relocations never accidentally use host `usize` width.
- [ ] Add object tests that inspect exact relocation offset, kind, addend and symbol.
- [ ] Add link tests with calls/data references crossing object files.
- [ ] Add overflow/too-far tests so the linker/backend reports an error or uses a veneer rather than truncating.

## M9 — SIA32-P / Cosmic privileged operations

- [ ] Keep privileged operations out of ordinary CLIF lowering unless represented intentionally as intrinsics/machine operations.
- [ ] Define compiler intrinsics or Forge machine builtins for SREAD/SWRITE/SSWAP.
- [ ] Add SRET/SRETCTX support for kernel code generation where required.
- [ ] Add TLBFENCE, TLBFENCE.VA and TLBFENCE.ASID intrinsics.
- [ ] Add WFI, SYNC.I and FENCE intrinsics with side-effect/barrier semantics that prevent illegal compiler motion.
- [ ] Mark privileged instructions as having the correct memory/control side effects in MachInst.
- [ ] Test generated privileged sequences in LightingSimulation's SIA32-P execution mode.
- [ ] Keep semihosting TRAP handling external to architectural codegen.

## M10 — Differential execution and correctness hardening

- [ ] Reuse the frozen LightingSimulation SIA encoder/interpreter as the independent executable oracle.
- [ ] For every machine instruction, compare Cranelift encoding to LightingSimulation golden encoding.
- [ ] Execute generated single-operation functions in LightingSimulation and compare results to host/reference semantics.
- [ ] Add seeded randomized CLIF integer programs and differential execution.
- [ ] Exercise all register allocations, especially aliases involving r0/r13/r14/r15.
- [ ] Stress spills with more live values than physical SIA GPRs.
- [ ] Stress nested blocks/critical edges/block parameters.
- [ ] Stress long functions, branch relaxation and literal-island placement.
- [ ] Stress calls with maximum register and stack arguments.
- [ ] Stress recursion and mutually recursive functions.
- [ ] Stress misaligned addresses where CLIF permits traps and verify no partial effects.
- [ ] Verify generated code never emits architecturally reserved SIA encodings.
- [ ] Add deterministic fuzz seeds for every backend bug found.

## M11 — Forge integration

- [ ] Add SIA target selection to `forge-codegen-cranelift`.
- [ ] Point Forge temporarily at the `sia32-backend` Cranelift revision during integration.
- [ ] Make Forge target layout report 32-bit pointers/usize/isize for SIA.
- [ ] Verify Forge ABI/layout is authoritative; do not let CLIF value types become the language layout definition.
- [ ] Compile scalar Forge functions to SIA object/raw code.
- [ ] Compile and run arithmetic/control-flow examples in LightingSimulation.
- [ ] Compile and run memory/array/struct examples.
- [ ] Compile and run cross-function calls.
- [ ] Compile and run existing freestanding/core/arena library tests that do not depend on unsupported FP/vector features.
- [ ] Add SIA as a target to the existing native-vs-interpreter semantic comparison harness.
- [ ] Compile a small no-std Forge program to an image accepted directly by LightingSimulation.
- [ ] Compile the first Cosmic kernel-side routines once SIA32-P intrinsics are available.

## M12 — CI, maintenance and completion gate

- [ ] Add a focused `cargo check`/test job for `cranelift-codegen --features sia32`.
- [ ] Run ISLE generation/checks for SIA.
- [ ] Add SIA filetests to normal filetest discovery.
- [ ] Keep existing x64/AArch64/RISC-V/s390x tests green.
- [ ] Add the SIA feature to `all-native-arch` only once the backend does not break all-arch builds.
- [ ] Add formatting, clippy and no-default-feature combinations relevant to this fork.
- [ ] Add a bounded cross-repository conformance job using a pinned LightingSimulation revision or frozen exported SIA vectors.
- [ ] Avoid making normal Cranelift CI clone mutable external branches; pin all external oracle inputs.
- [ ] Document supported CLIF types and instructions, optional SIA extensions and intentionally unsupported operations.
- [ ] Document the ABI and relocation contract as versioned interfaces shared with Forge/Cosmic.
- [ ] Rebase the completed backend from the Forge-pinned Cranelift commit onto current `crainlift/main`, resolving upstream API drift separately from backend correctness.
- [ ] Update Forge's Cranelift pin only after the rebased SIA backend and the existing ARM64/RISC-V Forge paths are all green.

## Minimum useful milestone

The backend is useful for the first Forge-on-Lighting execution milestone when all of the following are true:

- `sia32` target lookup works with 32-bit pointers.
- exact SIA encoding is independently verified.
- constants, I32 arithmetic/logic/compare, branches, 8/16/32-bit loads/stores and function calls/returns lower correctly.
- register allocation, spills and stack frames work under SIA's small register file.
- branch relaxation and LDPC literal islands work beyond short-range toy functions.
- the SIA ABI is frozen and tested across multiple functions.
- required relocations are consumable by Forge.
- a compiled Forge program can execute in LightingSimulation and match the interpreter/reference result.

I64, optional multiply/divide, soft-float, SIA32-P intrinsics and unwind/debug metadata can be staged after that minimum milestone unless an immediate Cosmic dependency requires one of them earlier.
