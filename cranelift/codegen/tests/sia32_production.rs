#![cfg(feature = "sia32")]

use cranelift_codegen::Context;
use cranelift_codegen::ir::{Function, InstBuilder, Signature, Type, UserFuncName, Value, types::{I8, I16, I32, I64}};
use cranelift_codegen::isa::{self, CallConv};
use cranelift_codegen::settings;
use cranelift_codegen::cursor::{Cursor, FuncCursor};
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

fn compile_i32_expression(build: impl FnOnce(&mut FuncCursor<'_>) -> Value) -> Result<Vec<u8>, String> {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(I32));
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
        assert_eq!(&code[code.len() - 2..], &[0xe0, 0xc0], "{ty} must emit SIA32 ret");
    }
}

#[test]
fn i64_constant_remains_rejected_by_sia32_m5() {
    let error = compile_iconst(I64, 1).expect_err("SIA32 M5 must not accept I64 lowering");
    assert!(error.contains("implemented in ISLE"), "unexpected I64 failure: {error}");
}

#[test]
fn integer_alu_compiles_through_production_sia32_pipeline() {
    let cases: &[(&str, fn(&mut FuncCursor<'_>) -> Value)] = &[
        ("iadd", |pos| { let a = pos.ins().iconst(I32, 19); let b = pos.ins().iconst(I32, 23); pos.ins().iadd(a, b) }),
        ("isub", |pos| { let a = pos.ins().iconst(I32, 19); let b = pos.ins().iconst(I32, 23); pos.ins().isub(a, b) }),
        ("band", |pos| { let a = pos.ins().iconst(I32, 0x55aa); let b = pos.ins().iconst(I32, 0x0f0f); pos.ins().band(a, b) }),
        ("bor", |pos| { let a = pos.ins().iconst(I32, 0x55aa); let b = pos.ins().iconst(I32, 0x0f0f); pos.ins().bor(a, b) }),
        ("bxor", |pos| { let a = pos.ins().iconst(I32, 0x55aa); let b = pos.ins().iconst(I32, 0x0f0f); pos.ins().bxor(a, b) }),
    ];
    for (name, build) in cases {
        let code = compile_i32_expression(*build)
            .unwrap_or_else(|error| panic!("{name} failed in production SIA32 pipeline: {error}"));
        assert_eq!(&code[code.len() - 2..], &[0xe0, 0xc0], "{name} must emit SIA32 ret");
    }
}

#[test]
fn shifts_and_unary_ops_compile_through_production_sia32_pipeline() {
    let cases: &[(&str, fn(&mut FuncCursor<'_>) -> Value)] = &[
        ("ishl.i32", |pos| { let a = pos.ins().iconst(I32, 3); let b = pos.ins().iconst(I32, 4); pos.ins().ishl(a, b) }),
        ("ushr.i32", |pos| { let a = pos.ins().iconst(I32, -16); let b = pos.ins().iconst(I32, 2); pos.ins().ushr(a, b) }),
        ("sshr.i32", |pos| { let a = pos.ins().iconst(I32, -16); let b = pos.ins().iconst(I32, 2); pos.ins().sshr(a, b) }),
        ("clz.i32", |pos| { let a = pos.ins().iconst(I32, 0x100); pos.ins().clz(a) }),
        ("ctz.i32", |pos| { let a = pos.ins().iconst(I32, 0x100); pos.ins().ctz(a) }),
        ("popcnt.i32", |pos| { let a = pos.ins().iconst(I32, 0x55aa); pos.ins().popcnt(a) }),
    ];
    for (name, build) in cases {
        let code = compile_i32_expression(*build)
            .unwrap_or_else(|error| panic!("{name} failed in production SIA32 pipeline: {error}"));
        assert_eq!(&code[code.len() - 2..], &[0xe0, 0xc0], "{name} must emit SIA32 ret");
    }
}
