#![cfg(feature = "sia32")]

use cranelift_codegen::Context;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, Type, UserFuncName,
    types::{I8, I16, I32, I64},
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_control::ControlPlane;
use target_lexicon::Triple;

#[test]
fn variable_i64_shifts_compile_for_all_integer_count_widths() {
    for count_ty in [I8, I16, I32, I64] {
        for operation in ["ishl", "ushr", "sshr"] {
            let text = compile_shift(count_ty, operation);
            assert!(!text.is_empty());
            assert_eq!(text.len() % 2, 0);
            if let Some(output) = std::env::var_os("SIA32_VARIABLE_SHIFT_OUTPUT") {
                let output = std::path::PathBuf::from(output);
                std::fs::create_dir_all(&output).unwrap();
                std::fs::write(output.join(format!("{operation}-{count_ty}.text")), text).unwrap();
            }
        }
    }
}

fn compile_shift(count_ty: Type, operation: &str) -> Vec<u8> {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(AbiParam::new(I64));
    sig.params.push(AbiParam::new(count_ty));
    sig.returns.push(AbiParam::new(I64));
    let mut func = Function::with_name_signature(UserFuncName::testcase(operation), sig);
    let block = func.dfg.make_block();
    let value = func.dfg.append_block_param(block, I64);
    let count = func.dfg.append_block_param(block, count_ty);
    func.layout.append_block(block);
    let mut pos = FuncCursor::new(&mut func).at_bottom(block);
    let result = match operation {
        "ishl" => pos.ins().ishl(value, count),
        "ushr" => pos.ins().ushr(value, count),
        "sshr" => pos.ins().sshr(value, count),
        _ => unreachable!(),
    };
    pos.ins().return_(&[result]);
    let triple: Triple = "sia32-unknown-none".parse().unwrap();
    let isa = isa::lookup(triple)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut context = Context::for_function(func);
    context
        .compile(&*isa, &mut ControlPlane::default())
        .unwrap()
        .code_buffer()
        .to_vec()
}
