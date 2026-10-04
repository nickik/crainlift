#![cfg(feature = "sia32")]

use cranelift_codegen::Context;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, Signature, UserFuncName, types::I64};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_control::ControlPlane;
use target_lexicon::Triple;

#[test]
fn i64_bitcounts_compile_through_production_sia32() {
    for operation in ["clz", "ctz", "popcnt"] {
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.push(AbiParam::new(I64));
        sig.returns.push(AbiParam::new(I64));
        let mut func = Function::with_name_signature(UserFuncName::testcase(operation), sig);
        let block = func.dfg.make_block();
        let value = func.dfg.append_block_param(block, I64);
        func.layout.append_block(block);
        let mut pos = FuncCursor::new(&mut func).at_bottom(block);
        let result = match operation {
            "clz" => pos.ins().clz(value),
            "ctz" => pos.ins().ctz(value),
            "popcnt" => pos.ins().popcnt(value),
            _ => unreachable!(),
        };
        pos.ins().return_(&[result]);
        let triple: Triple = "sia32-unknown-none".parse().unwrap();
        let isa = isa::lookup(triple)
            .unwrap()
            .finish(settings::Flags::new(settings::builder()))
            .unwrap();
        let mut context = Context::for_function(func);
        let text = context
            .compile(&*isa, &mut ControlPlane::default())
            .unwrap()
            .code_buffer()
            .to_vec();
        assert!(!text.is_empty());
        assert_eq!(text.len() % 2, 0);
        if let Some(output) = std::env::var_os("SIA32_BITCOUNT_OUTPUT") {
            let output = std::path::PathBuf::from(output);
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(output.join(format!("{operation}.text")), text).unwrap();
        }
    }
}
