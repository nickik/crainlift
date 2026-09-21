//! Exact SIA32-I / SIA32-P instruction encoding helpers.
//!
//! These helpers intentionally mirror the frozen executable contract in
//! LightingSimulation. They validate architectural operand restrictions so
//! later lowering cannot silently emit a reserved or ambiguous encoding.

use super::regs::Reg;

pub const BREAK: u16 = 0xCFEF;
pub const NOP: u16 = 0xCFFF;
pub const EXT_RESERVED: u16 = 0xF000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    ImmediateOutOfRange {
        kind: &'static str,
        value: i32,
        min: i32,
        max: i32,
    },
    InvalidAlias {
        kind: &'static str,
    },
    RegisterGroupWrap {
        kind: &'static str,
        first: u8,
        width: u8,
    },
    UpdatingLoadBaseOverlap {
        kind: &'static str,
        first: u8,
        width: u8,
        base: u8,
    },
    TrapImmediateReserved(u8),
}

#[inline]
const fn r3(primary: u16, a: Reg, b: Reg, c: Reg) -> u16 {
    (primary << 12) | ((a.index() as u16) << 8) | ((b.index() as u16) << 4) | c.index() as u16
}

#[inline]
const fn r2f(primary: u16, a: Reg, b: Reg, function: u8) -> u16 {
    (primary << 12) | ((a.index() as u16) << 8) | ((b.index() as u16) << 4) | function as u16
}

fn signed_range(kind: &'static str, value: i32, min: i32, max: i32) -> Result<(), EncodeError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(EncodeError::ImmediateOutOfRange {
            kind,
            value,
            min,
            max,
        })
    }
}

fn group(kind: &'static str, first: Reg, width: u8) -> Result<(), EncodeError> {
    if first.index() <= 16 - width {
        Ok(())
    } else {
        Err(EncodeError::RegisterGroupWrap {
            kind,
            first: first.index(),
            width,
        })
    }
}

fn updating_load_group(
    kind: &'static str,
    first: Reg,
    width: u8,
    base: Reg,
) -> Result<(), EncodeError> {
    group(kind, first, width)?;
    let first = first.index();
    let base = base.index();
    if (first..first + width).contains(&base) {
        Err(EncodeError::UpdatingLoadBaseOverlap {
            kind,
            first,
            width,
            base,
        })
    } else {
        Ok(())
    }
}

pub const fn add(rd: Reg, ra: Reg, rb: Reg) -> u16 {
    r3(0x0, rd, ra, rb)
}
pub const fn mov(rd: Reg, rs: Reg) -> u16 {
    add(rd, Reg::ZERO, rs)
}
pub const fn clz(rd: Reg, rs: Reg) -> u16 {
    r3(0x0, Reg::ZERO, rd, rs)
}
pub const fn cmov(rd: Reg, rs: Reg, rc: Reg) -> u16 {
    r3(0x1, rd, rs, rc)
}
pub const fn ctz(rd: Reg, rs: Reg) -> u16 {
    r3(0x1, Reg::ZERO, rd, rs)
}
pub const fn lda_w(rd: Reg, rb: Reg, ri: Reg) -> u16 {
    r3(0x2, rd, rb, ri)
}
pub const fn cpop(rd: Reg, rs: Reg) -> u16 {
    r3(0x2, Reg::ZERO, rd, rs)
}
pub const fn sta_w(rs: Reg, rb: Reg, ri: Reg) -> u16 {
    r3(0x3, rs, rb, ri)
}

pub const fn sub(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x0)
}
pub const fn addo(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x1)
}
pub const fn subo(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x2)
}
pub const fn cmpeq(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x3)
}
pub const fn cmplt(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x4)
}
pub const fn cmpltu(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x5)
}
pub const fn min(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x6)
}
pub const fn minu(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x7)
}
pub const fn max(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x8)
}
pub const fn maxu(rd: Reg, rs: Reg) -> u16 {
    r2f(0x4, rd, rs, 0x9)
}

pub const fn and(rd: Reg, rs: Reg) -> u16 {
    r2f(0x5, rd, rs, 0x0)
}
pub const fn or(rd: Reg, rs: Reg) -> u16 {
    r2f(0x5, rd, rs, 0x1)
}
pub const fn xor(rd: Reg, rs: Reg) -> u16 {
    r2f(0x5, rd, rs, 0x2)
}
pub const fn shl(rd: Reg, rs: Reg) -> u16 {
    r2f(0x5, rd, rs, 0x3)
}
pub const fn shr(rd: Reg, rs: Reg) -> u16 {
    r2f(0x5, rd, rs, 0x4)
}
pub const fn sar(rd: Reg, rs: Reg) -> u16 {
    r2f(0x5, rd, rs, 0x5)
}

