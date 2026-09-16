//! DEC SIA32 backend scaffold.
//!
//! The target is registered and exposes the correct frontend properties before
//! full MachInst lowering lands. Compilation deliberately returns Unsupported
//! instead of silently selecting another ISA.

use crate::dominator_tree::DominatorTree;
use crate::ir::{self, Function, Type};
use crate::isa::{
    Builder as IsaBuilder, FunctionAlignment, IsaFlagsHashKey, OwnedTargetIsa, TargetIsa,
};
use crate::machinst::{CompiledCodeStencil, Reg, TextSectionBuilder};
use crate::result::CodegenResult;
use crate::settings::{self as shared_settings, Flags};
use crate::CodegenError;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use cranelift_control::ControlPlane;
use target_lexicon::{Architecture, Triple};

mod abi_contract;
// The exact encoder and architectural-register model are currently exposed for
// conformance tests while the MachInst layer is being built. Their individual
// helpers are intentionally documented by the SIA architectural reference
// rather than duplicating hundreds of one-line rustdoc comments here.
#[allow(missing_docs)]
pub mod encode;
#[allow(missing_docs)]
pub mod regs;
mod settings;

/// Native SIA32 target backend.
pub struct Sia32Backend {
    triple: Triple,
    flags: shared_settings::Flags,
    isa_flags: settings::Flags,
}

impl Sia32Backend {
    /// Create a SIA32 backend from a target triple and resolved shared/ISA flags.
    pub fn new_with_flags(
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
}

impl TargetIsa for Sia32Backend {
    fn compile_function(
        &self,
        _func: &Function,
        _domtree: &DominatorTree,
        _regalloc_ctx: &mut regalloc2::Ctx,
        _want_disasm: bool,
        _ctrl_plane: &mut ControlPlane,
    ) -> CodegenResult<CompiledCodeStencil> {
        Err(CodegenError::Unsupported(
            "SIA32 target is registered, but MachInst lowering is not implemented yet".into(),
        ))
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

    fn text_section_builder(&self, _num_labeled_funcs: usize) -> Box<dyn TextSectionBuilder> {
        // There is no useful text-section builder until SIA's MachInst and
        // branch-relaxation layer exists. Do not fake another ISA's behavior.
        panic!("SIA32 text-section building requires the MachInst emitter milestone")
    }

    fn function_alignment(&self) -> FunctionAlignment {
        FunctionAlignment {
            minimum: 2,
            preferred: 4,
        }
    }

    fn page_size_align_log2(&self) -> u8 {
        // Lighting's architectural base page is 2 KiB.
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
        // SIA's ABI extension is signature-dependent, so do not impose a
        // blanket sign- or zero-extension here.
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
