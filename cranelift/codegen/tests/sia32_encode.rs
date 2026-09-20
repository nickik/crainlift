use cranelift_codegen::isa;
use cranelift_codegen::isa::sia32::{self, encode as e, regs::Reg};
use cranelift_codegen::settings;
use target_lexicon::{Endianness, PointerWidth, Triple};

fn r(index: u8) -> Reg {
    Reg::new(index).unwrap()
}

#[test]
fn sia32_target_lookup_reports_frozen_frontend_properties() {
    let triple: Triple = "sia32-unknown-none".parse().unwrap();
    let builder = isa::lookup(triple).expect("SIA32 backend must be registered");
    let shared = settings::Flags::new(settings::builder());
    let target = builder
        .finish(shared)
        .expect("SIA32 backend must construct");

    assert_eq!(target.name(), "sia32");
    assert_eq!(target.triple().pointer_width(), Ok(PointerWidth::U32));
    assert_eq!(target.triple().endianness(), Ok(Endianness::Little));
    assert_eq!(target.pointer_bits(), 32);
    assert_eq!(target.pointer_bytes(), 4);
    assert_eq!(target.frontend_config().page_size_align_log2, 11);
    assert_eq!(target.function_alignment().minimum, 2);
    assert_eq!(target.function_alignment().preferred, 4);
    assert!(isa::ALL_ARCHITECTURES.contains(&"sia32"));
}

#[test]
fn register_abi_roles_are_frozen() {
    assert!(Reg::new(16).is_none());
    assert!(r(0).is_zero());
    assert_eq!(Reg::SP.index(), 13);
    assert_eq!(Reg::LR.index(), 14);
    assert_eq!(Reg::SCRATCH.index(), 12);
    assert_eq!(sia32::regs::ARG_REGS.map(Reg::index), [1, 2, 3, 4, 5, 6]);
    assert_eq!(sia32::regs::RETURN_LOW.index(), 1);
    assert_eq!(sia32::regs::RETURN_HIGH.index(), 2);
    assert!(r(8).caller_saved());
    assert!(!r(9).caller_saved());
    assert!(r(9).callee_saved());
    assert!(r(15).callee_saved());
    assert!(!r(12).normally_allocatable());
    assert!(r(15).normally_allocatable());
}

#[test]
fn exact_core_encodings_match_sia_reference() {
    assert_eq!(e::add(r(1), r(2), r(3)), 0x0123);
    assert_eq!(e::mov(r(1), r(2)), 0x0102);
    assert_eq!(e::clz(r(4), r(5)), 0x0045);
    assert_eq!(e::cmov(r(1), r(2), r(3)), 0x1123);
    assert_eq!(e::ctz(r(4), r(5)), 0x1045);
    assert_eq!(e::lda_w(r(1), r(2), r(3)), 0x2123);
    assert_eq!(e::cpop(r(4), r(5)), 0x2045);
    assert_eq!(e::sta_w(r(1), r(2), r(3)), 0x3123);

    assert_eq!(e::sub(r(1), r(2)), 0x4120);
    assert_eq!(e::addo(r(1), r(2)), 0x4121);
    assert_eq!(e::subo(r(1), r(2)), 0x4122);
    assert_eq!(e::cmpeq(r(1), r(2)), 0x4123);
    assert_eq!(e::cmplt(r(1), r(2)), 0x4124);
    assert_eq!(e::cmpltu(r(1), r(2)), 0x4125);
    assert_eq!(e::min(r(1), r(2)), 0x4126);
    assert_eq!(e::minu(r(1), r(2)), 0x4127);
    assert_eq!(e::max(r(1), r(2)), 0x4128);
    assert_eq!(e::maxu(r(1), r(2)), 0x4129);

    assert_eq!(e::and(r(1), r(2)), 0x5120);
    assert_eq!(e::or(r(1), r(2)), 0x5121);
    assert_eq!(e::xor(r(1), r(2)), 0x5122);
    assert_eq!(e::shl(r(1), r(2)), 0x5123);
    assert_eq!(e::shr(r(1), r(2)), 0x5124);
    assert_eq!(e::sar(r(1), r(2)), 0x5125);
    assert_eq!(e::shli(r(1), 31).unwrap(), 0x51F7);
    assert_eq!(e::shri(r(1), 0).unwrap(), 0x5108);
    assert_eq!(e::sari(r(1), 16).unwrap(), 0x510B);

    assert_eq!(e::li(r(1), -1).unwrap(), 0x60FF);
    assert_eq!(e::addi(r(2), 63).unwrap(), 0x693F);
}

