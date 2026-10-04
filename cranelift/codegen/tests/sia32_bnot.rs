#![cfg(feature = "sia32")]

use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, UserFuncName,
    types::{I8, I16, I32},
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::{Context, settings};
use cranelift_control::ControlPlane;

#[test]
fn bitwise_not_compiles_through_production_sia32() {
    for ty in [I8, I16, I32] {
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.push(AbiParam::new(I32));
        sig.returns.push(AbiParam::new(I32));
        let mut func = Function::with_name_signature(UserFuncName::testcase("bnot"), sig);
        let block = func.dfg.make_block();
        let lhs = func.dfg.append_block_param(block, I32);
        func.layout.append_block(block);
        let mut cursor = FuncCursor::new(&mut func).at_bottom(block);
        let value = if ty == I32 {
            lhs
        } else {
            cursor.ins().ireduce(ty, lhs)
        };
        let result = cursor.ins().bnot(value);
        let result = if ty == I32 {
            result
        } else {
            cursor.ins().uextend(I32, result)
        };
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
        if let Some(path) = std::env::var_os("SIA32_BNOT_TEXT") {
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let path = if ty == I32 {
                path
            } else {
                path.with_file_name(format!("bnot-{ty}.text"))
            };
            std::fs::write(path, code).unwrap();
        }
    }
}
