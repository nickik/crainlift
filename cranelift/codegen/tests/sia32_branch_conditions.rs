#![cfg(feature = "sia32")]
use cranelift_codegen::Context;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, Signature, UserFuncName,
    types::{I8, I16, I32, I64},
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_control::ControlPlane;
use target_lexicon::Triple;

#[test]
fn branch_conditions_compile_at_all_integer_widths() {
    for ty in [I8, I16, I32, I64] {
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.push(AbiParam::new(ty));
        sig.returns.push(AbiParam::new(I32));
        let mut func = Function::with_name_signature(UserFuncName::testcase("wide_branch"), sig);
        let entry = func.dfg.make_block();
        let yes = func.dfg.make_block();
        let no = func.dfg.make_block();
        let test = func.dfg.append_block_param(entry, ty);
        for block in [entry, yes, no] {
            func.layout.append_block(block);
        }
        let mut pos = FuncCursor::new(&mut func).at_bottom(entry);
        pos.ins().brif(test, yes, &[], no, &[]);
        pos.goto_bottom(yes);
        let value = pos.ins().iconst(I32, 0x57);
        pos.ins().return_(&[value]);
        pos.goto_bottom(no);
        let value = pos.ins().iconst(I32, 0x23);
        pos.ins().return_(&[value]);
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
        if let Some(output) = std::env::var_os("SIA32_BRANCH_CONDITION_OUTPUT") {
            let output = std::path::PathBuf::from(output);
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(output.join(format!("brif-{ty}.text")), text).unwrap();
        }
    }
}