#[test]
fn exact_memory_and_control_encodings_match_sia_reference() {
    assert_eq!(e::lb(r(1), r(2)), 0x7120);
    assert_eq!(e::lbu(r(1), r(2)), 0x7121);
    assert_eq!(e::lh(r(1), r(2)), 0x7122);
    assert_eq!(e::lhu(r(1), r(2)), 0x7123);
    assert_eq!(e::lw(r(1), r(2)), 0x7124);
    assert_eq!(e::sb(r(1), r(2)), 0x7125);
    assert_eq!(e::sh(r(1), r(2)), 0x7126);
    assert_eq!(e::sw(r(1), r(2)), 0x7127);
    assert_eq!(e::lw_post(r(1), r(2)).unwrap(), 0x712C);
    assert_eq!(e::sw_post(r(1), r(2)), 0x712D);
    assert_eq!(e::lw_pre(r(1), r(2)).unwrap(), 0x712E);
    assert_eq!(e::sw_pre(r(1), r(2)), 0x712F);

    assert_eq!(e::ldp(r(1), r(3)).unwrap(), 0x8130);
    assert_eq!(e::ldp_post(r(1), r(3)).unwrap(), 0x8131);
    assert_eq!(e::stp(r(1), r(3)).unwrap(), 0x8132);
    assert_eq!(e::stp_post(r(1), r(3)).unwrap(), 0x8133);
    assert_eq!(e::stp_pre(r(1), r(3)).unwrap(), 0x8134);
    assert_eq!(e::ld4(r(4), r(8)).unwrap(), 0x8485);
    assert_eq!(e::ld4_post(r(4), r(8)).unwrap(), 0x8486);
    assert_eq!(e::st4(r(4), r(8)).unwrap(), 0x8487);
    assert_eq!(e::st4_post(r(4), r(8)).unwrap(), 0x8488);
    assert_eq!(e::st4_pre(r(4), r(8)).unwrap(), 0x8489);

    assert_eq!(e::ldpc_w(r(1), -1).unwrap(), 0x91FF);
    assert_eq!(e::bnz(r(1), -1).unwrap(), 0xA0FF);
    assert_eq!(e::dbnz(r(1), -1).unwrap(), 0xA8FF);
    assert_eq!(e::b(-1).unwrap(), 0xB7FF);
    assert_eq!(e::bl(-1).unwrap(), 0xBFFF);

    assert_eq!(e::jalr(r(1), r(2)), 0xC120);
    assert_eq!(e::jr(r(2)), 0xC020);
    assert_eq!(e::callr(r(2)), 0xCE20);
    assert_eq!(e::ret(), 0xC0E0);
    assert_eq!(e::bset(r(1), r(2)), 0xC121);
    assert_eq!(e::bclr(r(1), r(2)), 0xC122);
    assert_eq!(e::binv(r(1), r(2)), 0xC123);
    assert_eq!(e::bext(r(1), r(2)), 0xC124);
    assert_eq!(e::mul(r(1), r(2)), 0xC125);
    assert_eq!(e::mulh(r(1), r(2)), 0xC126);
    assert_eq!(e::mulhu(r(1), r(2)), 0xC127);
    assert_eq!(e::mulhsu(r(1), r(2)), 0xC128);
    assert_eq!(e::mulo(r(1), r(2)), 0xC129);
    assert_eq!(e::div(r(1), r(2)), 0xC12A);
    assert_eq!(e::divu(r(1), r(2)), 0xC12B);
    assert_eq!(e::rem(r(1), r(2)), 0xC12C);
    assert_eq!(e::remu(r(1), r(2)), 0xC12D);
    assert_eq!(e::rev8(r(1), r(2)), 0xC12E);

    assert_eq!(e::trap(0).unwrap(), 0xC00F);
    assert_eq!(e::trap(253).unwrap(), 0xCFDF);
    assert_eq!(e::BREAK, 0xCFEF);
    assert_eq!(e::NOP, 0xCFFF);
    assert_eq!(e::adc(r(1), r(2), r(3)).unwrap(), 0xD123);
    assert_eq!(e::sbb(r(1), r(2), r(3)).unwrap(), 0xE123);
    assert_eq!(e::to_le_bytes(0x0123), [0x23, 0x01]);
}

