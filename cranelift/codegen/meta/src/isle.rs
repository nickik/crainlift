use std::io::Result;

/// A list of compilations (transformations from ISLE source to
/// generated Rust source) that exist in the repository.
#[derive(Clone, Debug)]
pub struct IsleCompilations {
    pub items: Vec<IsleCompilation>,
}

impl IsleCompilations {
    pub fn lookup(&self, name: &str) -> Option<&IsleCompilation> {
        for compilation in &self.items {
            if compilation.name == name {
                return Some(compilation);
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
pub struct IsleCompilation {
    pub name: String,
    pub output: std::path::PathBuf,
    pub tracked_inputs: Vec<std::path::PathBuf>,
    pub untracked_inputs: Vec<std::path::PathBuf>,
}

impl IsleCompilation {
    /// All inputs to the computation, tracked or untracked. May contain directories.
    pub fn inputs(&self) -> Vec<std::path::PathBuf> {
        self.tracked_inputs
            .iter()
            .chain(self.untracked_inputs.iter())
            .cloned()
            .collect()
    }

    /// Get all path inputs. Directory inputs are expanded to all ISLE files.
    pub fn paths(&self) -> Result<Vec<std::path::PathBuf>> {
        let mut paths = Vec::new();
        for input in self.inputs() {
            paths.extend(Self::expand_paths(&input)?);
        }
        Ok(paths)
    }

    fn expand_paths(input: &std::path::PathBuf) -> Result<Vec<std::path::PathBuf>> {
        if input.is_file() {
            return Ok(vec![input.clone()]);
        }
        if !input.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("ISLE input does not exist: {}", input.display()),
            ));
        }
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(input).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!(
                    "failed to read ISLE input directory {}: {e}",
                    input.display()
                ),
            )
        })? {
            let path = entry?.path();
            if let Some(ext) = path.extension() {
                if ext == "isle" {
                    paths.push(path);
                }
            }
        }
        Ok(paths)
    }
}

pub fn shared_isle_lower_paths(codegen_crate_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let inst_specs_isle = codegen_crate_dir.join("src").join("inst_specs.isle");
    let prelude_isle = codegen_crate_dir.join("src").join("prelude.isle");
    let prelude_lower_isle = codegen_crate_dir.join("src").join("prelude_lower.isle");
    vec![inst_specs_isle, prelude_isle, prelude_lower_isle]
}

