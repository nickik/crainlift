//! Compatibility wrapper around target-lexicon 0.13.5 with SIA32 support.
//!
//! Existing target behavior is delegated to the exact upstream 0.13.5 crate;
//! only the new `sia32` architecture is defined locally.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std as alloc;

use alloc::borrow::Cow;
use alloc::format;
use core::fmt;
use core::str::FromStr;

pub use target_lexicon_upstream::{
    Aarch64Architecture, ArmArchitecture, BinaryFormat, CDataModel, CallingConvention,
    CleverArchitecture, CustomVendor, DeploymentTarget, Endianness, Environment, Mips32Architecture,
    Mips64Architecture, OperatingSystem, ParseError, PointerWidth, Riscv32Architecture,
    Riscv64Architecture, Size, Vendor, X86_32Architecture,
};
#[cfg(feature = "arch_z80")]
pub use target_lexicon_upstream::Z80Architecture;

/// Target architecture, extended with DEC SIA32.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Architecture {
    Unknown,
    Arm(ArmArchitecture),
    AmdGcn,
    Aarch64(Aarch64Architecture),
    Asmjs,
    Avr,
    Bpfeb,
    Bpfel,
    Hexagon,
    X86_32(X86_32Architecture),
    M68k,
    LoongArch64,
    Mips32(Mips32Architecture),
    Mips64(Mips64Architecture),
    Msp430,
    Nvptx64,
    Pulley32,
    Pulley64,
    Pulley32be,
    Pulley64be,
    Powerpc,
    Powerpc64,
    Powerpc64le,
    Riscv32(Riscv32Architecture),
    Riscv64(Riscv64Architecture),
    /// DEC Simple Instruction Architecture, 32-bit profile.
    Sia32,
    S390x,
    Sparc,
    Sparc64,
    Sparcv9,
    Wasm32,
    Wasm64,
    X86_64,
    X86_64h,
    XTensa,
    Clever(CleverArchitecture),
    #[cfg(feature = "arch_zkasm")]
    ZkAsm,
    #[cfg(feature = "arch_z80")]
    Z80(Z80Architecture),
}

impl Architecture {
    fn from_upstream(a: target_lexicon_upstream::Architecture) -> Self {
        use target_lexicon_upstream::Architecture as U;
        match a {
            U::Unknown => Self::Unknown,
            U::Arm(v) => Self::Arm(v),
            U::AmdGcn => Self::AmdGcn,
            U::Aarch64(v) => Self::Aarch64(v),
            U::Asmjs => Self::Asmjs,
            U::Avr => Self::Avr,
            U::Bpfeb => Self::Bpfeb,
            U::Bpfel => Self::Bpfel,
            U::Hexagon => Self::Hexagon,
            U::X86_32(v) => Self::X86_32(v),
            U::M68k => Self::M68k,
            U::LoongArch64 => Self::LoongArch64,
            U::Mips32(v) => Self::Mips32(v),
            U::Mips64(v) => Self::Mips64(v),
            U::Msp430 => Self::Msp430,
            U::Nvptx64 => Self::Nvptx64,
            U::Pulley32 => Self::Pulley32,
            U::Pulley64 => Self::Pulley64,
            U::Pulley32be => Self::Pulley32be,
            U::Pulley64be => Self::Pulley64be,
            U::Powerpc => Self::Powerpc,
            U::Powerpc64 => Self::Powerpc64,
            U::Powerpc64le => Self::Powerpc64le,
            U::Riscv32(v) => Self::Riscv32(v),
            U::Riscv64(v) => Self::Riscv64(v),
            U::S390x => Self::S390x,
            U::Sparc => Self::Sparc,
            U::Sparc64 => Self::Sparc64,
            U::Sparcv9 => Self::Sparcv9,
            U::Wasm32 => Self::Wasm32,
            U::Wasm64 => Self::Wasm64,
            U::X86_64 => Self::X86_64,
            U::X86_64h => Self::X86_64h,
            U::XTensa => Self::XTensa,
            U::Clever(v) => Self::Clever(v),
            #[cfg(feature = "arch_zkasm")]
            U::ZkAsm => Self::ZkAsm,
            #[cfg(feature = "arch_z80")]
            U::Z80(v) => Self::Z80(v),
            #[allow(unreachable_patterns)]
            _ => Self::Unknown,
        }
    }

