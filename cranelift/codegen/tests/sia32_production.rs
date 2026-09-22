#![cfg(feature = "sia32")]

use cranelift_codegen::Context;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    Function, InstBuilder, MemFlagsData, Signature, Type, UserFuncName, Value,
    types::{I8, I16, I32, I64},
};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_control::ControlPlane;
use target_lexicon::Triple;

fn compile_function(func: Function) -> Result<Vec<u8>, String> {
    let triple: Triple = "sia32-unknown-none".parse().unwrap();
    let isa = isa::lookup(triple)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let mut ctx = Context::for_function(func);
    let mut ctrl_plane = ControlPlane::default();
    let compiled = ctx
        .compile(&*isa, &mut ctrl_plane)
        .map_err(|error| error.inner.to_string())?;
    Ok(compiled.code_buffer().to_vec())
}

fn compile_iconst(ty: Type, value: i64) -> Result<Vec<u8>, String> {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(ty));
    let mut func = Function::with_name_signature(UserFuncName::testcase("iconst"), sig);
    let block = func.dfg.make_block();
    func.layout.append_block(block);
    {
        let mut pos = FuncCursor::new(&mut func);
        pos.goto_bottom(block);
        let value = pos.ins().iconst(ty, value);
        pos.ins().return_(&[value]);
    }

    compile_function(func)
}

fn compile_expression(
    return_ty: Type,
    build: impl FnOnce(&mut FuncCursor<'_>) -> Value,
) -> Result<Vec<u8>, String> {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(return_ty));
    let mut func = Function::with_name_signature(UserFuncName::testcase("expr"), sig);
    let block = func.dfg.make_block();
    func.layout.append_block(block);
    {
        let mut pos = FuncCursor::new(&mut func);
        pos.goto_bottom(block);
        let value = build(&mut pos);
        pos.ins().return_(&[value]);
    }
    compile_function(func)
}

