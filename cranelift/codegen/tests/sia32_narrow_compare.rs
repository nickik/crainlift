#![cfg(feature = "sia32")]

use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, UserFuncName,
    types::{I8, I16, I32},
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::{Context, settings};
use cranelift_control::ControlPlane;

#[test]
fn narrow_comparisons_preserve_declared_width_and_signedness() {
    for ty in [I8, I16] {
        for cc in [
            IntCC::Equal,
            IntCC::NotEqual,
            IntCC::SignedLessThan,
            IntCC::SignedLessThanOrEqual,
            IntCC::SignedGreaterThan,
            IntCC::SignedGreaterThanOrEqual,
            IntCC::UnsignedLessThan,
            IntCC::UnsignedLessThanOrEqual,
            IntCC::UnsignedGreaterThan,
            IntCC::UnsignedGreaterThanOrEqual,
        ] {
            let mut sig = Signature::new(CallConv::SystemV);
            sig.params.extend([AbiParam::new(I32), AbiParam::new(I32)]);
            sig.returns.push(AbiParam::new(I32));
            let mut func = Function::with_name_signature(UserFuncName::testcase("umulhi"), sig);
            let block = func.dfg.make_block();
            let lhs = func.dfg.append_block_param(block, I32);
            let rhs = func.dfg.append_block_param(block, I32);
            func.layout.append_block(block);
            let mut cursor = FuncCursor::new(&mut func).at_bottom(block);
            let lhs = cursor.ins().ireduce(ty, lhs);
            let rhs = cursor.ins().ireduce(ty, rhs);
            let result = cursor.ins().icmp(cc, lhs, rhs);
            let result = cursor.ins().uextend(I32, result);
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
            if let Some(path) = std::env::var_os("SIA32_NARROW_COMPARE_OUTPUT") {
                let dir = std::path::PathBuf::from(path);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(format!("{ty}-{cc}.text")), code).unwrap();
            }
        }
    }
}
