use cranelift_codegen::isa::sia32::encode;
use cranelift_codegen::isa::sia32::regs::Reg;

#[test]
fn sia32_privileged_encodings_match_m27_contract() {
    let r3 = Reg::new(3).unwrap();
    assert_eq!(encode::sread(r3, encode::SYSREG_EPC).unwrap(), 0xf031);
    assert_eq!(encode::sread(r3, encode::SYSREG_CAUSE).unwrap(), 0xf032);
    assert_eq!(encode::swrite(r3, encode::SYSREG_VMCTX).unwrap(), 0xf135);
    assert_eq!(encode::sswap_scratch(Reg::new(13).unwrap()), 0xf2d4);
    assert_eq!(encode::sret(), 0xf300);
    assert_eq!(encode::sretctx(Reg::new(8).unwrap()), 0xf381);
    assert_eq!(encode::tlbfence(), 0xf400);
    assert_eq!(encode::tlbfence_va(Reg::new(1).unwrap()), 0xf411);
    assert_eq!(encode::tlbfence_asid(Reg::new(2).unwrap()), 0xf422);
    assert_eq!(encode::wfi(), 0xf500);
    assert_eq!(encode::sync_i(), 0xf600);
    assert_eq!(encode::fence(), 0xf700);
}