fn shift_imm(rd: Reg, amount: u8, low_function: u8, high_function: u8) -> Result<u16, EncodeError> {
    if amount >= 32 {
        return Err(EncodeError::ImmediateOutOfRange {
            kind: "shift amount",
            value: amount as i32,
            min: 0,
            max: 31,
        });
    }
    let (nibble, function) = if amount < 16 {
        (amount, low_function)
    } else {
        (amount - 16, high_function)
    };
    let imm = Reg::new(nibble).expect("shift nibble is always a register-sized field");
    Ok(r2f(0x5, rd, imm, function))
}

pub fn shli(rd: Reg, amount: u8) -> Result<u16, EncodeError> {
    shift_imm(rd, amount, 0x6, 0x7)
}
pub fn shri(rd: Reg, amount: u8) -> Result<u16, EncodeError> {
    shift_imm(rd, amount, 0x8, 0x9)
}
pub fn sari(rd: Reg, amount: u8) -> Result<u16, EncodeError> {
    shift_imm(rd, amount, 0xA, 0xB)
}

pub fn li(rd: Reg, imm: i32) -> Result<u16, EncodeError> {
    signed_range("LI imm7", imm, -64, 63)?;
    Ok(0x6000 | ((rd.index() as u16) << 7) | ((imm as u16) & 0x7F))
}

pub fn addi(rd: Reg, imm: i32) -> Result<u16, EncodeError> {
    signed_range("ADDI imm7", imm, -64, 63)?;
    Ok(0x6800 | ((rd.index() as u16) << 7) | ((imm as u16) & 0x7F))
}

pub const fn lb(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x0)
}
pub const fn lbu(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x1)
}
pub const fn lh(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x2)
}
pub const fn lhu(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x3)
}
pub const fn lw(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x4)
}
pub const fn sb(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x5)
}
pub const fn sh(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x6)
}
pub const fn sw(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0x7)
}

pub fn lw_post(rv: Reg, rb: Reg) -> Result<u16, EncodeError> {
    if rv == rb {
        return Err(EncodeError::InvalidAlias {
            kind: "LW [rb]+ requires rv != rb",
        });
    }
    Ok(r2f(0x7, rv, rb, 0xC))
}
pub const fn sw_post(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0xD)
}
pub fn lw_pre(rv: Reg, rb: Reg) -> Result<u16, EncodeError> {
    if rv == rb {
        return Err(EncodeError::InvalidAlias {
            kind: "LW -[rb] requires rv != rb",
        });
    }
    Ok(r2f(0x7, rv, rb, 0xE))
}
pub const fn sw_pre(rv: Reg, rb: Reg) -> u16 {
    r2f(0x7, rv, rb, 0xF)
}

pub fn ldp(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("LDP", first, 2)?;
    Ok(r2f(0x8, first, rb, 0x0))
}
pub fn ldp_post(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    updating_load_group("LDP [rb]+", first, 2, rb)?;
    Ok(r2f(0x8, first, rb, 0x1))
}
pub fn stp(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("STP", first, 2)?;
    Ok(r2f(0x8, first, rb, 0x2))
}
pub fn stp_post(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("STP [rb]+", first, 2)?;
    Ok(r2f(0x8, first, rb, 0x3))
}
pub fn stp_pre(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("STP -[rb]", first, 2)?;
    Ok(r2f(0x8, first, rb, 0x4))
}
pub fn ld4(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("LD4", first, 4)?;
    Ok(r2f(0x8, first, rb, 0x5))
}
pub fn ld4_post(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    updating_load_group("LD4 [rb]+", first, 4, rb)?;
    Ok(r2f(0x8, first, rb, 0x6))
}
pub fn st4(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("ST4", first, 4)?;
    Ok(r2f(0x8, first, rb, 0x7))
}
pub fn st4_post(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("ST4 [rb]+", first, 4)?;
    Ok(r2f(0x8, first, rb, 0x8))
}
pub fn st4_pre(first: Reg, rb: Reg) -> Result<u16, EncodeError> {
    group("ST4 -[rb]", first, 4)?;
    Ok(r2f(0x8, first, rb, 0x9))
}

pub fn ldpc_w(rd: Reg, displacement_words: i32) -> Result<u16, EncodeError> {
    signed_range("LDPC.W disp8 words", displacement_words, -128, 127)?;
    Ok(0x9000 | ((rd.index() as u16) << 8) | ((displacement_words as u16) & 0xFF))
}

pub fn bnz(rs: Reg, displacement_halfwords: i32) -> Result<u16, EncodeError> {
    signed_range("BNZ disp7 halfwords", displacement_halfwords, -64, 63)?;
    Ok(0xA000 | ((rs.index() as u16) << 7) | ((displacement_halfwords as u16) & 0x7F))
}
pub fn dbnz(rs: Reg, displacement_halfwords: i32) -> Result<u16, EncodeError> {
    signed_range("DBNZ disp7 halfwords", displacement_halfwords, -64, 63)?;
    Ok(0xA800 | ((rs.index() as u16) << 7) | ((displacement_halfwords as u16) & 0x7F))
}
pub fn b(displacement_halfwords: i32) -> Result<u16, EncodeError> {
    signed_range("B disp11 halfwords", displacement_halfwords, -1024, 1023)?;
    Ok(0xB000 | ((displacement_halfwords as u16) & 0x7FF))
}
pub fn bl(displacement_halfwords: i32) -> Result<u16, EncodeError> {
    signed_range("BL disp11 halfwords", displacement_halfwords, -1024, 1023)?;
    Ok(0xB800 | ((displacement_halfwords as u16) & 0x7FF))
}