fn compile_i32_expression(
    build: impl FnOnce(&mut FuncCursor<'_>) -> Value,
) -> Result<Vec<u8>, String> {
    compile_expression(I32, build)
}

#[test]
fn iconst_i32_compiles_through_production_sia32_pipeline() {
    for value in [0, 1, -1, 42, 0x1234_5678] {
        let code = compile_iconst(I32, value).expect("production SIA32 I32 compilation");
        assert!(!code.is_empty(), "iconst {value:#x} emitted no SIA32 bytes");
        assert_eq!(code.len() % 2, 0, "SIA32 instructions are 16-bit words");
    }
}

#[test]
fn integer_constants_use_production_sia32_lowering() {
    for (ty, value) in [(I8, -1), (I16, 0x1234), (I32, 0x1234_5678)] {
        let code = compile_iconst(ty, value).unwrap_or_else(|error| {
            panic!("{ty} iconst failed in production SIA32 pipeline: {error}")
        });
        assert!(!code.is_empty());
        assert_eq!(code.len() % 2, 0);
        assert_eq!(
            &code[code.len() - 2..],
            &[0xe0, 0xc0],
            "{ty} must emit SIA32 ret"
        );
    }
}

#[test]
fn i64_constant_remains_rejected_by_sia32_m5() {
    let error = compile_iconst(I64, 1).expect_err("SIA32 M5 must not accept I64 lowering");
    assert!(
        error.contains("implemented in ISLE"),
        "unexpected I64 failure: {error}"
    );
}

#[test]
fn integer_alu_compiles_through_production_sia32_pipeline() {
    let cases: &[(&str, fn(&mut FuncCursor<'_>) -> Value)] = &[
        ("iadd", |pos| {
            let a = pos.ins().iconst(I32, 19);
            let b = pos.ins().iconst(I32, 23);
            pos.ins().iadd(a, b)
        }),
        ("isub", |pos| {
            let a = pos.ins().iconst(I32, 19);
            let b = pos.ins().iconst(I32, 23);
            pos.ins().isub(a, b)
        }),
        ("band", |pos| {
            let a = pos.ins().iconst(I32, 0x55aa);
            let b = pos.ins().iconst(I32, 0x0f0f);
            pos.ins().band(a, b)
        }),
        ("bor", |pos| {
            let a = pos.ins().iconst(I32, 0x55aa);
            let b = pos.ins().iconst(I32, 0x0f0f);
            pos.ins().bor(a, b)
        }),
        ("bxor", |pos| {
            let a = pos.ins().iconst(I32, 0x55aa);
            let b = pos.ins().iconst(I32, 0x0f0f);
            pos.ins().bxor(a, b)
        }),
    ];
    for (name, build) in cases {
        let code = compile_i32_expression(*build)
            .unwrap_or_else(|error| panic!("{name} failed in production SIA32 pipeline: {error}"));
        assert_eq!(
            &code[code.len() - 2..],
            &[0xe0, 0xc0],
            "{name} must emit SIA32 ret"
        );
    }
}

#[test]
fn shifts_and_unary_ops_compile_through_production_sia32_pipeline() {
    let cases: &[(&str, fn(&mut FuncCursor<'_>) -> Value)] = &[
        ("ishl.i32", |pos| {
            let a = pos.ins().iconst(I32, 3);
            let b = pos.ins().iconst(I32, 4);
            pos.ins().ishl(a, b)
        }),
        ("ushr.i32", |pos| {
            let a = pos.ins().iconst(I32, -16);
            let b = pos.ins().iconst(I32, 2);
            pos.ins().ushr(a, b)
        }),
        ("sshr.i32", |pos| {
            let a = pos.ins().iconst(I32, -16);
            let b = pos.ins().iconst(I32, 2);
            pos.ins().sshr(a, b)
        }),
        ("clz.i32", |pos| {
            let a = pos.ins().iconst(I32, 0x100);
            pos.ins().clz(a)
        }),
        ("ctz.i32", |pos| {
            let a = pos.ins().iconst(I32, 0x100);
            pos.ins().ctz(a)
        }),
        ("popcnt.i32", |pos| {
            let a = pos.ins().iconst(I32, 0x55aa);
            pos.ins().popcnt(a)
        }),
    ];
    for (name, build) in cases {
        let code = compile_i32_expression(*build)
            .unwrap_or_else(|error| panic!("{name} failed in production SIA32 pipeline: {error}"));
        assert_eq!(
            &code[code.len() - 2..],
            &[0xe0, 0xc0],
            "{name} must emit SIA32 ret"
        );
    }
}

#[test]
fn narrow_integer_conversions_compile_through_production_sia32_pipeline() {
    let cases: &[(&str, Type, fn(&mut FuncCursor<'_>) -> Value)] = &[
        ("uextend.i8", I32, |pos| {
            let v = pos.ins().iconst(I8, -1);
            pos.ins().uextend(I32, v)
        }),
        ("uextend.i16", I32, |pos| {
            let v = pos.ins().iconst(I16, -1);
            pos.ins().uextend(I32, v)
        }),
        ("sextend.i8", I32, |pos| {
            let v = pos.ins().iconst(I8, -1);
            pos.ins().sextend(I32, v)
        }),
        ("sextend.i16", I32, |pos| {
            let v = pos.ins().iconst(I16, -1);
            pos.ins().sextend(I32, v)
        }),
        ("ireduce.i8", I8, |pos| {
            let v = pos.ins().iconst(I32, 0x1234);
            pos.ins().ireduce(I8, v)
        }),
        ("ireduce.i16", I16, |pos| {
            let v = pos.ins().iconst(I32, 0x1234_5678);
            pos.ins().ireduce(I16, v)
        }),
    ];
    for (name, return_ty, build) in cases {
        let code = compile_expression(*return_ty, *build)
            .unwrap_or_else(|error| panic!("{name} failed in production SIA32 pipeline: {error}"));
        assert_eq!(
            &code[code.len() - 2..],
            &[0xe0, 0xc0],
            "{name} must emit SIA32 ret"
        );
    }
}

#[test]
fn scalar_memory_ops_compile_through_production_sia32_pipeline() {
    for ty in [I8, I16, I32] {
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.push(cranelift_codegen::ir::AbiParam::new(I32));
        sig.returns.push(cranelift_codegen::ir::AbiParam::new(ty));
        let mut func = Function::with_name_signature(UserFuncName::testcase("load"), sig);
        let block = func.dfg.make_block();
        let ptr = func.dfg.append_block_param(block, I32);
        func.layout.append_block(block);
        {
            let mut pos = FuncCursor::new(&mut func);
            pos.goto_bottom(block);
            let value = pos.ins().load(ty, MemFlagsData::new(), ptr, 4);
            pos.ins().return_(&[value]);
        }
        let code = compile_function(func).unwrap_or_else(|error| {
            panic!("load {ty} failed in production SIA32 pipeline: {error}")
        });
        assert_eq!(&code[code.len() - 2..], &[0xe0, 0xc0]);
    }

    for ty in [I8, I16, I32] {
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params.push(cranelift_codegen::ir::AbiParam::new(I32));
        sig.returns.push(cranelift_codegen::ir::AbiParam::new(ty));
        let mut func = Function::with_name_signature(UserFuncName::testcase("store"), sig);
        let block = func.dfg.make_block();
        let ptr = func.dfg.append_block_param(block, I32);
        func.layout.append_block(block);
        {
            let mut pos = FuncCursor::new(&mut func);
            pos.goto_bottom(block);
            let value = pos.ins().iconst(ty, 7);
            pos.ins().store(MemFlagsData::new(), value, ptr, 8);
            pos.ins().return_(&[value]);
        }
        let code = compile_function(func).unwrap_or_else(|error| {
            panic!("store {ty} failed in production SIA32 pipeline: {error}")
        });
        assert_eq!(&code[code.len() - 2..], &[0xe0, 0xc0]);
    }
}

#[test]
fn basic_control_flow_compiles_through_production_sia32_pipeline() {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(I32));
    let mut jump_func = Function::with_name_signature(UserFuncName::testcase("jump"), sig.clone());
    let entry = jump_func.dfg.make_block();
    let done = jump_func.dfg.make_block();
    jump_func.layout.append_block(entry);
    jump_func.layout.append_block(done);
    {
        let mut pos = FuncCursor::new(&mut jump_func);
        pos.goto_bottom(entry);
        pos.ins().jump(done, &[]);
        pos.goto_bottom(done);
        let value = pos.ins().iconst(I32, 9);
        pos.ins().return_(&[value]);
    }
    let jump_code = compile_function(jump_func).expect("production SIA32 jump lowering");
    assert_eq!(&jump_code[jump_code.len() - 2..], &[0xe0, 0xc0]);

    let mut branch_func = Function::with_name_signature(UserFuncName::testcase("brif"), sig);
    let entry = branch_func.dfg.make_block();
    let taken = branch_func.dfg.make_block();
    let not_taken = branch_func.dfg.make_block();
    branch_func.layout.append_block(entry);
    branch_func.layout.append_block(taken);
    branch_func.layout.append_block(not_taken);
    {
        let mut pos = FuncCursor::new(&mut branch_func);
        pos.goto_bottom(entry);
        let cond = pos.ins().iconst(I32, 1);
        pos.ins().brif(cond, taken, &[], not_taken, &[]);
        pos.goto_bottom(taken);
        let yes = pos.ins().iconst(I32, 1);
        pos.ins().return_(&[yes]);
        pos.goto_bottom(not_taken);
        let no = pos.ins().iconst(I32, 0);
        pos.ins().return_(&[no]);
    }
    let branch_code = compile_function(branch_func).expect("production SIA32 conditional lowering");
    assert!(
        branch_code.len() > 8,
        "conditional branch must emit both branch paths"
    );
}

#[test]
fn integrated_native_sia32_function_emits_bytes() {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(cranelift_codegen::ir::AbiParam::new(I32));
    sig.params.push(cranelift_codegen::ir::AbiParam::new(I32));
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(I32));
    let mut func = Function::with_name_signature(UserFuncName::testcase("integrated"), sig);
    let entry = func.dfg.make_block();
    let yes = func.dfg.make_block();
    let no = func.dfg.make_block();
    let ptr = func.dfg.append_block_param(entry, I32);
    let condition = func.dfg.append_block_param(entry, I32);
    func.layout.append_block(entry);
    func.layout.append_block(yes);
    func.layout.append_block(no);
    {
        let mut pos = FuncCursor::new(&mut func);
        pos.goto_bottom(entry);
        let loaded = pos.ins().load(I32, MemFlagsData::new(), ptr, 4);
        let increment = pos.ins().iconst(I32, 7);
        let result = pos.ins().iadd(loaded, increment);
        pos.ins().store(MemFlagsData::new(), result, ptr, 8);
        pos.ins().brif(condition, yes, &[], no, &[]);
        pos.goto_bottom(yes);
        pos.ins().return_(&[result]);
        pos.goto_bottom(no);
        let zero = pos.ins().iconst(I32, 0);
        pos.ins().return_(&[zero]);
    }
    let code = compile_function(func).expect("integrated production SIA32 compilation");
    assert!(
        code.len() > 16,
        "integrated SIA32 function emitted too little code"
    );
    assert_eq!(code.len() % 2, 0, "SIA32 native output is word aligned");
    assert!(
        code.windows(2).any(|word| word == [0xe0, 0xc0]),
        "integrated function must emit ret"
    );
}

#[test]
fn sia32_integer_comparisons_compile_to_canonical_booleans() {
    let cases = [
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
    ];

    for cc in cases {
        let code = compile_expression(I8, |pos| {
            let lhs = pos.ins().iconst(I32, 1);
            let rhs = pos.ins().iconst(I32, 0);
            pos.ins().icmp(cc, lhs, rhs)
        })
        .unwrap_or_else(|error| {
            panic!("SIA32 {cc:?} must lower through the production pipeline: {error}")
        });
        assert!(!code.is_empty(), "{cc:?} emitted no code");
        assert_eq!(code.len() % 2, 0, "{cc:?} output must be halfword aligned");
    }
}

#[test]
fn architectural_stack_pointer_can_be_read_and_restored_for_dynamic_alloca() {
    let code = compile_i32_expression(|pos| {
        let saved_sp = pos.ins().sia_gpr_read(13);
        let size = pos.ins().iconst(I32, 32);
        let new_sp = pos.ins().isub(saved_sp, size);
        pos.ins().sia_gpr_write(new_sp, 13);
        pos.ins().sia_gpr_write(saved_sp, 13);
        saved_sp
    })
    .expect("SIA32 compiler stack-pointer access must lower through production pipeline");
    assert!(!code.is_empty());
    assert_eq!(&code[code.len() - 2..], &[0xe0, 0xc0]);
}