/// Construct the list of ISLE compilation units.
pub fn get_isle_compilations(
    codegen_crate_dir: &std::path::Path,
    gen_dir: &std::path::Path,
) -> IsleCompilations {
    let numerics_isle = gen_dir.join("numerics.isle");
    let clif_lower_isle = gen_dir.join("clif_lower.isle");
    let clif_opt_isle = gen_dir.join("clif_opt.isle");
    let prelude_isle = codegen_crate_dir.join("src").join("prelude.isle");
    let prelude_opt_isle = codegen_crate_dir.join("src").join("prelude_opt.isle");
    let prelude_lower_isle = codegen_crate_dir.join("src").join("prelude_lower.isle");
    #[cfg(feature = "pulley")]
    let pulley_gen = gen_dir.join("pulley_gen.isle");

    let spec_inputs = |extra: &[&str]| -> Vec<std::path::PathBuf> {
        if !cfg!(feature = "spec") {
            return vec![];
        }
        let spec_dir = codegen_crate_dir.join("src").join("spec");
        let mut inputs = vec![
            spec_dir.join("prelude_spec.isle"),
            spec_dir.join("inst_specs.isle"),
            spec_dir.join("inst_tags.isle"),
        ];
        inputs.extend(extra.iter().map(|f| spec_dir.join(f)));
        inputs
    };
    let lower_spec_inputs = |extra: &[&str]| -> Vec<std::path::PathBuf> {
        let mut inputs = spec_inputs(extra);
        if cfg!(feature = "spec") {
            let spec_dir = codegen_crate_dir.join("src").join("spec");
            inputs.push(spec_dir.join("prelude_lower_spec.isle"));
        }
        inputs
    };

    let src_opts = codegen_crate_dir.join("src").join("opts");
    let src_isa_x64 = codegen_crate_dir.join("src").join("isa").join("x64");
    let src_isa_aarch64 = codegen_crate_dir.join("src").join("isa").join("aarch64");
    let src_isa_s390x = codegen_crate_dir.join("src").join("isa").join("s390x");
    let src_isa_risc_v = codegen_crate_dir.join("src").join("isa").join("riscv64");
    let src_isa_sia32 = codegen_crate_dir.join("src").join("isa").join("sia32");
    #[cfg(feature = "pulley")]
    let src_isa_pulley_shared = codegen_crate_dir
        .join("src")
        .join("isa")
        .join("pulley_shared");

    IsleCompilations {
        items: vec![
            IsleCompilation {
                name: "opt".to_string(),
                output: gen_dir.join("isle_opt.rs"),
                tracked_inputs: [
                    vec![prelude_isle.clone(), prelude_opt_isle],
                    spec_inputs(&["fpconst.isle", "opt.isle"]),
                    vec![
                        src_opts.join("arithmetic.isle"),
                        src_opts.join("bitops.isle"),
                        src_opts.join("cprop.isle"),
                        src_opts.join("extends.isle"),
                        src_opts.join("icmp.isle"),
                        src_opts.join("remat.isle"),
                        src_opts.join("selects.isle"),
                        src_opts.join("shifts.isle"),
                        src_opts.join("skeleton.isle"),
                        src_opts.join("spaceship.isle"),
                        src_opts.join("spectre.isle"),
                        src_opts.join("vector.isle"),
                    ],
                ]
                .concat(),
                untracked_inputs: vec![numerics_isle.clone(), clif_opt_isle],
            },
            IsleCompilation {
                name: "x64".to_string(),
                output: gen_dir.join("isle_x64.rs"),
                tracked_inputs: [
                    vec![prelude_isle.clone(), prelude_lower_isle.clone()],
                    lower_spec_inputs(&["fpconst.isle", "state.isle"]),
                    vec![
                        src_isa_x64.join("inst.isle"),
                        src_isa_x64.join("lower.isle"),
                    ],
                ]
                .concat(),
                untracked_inputs: vec![
                    numerics_isle.clone(),
                    clif_lower_isle.clone(),
                    gen_dir.join("assembler.isle"),
                ],
            },
            IsleCompilation {
                name: "aarch64".to_string(),
                output: gen_dir.join("isle_aarch64.rs"),
                tracked_inputs: [
                    vec![prelude_isle.clone(), prelude_lower_isle.clone()],
                    lower_spec_inputs(&["fpconst.isle", "state.isle"]),
                    vec![
                        src_isa_aarch64.join("inst.isle"),
                        src_isa_aarch64.join("inst_neon.isle"),
                    ],
                    if cfg!(feature = "spec") {
                        vec![src_isa_aarch64.join("spec")]
                    } else {
                        vec![]
                    },
                    vec![
                        src_isa_aarch64.join("lower.isle"),
                        src_isa_aarch64.join("lower_dynamic_neon.isle"),
                    ],
                ]
                .concat(),
                untracked_inputs: vec![numerics_isle.clone(), clif_lower_isle.clone()],
            },
            IsleCompilation {
                name: "s390x".to_string(),
                output: gen_dir.join("isle_s390x.rs"),
                tracked_inputs: [
                    vec![prelude_isle.clone(), prelude_lower_isle.clone()],
                    lower_spec_inputs(&[]),
                    vec![
                        src_isa_s390x.join("inst.isle"),
                        src_isa_s390x.join("lower.isle"),
                    ],
                ]
                .concat(),
                untracked_inputs: vec![numerics_isle.clone(), clif_lower_isle.clone()],
            },
            IsleCompilation {
                name: "riscv64".to_string(),
                output: gen_dir.join("isle_riscv64.rs"),
                tracked_inputs: [
                    vec![prelude_isle.clone(), prelude_lower_isle.clone()],
                    lower_spec_inputs(&[]),
                    vec![
                        src_isa_risc_v.join("inst.isle"),
                        src_isa_risc_v.join("inst_vector.isle"),
                        src_isa_risc_v.join("lower.isle"),
                    ],
                ]
                .concat(),
                untracked_inputs: vec![numerics_isle.clone(), clif_lower_isle.clone()],
            },
            IsleCompilation {
                name: "sia32".to_string(),
                output: gen_dir.join("isle_sia32.rs"),
                tracked_inputs: [
                    vec![prelude_isle.clone(), prelude_lower_isle.clone()],
                    lower_spec_inputs(&[]),
                    vec![
                        src_isa_sia32.join("inst.isle"),
                        src_isa_sia32.join("lower.isle"),
                    ],
                ]
                .concat(),
                untracked_inputs: vec![numerics_isle.clone(), clif_lower_isle.clone()],
            },
            #[cfg(feature = "pulley")]
            IsleCompilation {
                name: "pulley".to_string(),
                output: gen_dir.join("isle_pulley_shared.rs"),
                tracked_inputs: vec![
                    prelude_isle.clone(),
                    prelude_lower_isle.clone(),
                    src_isa_pulley_shared.join("inst.isle"),
                    src_isa_pulley_shared.join("lower.isle"),
                ],
                untracked_inputs: vec![numerics_isle, pulley_gen, clif_lower_isle],
            },
        ],
    }
}