    fn to_upstream(self) -> Option<target_lexicon_upstream::Architecture> {
        use target_lexicon_upstream::Architecture as U;
        Some(match self {
            Self::Sia32 => return None,
            Self::Unknown => U::Unknown,
            Self::Arm(v) => U::Arm(v),
            Self::AmdGcn => U::AmdGcn,
            Self::Aarch64(v) => U::Aarch64(v),
            Self::Asmjs => U::Asmjs,
            Self::Avr => U::Avr,
            Self::Bpfeb => U::Bpfeb,
            Self::Bpfel => U::Bpfel,
            Self::Hexagon => U::Hexagon,
            Self::X86_32(v) => U::X86_32(v),
            Self::M68k => U::M68k,
            Self::LoongArch64 => U::LoongArch64,
            Self::Mips32(v) => U::Mips32(v),
            Self::Mips64(v) => U::Mips64(v),
            Self::Msp430 => U::Msp430,
            Self::Nvptx64 => U::Nvptx64,
            Self::Pulley32 => U::Pulley32,
            Self::Pulley64 => U::Pulley64,
            Self::Pulley32be => U::Pulley32be,
            Self::Pulley64be => U::Pulley64be,
            Self::Powerpc => U::Powerpc,
            Self::Powerpc64 => U::Powerpc64,
            Self::Powerpc64le => U::Powerpc64le,
            Self::Riscv32(v) => U::Riscv32(v),
            Self::Riscv64(v) => U::Riscv64(v),
            Self::S390x => U::S390x,
            Self::Sparc => U::Sparc,
            Self::Sparc64 => U::Sparc64,
            Self::Sparcv9 => U::Sparcv9,
            Self::Wasm32 => U::Wasm32,
            Self::Wasm64 => U::Wasm64,
            Self::X86_64 => U::X86_64,
            Self::X86_64h => U::X86_64h,
            Self::XTensa => U::XTensa,
            Self::Clever(v) => U::Clever(v),
            #[cfg(feature = "arch_zkasm")]
            Self::ZkAsm => U::ZkAsm,
            #[cfg(feature = "arch_z80")]
            Self::Z80(v) => U::Z80(v),
        })
    }

    fn proxy_upstream(self) -> target_lexicon_upstream::Architecture {
        self.to_upstream().unwrap_or(target_lexicon_upstream::Architecture::Riscv32(
            Riscv32Architecture::Riscv32,
        ))
    }

    /// Return architecture endianness.
    pub fn endianness(self) -> Result<Endianness, ()> {
        if self == Self::Sia32 {
            Ok(Endianness::Little)
        } else {
            self.to_upstream().ok_or(())?.endianness()
        }
    }

    /// Return architecture pointer width.
    pub fn pointer_width(self) -> Result<PointerWidth, ()> {
        if self == Self::Sia32 {
            Ok(PointerWidth::U32)
        } else {
            self.to_upstream().ok_or(())?.pointer_width()
        }
    }

    /// Return whether this is a Clever architecture.
    pub fn is_clever(&self) -> bool {
        matches!(self, Self::Clever(_))
    }

    /// Return the canonical architecture spelling.
    pub fn into_str(self) -> Cow<'static, str> {
        if self == Self::Sia32 {
            Cow::Borrowed("sia32")
        } else {
            self.proxy_upstream().into_str()
        }
    }

    /// Return the current host architecture.
    pub fn host() -> Self {
        Self::from_upstream(target_lexicon_upstream::Architecture::host())
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.into_str())
    }
}

impl FromStr for Architecture {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "sia32" {
            Ok(Self::Sia32)
        } else {
            target_lexicon_upstream::Architecture::from_str(s).map(Self::from_upstream)
        }
    }
}

/// LLVM-style target triple extended with SIA32.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Triple {
    pub architecture: Architecture,
    pub vendor: Vendor,
    pub operating_system: OperatingSystem,
    pub environment: Environment,
    pub binary_format: BinaryFormat,
}

impl Triple {
    fn from_upstream(t: target_lexicon_upstream::Triple) -> Self {
        Self {
            architecture: Architecture::from_upstream(t.architecture),
            vendor: t.vendor,
            operating_system: t.operating_system,
            environment: t.environment,
            binary_format: t.binary_format,
        }
    }

