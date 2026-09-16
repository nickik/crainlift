#![cfg(feature = "sia32")]

use cranelift_codegen::Context;
use cranelift_codegen::ir::{Function, InstBuilder, Signature, UserFuncName, types::I32};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_control::ControlPlane;
use target_lexicon::Triple;

fn compile_iconst(value: i64) -> Vec<u8> {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(I32));
    let mut func = Function::with_name_signature(UserFuncName::testcase("iconst"), sig);
    let block = func.dfg.make_block();
    func.layout.append_block(block);
    {
        let mut pos = FuncCursor::new(&mut func);
        pos.goto_bottom(block);
        let value = pos.ins().iconst(I32, value);
        pos.ins().return_(&[value]);
    }

    let triple: Triple = "sia32-unknown-none".parse().unwrap();
    let isa = isa::lookup(triple)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut ctx = Context::for_function(func);
    let mut ctrl_plane = ControlPlane::default();
    let compiled = ctx.compile(&*isa, &mut ctrl_plane).expect("production SIA32 compilation");
    compiled.code_buffer().to_vec()
}

#[test]
fn iconst_i32_compiles_through_production_sia32_pipeline() {
    for value in [0, 1, -1, 42, 0x1234_5678] {
        let code = compile_iconst(value);
        assert!(!code.is_empty(), "iconst {value:#x} emitted no SIA32 bytes");
        assert_eq!(code.len() % 2, 0, "SIA32 instructions are 16-bit words");
    }
}
