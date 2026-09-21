//! DEC SIA32 native Cranelift backend.

use crate::dominator_tree::DominatorTree;
use crate::ir::{self, Function, Type};
use crate::isa::{
    Builder as IsaBuilder, FunctionAlignment, IsaFlagsHashKey, OwnedTargetIsa, TargetIsa,
};
use crate::machinst::CompiledCode;
use crate::machinst::{
    CompiledCodeStencil, MachInst, MachTextSectionBuilder, Reg, SigSet, TextSectionBuilder, VCode,
    compile,
};
use crate::result::CodegenResult;
use crate::settings::{self as shared_settings, Flags};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use cranelift_control::ControlPlane;
use target_lexicon::{Architecture, Triple};

mod abi;
mod abi_contract;
#[allow(missing_docs)]
pub mod encode;
mod inst;
mod inst_abi_compat;
mod label;
mod lower;
#[allow(missing_docs)]
pub mod regs;
mod settings;

/// Native SIA32 target backend. Consumers obtain it through `isa_builder` as a
/// `TargetIsa`; the concrete MachInst type is intentionally an implementation
/// detail of cranelift-codegen.
pub(crate) struct Sia32Backend {
    triple: Triple,
    flags: shared_settings::Flags,
    isa_flags: settings::Flags,
}

impl Sia32Backend {
    /// Create a SIA32 backend from a target triple and resolved shared/ISA flags.
    pub(crate) fn new_with_flags(
        triple: Triple,
        flags: shared_settings::Flags,
        isa_flags: settings::Flags,
    ) -> Self {
        Self {
            triple,
            flags,
            isa_flags,
        }
    }

    /// Lower CLIF to SIA32 VCode, run register allocation, lay out blocks and
    /// finalize local branches. Binary emission happens in `compile_function`.
    fn compile_vcode(
        &self,
        func: &Function,
        domtree: &DominatorTree,
        regalloc_ctx: &mut regalloc2::Ctx,
        ctrl_plane: &mut ControlPlane,
    ) -> CodegenResult<VCode<inst::Inst>> {
        let sigs = SigSet::new::<abi::Sia32MachineDeps>(func, &self.flags)?;
        let abi = abi::Sia32Callee::new(func, self, &self.isa_flags, &sigs)?;
        compile::compile::<Sia32Backend>(
            func,
            domtree,
            regalloc_ctx,
            self,
            abi,
            (),
            sigs,
            ctrl_plane,
        )
    }
}

impl TargetIsa for Sia32Backend {
    fn compile_function(
        &self,
        func: &Function,
        domtree: &DominatorTree,
        regalloc_ctx: &mut regalloc2::Ctx,
        want_disasm: bool,
        ctrl_plane: &mut ControlPlane,
    ) -> CodegenResult<CompiledCodeStencil> {
        let vcode = self.compile_vcode(func, domtree, regalloc_ctx, ctrl_plane)?;
        let want_disasm = want_disasm || log::log_enabled!(log::Level::Debug);
        let emit_result = vcode.emit(&regalloc_ctx.output, want_disasm, &self.flags, ctrl_plane)?;
        let value_labels_ranges = emit_result.value_labels_ranges;
        let buffer = emit_result.buffer;

        if let Some(disasm) = emit_result.disasm.as_ref() {
            log::debug!("disassembly:\n{disasm}");
        }

        Ok(CompiledCodeStencil(CompiledCode {
            buffer,
            vcode: emit_result.disasm,
            value_labels_ranges,
            bb_starts: emit_result.bb_offsets,
            bb_edges: emit_result.bb_edges,
        }))
    }

    fn name(&self) -> &'static str {
        "sia32"
    }
    fn dynamic_vector_bytes(&self, _dynamic_ty: ir::Type) -> u32 {
        0
    }
    fn triple(&self) -> &Triple {
        &self.triple
    }
    fn flags(&self) -> &shared_settings::Flags {
        &self.flags
    }
    fn isa_flags(&self) -> Vec<shared_settings::Value> {
        self.isa_flags.iter().collect()
    }
    fn isa_flags_hash_key(&self) -> IsaFlagsHashKey<'_> {
        IsaFlagsHashKey(self.isa_flags.hash_key())
    }

    #[cfg(feature = "unwind")]
    fn emit_unwind_info(
        &self,
        _result: &crate::machinst::CompiledCode,
        _kind: crate::isa::unwind::UnwindInfoKind,
    ) -> CodegenResult<Option<crate::isa::unwind::UnwindInfo>> {
        Ok(None)
    }

    fn text_section_builder(&self, num_funcs: usize) -> Box<dyn TextSectionBuilder> {
        Box::new(MachTextSectionBuilder::<inst::Inst>::new(num_funcs))
    }

    fn function_alignment(&self) -> FunctionAlignment {
        <inst::Inst as MachInst>::function_alignment()
    }

    fn page_size_align_log2(&self) -> u8 {
        11
    }

    fn pretty_print_reg(&self, reg: Reg, _size: u8) -> String {
        match reg.to_real_reg() {
            Some(real) => match real.hw_enc() {
                13 => "sp".into(),
                14 => "lr".into(),
                n if n < 16 => format!("r{n}"),
                _ => format!("{reg:?}"),
            },
            None => format!("{reg:?}"),
        }
    }

    fn has_native_fma(&self) -> bool {
        false
    }
    fn has_round(&self) -> bool {
        false
    }
    fn has_blendv_lowering(&self, _ty: Type) -> bool {
        false
    }
    fn has_x86_pshufb_lowering(&self) -> bool {
        false
    }
    fn has_x86_pmulhrsw_lowering(&self) -> bool {
        false
    }
    fn has_x86_pmaddubsw_lowering(&self) -> bool {
        false
    }

    fn default_argument_extension(&self) -> ir::ArgumentExtension {
        ir::ArgumentExtension::None
    }
}

impl fmt::Display for Sia32Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MachBackend")
            .field("name", &self.name())
            .field("triple", &self.triple())
            .field("flags", &format!("{}", self.flags()))
            .finish()
    }
}

/// Create an ISA builder for a SIA32 target triple.
pub fn isa_builder(triple: Triple) -> IsaBuilder {
    match triple.architecture {
        Architecture::Sia32 => {}
        _ => unreachable!(),
    }
    IsaBuilder {
        triple,
        setup: settings::builder(),
        constructor: isa_constructor,
    }
}

fn isa_constructor(
    triple: Triple,
    shared_flags: Flags,
    builder: &shared_settings::Builder,
) -> CodegenResult<OwnedTargetIsa> {
    let isa_flags = settings::Flags::new(&shared_flags, builder);
    Ok(Sia32Backend::new_with_flags(triple, shared_flags, isa_flags).wrapped())
}