    fn to_upstream_proxy(&self) -> target_lexicon_upstream::Triple {
        target_lexicon_upstream::Triple {
            architecture: self.architecture.proxy_upstream(),
            vendor: self.vendor.clone(),
            operating_system: self.operating_system,
            environment: self.environment,
            binary_format: self.binary_format,
        }
    }

    /// Return an all-unknown triple.
    pub fn unknown() -> Self {
        Self::from_upstream(target_lexicon_upstream::Triple::unknown())
    }

    /// Return the current host triple.
    pub fn host() -> Self {
        Self::from_upstream(target_lexicon_upstream::Triple::host())
    }

    /// Return target endianness.
    pub fn endianness(&self) -> Result<Endianness, ()> {
        self.architecture.endianness()
    }

    /// Return target pointer width.
    pub fn pointer_width(&self) -> Result<PointerWidth, ()> {
        match self.environment {
            Environment::Gnux32 | Environment::GnuIlp32 => Ok(PointerWidth::U32),
            _ => self.architecture.pointer_width(),
        }
    }

    /// Return the default calling convention.
    pub fn default_calling_convention(&self) -> Result<CallingConvention, ()> {
        if self.architecture == Architecture::Sia32 {
            Ok(CallingConvention::SystemV)
        } else {
            self.to_upstream_proxy().default_calling_convention()
        }
    }

    /// Return the C data model for the target.
    pub fn data_model(&self) -> Result<CDataModel, ()> {
        if self.architecture == Architecture::Sia32 {
            Ok(CDataModel::ILP32)
        } else {
            self.to_upstream_proxy().data_model()
        }
    }
}

impl FromStr for Triple {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let is_sia = s == "sia32" || s.starts_with("sia32-");
        if !is_sia {
            return target_lexicon_upstream::Triple::from_str(s).map(Self::from_upstream);
        }

        // Let upstream 0.13.5 parse every non-architecture field so SIA follows
        // exactly the same vendor/OS/environment/binary-format grammar.
        let proxy = format!("riscv32{}", &s["sia32".len()..]);
        let mut triple = Self::from_upstream(target_lexicon_upstream::Triple::from_str(&proxy)?);
        triple.architecture = Architecture::Sia32;
        Ok(triple)
    }
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let proxy = self.to_upstream_proxy().to_string();
        if self.architecture == Architecture::Sia32 {
            if let Some(rest) = proxy.strip_prefix("riscv32") {
                return write!(f, "sia32{rest}");
            }
        }
        f.write_str(&proxy)
    }
}

/// Wrapper whose default is the host triple.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DefaultToHost(pub Triple);
impl Default for DefaultToHost {
    fn default() -> Self { Self(Triple::host()) }
}

/// Wrapper whose default is an unknown triple.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DefaultToUnknown(pub Triple);
impl Default for DefaultToUnknown {
    fn default() -> Self { Self(Triple::unknown()) }
}

/// Convenient target-triple literal syntax.
#[macro_export]
macro_rules! triple {
    ($str:tt) => {
        <$crate::Triple as core::str::FromStr>::from_str($str).expect("invalid triple literal")
    };
}

#[cfg(feature = "serde_support")]
impl serde::Serialize for Triple {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde_support")]
impl<'de> serde::de::Deserialize<'de> for Triple {
    fn deserialize<D: serde::de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use alloc::string::String;
        use serde::Deserialize;
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sia32_properties() {
        let t: Triple = "sia32-unknown-none".parse().unwrap();
        assert_eq!(t.architecture, Architecture::Sia32);
        assert_eq!(t.pointer_width(), Ok(PointerWidth::U32));
        assert_eq!(t.endianness(), Ok(Endianness::Little));
        assert_eq!(t.default_calling_convention(), Ok(CallingConvention::SystemV));
        assert_eq!(t.data_model(), Ok(CDataModel::ILP32));
        assert_eq!(t.to_string(), "sia32-unknown-none");
    }

    #[test]
    fn existing_triples_remain_upstream_compatible() {
        for s in [
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "riscv64gc-unknown-linux-gnu",
            "s390x-unknown-linux-gnu",
        ] {
            let local: Triple = s.parse().unwrap();
            let upstream: target_lexicon_upstream::Triple = s.parse().unwrap();
            assert_eq!(local.to_string(), upstream.to_string());
            assert_eq!(local.pointer_width(), upstream.pointer_width());
            assert_eq!(local.endianness(), upstream.endianness());
        }
    }
}
