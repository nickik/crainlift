#![cfg(feature = "sia32")]

use cranelift_codegen::binemit::Reloc;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{
    AbiParam, ExtFuncData, ExternalName, Function, InstBuilder, Signature, UserExternalName,
    UserFuncName, types::I32,
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::{Context, settings};
use cranelift_control::ControlPlane;

#[test]
fn function_addresses_and_indirect_calls_preserve_symbol_relocations() {
    for colocated in [false, true] {
        for indirect in [false, true] {
            let mut sig = Signature::new(CallConv::SystemV);
            sig.params.push(AbiParam::new(I32));
            sig.returns.push(AbiParam::new(I32));
            let mut func =
                Function::with_name_signature(UserFuncName::testcase("func_addr"), sig.clone());
            let callee_sig = func.import_signature(sig);
            let name = ExternalName::User(func.declare_imported_user_function(UserExternalName {
                namespace: 17,
                index: 23,
            }));
            let callee = func.import_function(ExtFuncData {
                name: name.clone(),
                signature: callee_sig,
                colocated,
                patchable: false,
            });
            let block = func.dfg.make_block();
            let input = func.dfg.append_block_param(block, I32);
            func.layout.append_block(block);
            let mut cursor = FuncCursor::new(&mut func).at_bottom(block);
            let address = cursor.ins().func_addr(I32, callee);
            let output = if indirect {
                let call = cursor.ins().call_indirect(callee_sig, address, &[input]);
                cursor.func.dfg.inst_results(call)[0]
            } else {
                address
            };
            cursor.ins().return_(&[output]);
            let isa = isa::lookup("sia32-unknown-none".parse().unwrap())
                .unwrap()
                .finish(settings::Flags::new(settings::builder()))
                .unwrap();
            let mut context = Context::for_function(func);
            let compiled = context
                .compile(&*isa, &mut ControlPlane::default())
                .unwrap();
            let relocs = compiled.buffer.relocs();
            assert_eq!(relocs.len(), 1);
            assert_eq!(relocs[0].kind, Reloc::Abs4);
            assert_eq!(relocs[0].addend, 0);
            assert_eq!(relocs[0].offset % 4, 0);
            assert_eq!(
                format!("{:?}", relocs[0].target),
                format!("ExternalName({name:?})")
            );
        }
    }
}
