#![cfg(feature = "sia32")]
use cranelift_codegen::Context;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, TrapCode, UserFuncName, types::I64,
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_control::ControlPlane;

#[test]
fn unsigned_i64_division_and_remainder_compile_with_zero_trap() {
    for operation in ["udiv", "urem"] {
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.extend([AbiParam::new(I64), AbiParam::new(I64)]);
        sig.returns.push(AbiParam::new(I64));
        let mut func = Function::with_name_signature(UserFuncName::testcase(operation), sig);
        let block = func.dfg.make_block();
        let lhs = func.dfg.append_block_param(block, I64);
        let rhs = func.dfg.append_block_param(block, I64);
        func.layout.append_block(block);
        let mut cursor = FuncCursor::new(&mut func).at_bottom(block);
        let result = if operation == "udiv" {
            cursor.ins().udiv(lhs, rhs)
        } else {
            cursor.ins().urem(lhs, rhs)
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
        let text = compiled.code_buffer();
        assert!(!text.is_empty());
        assert!(
            text.len() < 0xff00,
            "{operation} exceeds Lighting function image: {}",
            text.len()
        );
        assert!(
            compiled
                .buffer
                .traps()
                .iter()
                .any(|trap| trap.code == TrapCode::INTEGER_DIVISION_BY_ZERO)
        );
        eprintln!("{operation}.i64 native code: {} bytes", text.len());
        if let Some(output) = std::env::var_os("SIA32_WIDE_DIVISION_OUTPUT") {
            let output = std::path::PathBuf::from(output);
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(output.join(format!("{operation}.text")), text).unwrap();
        }
    }
}