#[test]
fn exact_system_encodings_match_sia32_p() {
    assert_eq!(e::sread(r(1), e::SYSREG_STATUS).unwrap(), 0xF010);
    assert_eq!(e::swrite(r(2), e::SYSREG_VMCTX).unwrap(), 0xF125);
    assert_eq!(e::sswap_scratch(r(3)), 0xF234);
    assert_eq!(e::sret(), 0xF300);
    assert_eq!(e::sretctx(r(4)), 0xF341);
    assert_eq!(e::tlbfence(), 0xF400);
    assert_eq!(e::tlbfence_va(r(5)), 0xF451);
    assert_eq!(e::tlbfence_asid(r(5)), 0xF452);
    assert_eq!(e::wfi(), 0xF500);
    assert_eq!(e::sync_i(), 0xF600);
    assert_eq!(e::fence(), 0xF700);
}

#[test]
fn immediate_boundaries_are_exact() {
    assert!(e::li(r(1), -64).is_ok());
    assert!(e::li(r(1), 63).is_ok());
    assert!(e::li(r(1), -65).is_err());
    assert!(e::li(r(1), 64).is_err());

    assert!(e::shli(r(1), 31).is_ok());
    assert!(e::shli(r(1), 32).is_err());

    assert!(e::ldpc_w(r(1), -128).is_ok());
    assert!(e::ldpc_w(r(1), 127).is_ok());
    assert!(e::ldpc_w(r(1), -129).is_err());
    assert!(e::ldpc_w(r(1), 128).is_err());

    assert!(e::bnz(r(1), -64).is_ok());
    assert!(e::bnz(r(1), 63).is_ok());
    assert!(e::bnz(r(1), -65).is_err());
    assert!(e::bnz(r(1), 64).is_err());

    assert!(e::b(-1024).is_ok());
    assert!(e::b(1023).is_ok());
    assert!(e::b(-1025).is_err());
    assert!(e::b(1024).is_err());
}

#[test]
fn illegal_aliases_and_groups_are_rejected() {
    assert!(e::trap(254).is_err());
    assert!(e::trap(255).is_err());
    assert!(e::adc(r(1), r(2), r(1)).is_err());
    assert!(e::sbb(r(1), r(2), r(1)).is_err());
    assert!(e::lw_post(r(2), r(2)).is_err());
    assert!(e::lw_pre(r(2), r(2)).is_err());
    assert!(e::ldp(r(15), r(1)).is_err());
    assert!(e::ld4(r(13), r(1)).is_err());
    assert!(e::ldp_post(r(2), r(3)).is_err());
    assert!(e::ld4_post(r(4), r(7)).is_err());
    assert!(e::swrite(r(1), e::SYSREG_CAUSE).is_err());
    assert!(e::swrite(r(1), e::SYSREG_BADADDR).is_err());

    assert!(e::ldp(r(14), r(1)).is_ok());
    assert!(e::ld4(r(12), r(1)).is_ok());
}

#[test]
fn firmware_reserved_scratch_immediate_sequence_is_architecturally_valid() {
    use cranelift_codegen::isa::sia32::{encode, regs::Reg};
    let r12 = Reg::new(12).unwrap();
    assert_eq!(encode::shli(r12, 7).unwrap(), 0x5c76);
    assert_eq!(encode::addi(r12, 8).unwrap(), 0x6e08);
}
