#![cfg(feature = "sia32")]

use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, Signature, UserFuncName, types::I64};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::{Context, settings};
use cranelift_control::ControlPlane;

#[test]
fn unsigned_i64_high_product_compiles_through_production_sia32() {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.extend([AbiParam::new(I64), AbiParam::new(I64)]);
    sig.returns.push(AbiParam::new(I64));
    let mut func = Function::with_name_signature(UserFuncName::testcase("umulhi"), sig);
    let block = func.dfg.make_block();
    let lhs = func.dfg.append_block_param(block, I64);
    let rhs = func.dfg.append_block_param(block, I64);
    func.layout.append_block(block);
    let mut cursor = FuncCursor::new(&mut func).at_bottom(block);
    let result = cursor.ins().umulhi(lhs, rhs);
    cursor.ins().return_(&[result]);
    let isa = isa::lookup("sia32-unknown-none".parse().unwrap())
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut context = Context::for_function(func);
    let compiled = context
        .compile(&*isa, &mut ControlPlane::default())
        .unwrap();
    let code = compiled.code_buffer();
    assert!(!code.is_empty());
    assert_eq!(code.len() % 2, 0);
    if let Some(path) = std::env::var_os("SIA32_WIDE_UMULHI_TEXT") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, code).unwrap();
    }
}
