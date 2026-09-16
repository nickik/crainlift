use crate::cdsl::{isa::TargetIsa, settings::SettingGroupBuilder};

pub(crate) fn define() -> TargetIsa {
    // Keep the first SIA32 settings group intentionally empty. Optional
    // architectural extensions (mul/div, later floating point, etc.) are added
    // only when lowering can actually honor them.
    let settings = SettingGroupBuilder::new("sia32");
    TargetIsa::new("sia32", settings.build())
}