pub const fn jalr(rd: Reg, rb: Reg) -> u16 {
    r2f(0xC, rd, rb, 0x0)
}
pub const fn jr(rb: Reg) -> u16 {
    jalr(Reg::ZERO, rb)
}
pub const fn callr(rb: Reg) -> u16 {
    jalr(Reg::LR, rb)
}
pub const fn ret() -> u16 {
    jalr(Reg::ZERO, Reg::LR)
}
pub const fn bset(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x1)
}
pub const fn bclr(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x2)
}
pub const fn binv(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x3)
}
pub const fn bext(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x4)
}
pub const fn mul(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x5)
}
pub const fn mulh(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x6)
}
pub const fn mulhu(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x7)
}
pub const fn mulhsu(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x8)
}
pub const fn mulo(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0x9)
}
pub const fn div(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0xA)
}
pub const fn divu(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0xB)
}
pub const fn rem(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0xC)
}
pub const fn remu(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0xD)
}
pub const fn rev8(rd: Reg, rs: Reg) -> u16 {
    r2f(0xC, rd, rs, 0xE)
}

pub fn trap(imm: u8) -> Result<u16, EncodeError> {
    if imm > 0xFD {
        return Err(EncodeError::TrapImmediateReserved(imm));
    }
    Ok(0xC00F | ((imm as u16) << 4))
}

pub fn adc(rd: Reg, rs: Reg, carry: Reg) -> Result<u16, EncodeError> {
    if rd == carry {
        return Err(EncodeError::InvalidAlias {
            kind: "ADC requires rd != carry register",
        });
    }
    Ok(r3(0xD, rd, rs, carry))
}
pub fn sbb(rd: Reg, rs: Reg, borrow: Reg) -> Result<u16, EncodeError> {
    if rd == borrow {
        return Err(EncodeError::InvalidAlias {
            kind: "SBB requires rd != borrow register",
        });
    }
    Ok(r3(0xE, rd, rs, borrow))
}

// SIA32-P SYSTEM encodings. Semantic privilege checks belong to execution/lowering;
// these helpers only emit structurally valid architectural forms.
pub const SYSREG_STATUS: u8 = 0;
pub const SYSREG_EPC: u8 = 1;
pub const SYSREG_CAUSE: u8 = 2;
pub const SYSREG_BADADDR: u8 = 3;
pub const SYSREG_SCRATCH: u8 = 4;
pub const SYSREG_VMCTX: u8 = 5;

const fn system(sysop: u8, reg: Reg, selector: u8) -> u16 {
    0xF000 | ((sysop as u16) << 8) | ((reg.index() as u16) << 4) | selector as u16
}

pub fn sread(rd: Reg, selector: u8) -> Result<u16, EncodeError> {
    if selector > SYSREG_VMCTX {
        return Err(EncodeError::ImmediateOutOfRange {
            kind: "SREAD sysreg",
            value: selector as i32,
            min: 0,
            max: SYSREG_VMCTX as i32,
        });
    }
    Ok(system(0x0, rd, selector))
}
pub fn swrite(rs: Reg, selector: u8) -> Result<u16, EncodeError> {
    if selector > SYSREG_VMCTX || selector == SYSREG_CAUSE || selector == SYSREG_BADADDR {
        return Err(EncodeError::InvalidAlias {
            kind: "SWRITE selector is reserved or read-only",
        });
    }
    Ok(system(0x1, rs, selector))
}
pub const fn sswap_scratch(reg: Reg) -> u16 {
    system(0x2, reg, SYSREG_SCRATCH)
}
pub const fn sret() -> u16 {
    system(0x3, Reg::ZERO, 0x0)
}
pub const fn sretctx(rs: Reg) -> u16 {
    system(0x3, rs, 0x1)
}
pub const fn tlbfence() -> u16 {
    system(0x4, Reg::ZERO, 0x0)
}
pub const fn tlbfence_va(rs: Reg) -> u16 {
    system(0x4, rs, 0x1)
}
pub const fn tlbfence_asid(rs: Reg) -> u16 {
    system(0x4, rs, 0x2)
}
pub const fn wfi() -> u16 {
    system(0x5, Reg::ZERO, 0x0)
}
pub const fn sync_i() -> u16 {
    system(0x6, Reg::ZERO, 0x0)
}
pub const fn fence() -> u16 {
    system(0x7, Reg::ZERO, 0x0)
}

/// Convert an encoded halfword to the actual little-endian instruction bytes.
pub const fn to_le_bytes(word: u16) -> [u8; 2] {
    word.to_le_bytes()
}
