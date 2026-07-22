use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use indexmap::{IndexMap, IndexSet};
use leo_abi_types::{Interface, Program};
use leo_package::{
    CompilationUnit, Dependency, Location, MANIFEST_FILENAME, Package, PackageKind, ProgramData,
    WORKSPACE_MANIFEST_FILENAME, Workspace, bare_unit_name,
};
use leo_span::{Symbol, create_session_if_not_set_then};

use crate::generator::ImportRef;

#[derive(Debug, Clone)]
pub struct ResolvedUnit {
    dir: PathBuf,
    bare_name: String,
    unit: CompilationUnit,
    package: Package,
    rebuild_source: Option<PathBuf>,
}

pub type Units = IndexMap<PathBuf, ResolvedUnit>;

#[derive(Debug, Clone)]
pub struct ResolvedWorkspace {
    workspace_root: Option<PathBuf>,
    units: Units,
}

impl ResolvedUnit {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn name(&self) -> &str {
        &self.bare_name
    }

    pub fn kind(&self) -> &PackageKind {
        &self.unit.kind
    }

    pub fn dependencies(&self) -> &IndexSet<Dependency> {
        &self.unit.dependencies
    }

    pub fn build_abi(&self) -> PathBuf {
        if self.is_bytecode_only() {
            self.package
                .unit_build_directory(&self.bare_name)
                .join(self.leo_abi_json_filename())
        } else {
            self.package.unit_abi_path(&self.bare_name)
        }
    }

    pub fn rebuild_source(&self) -> Option<PathBuf> {
        self.rebuild_source.clone()
    }

    pub(crate) fn is_bytecode_only(&self) -> bool {
        matches!(self.unit.data, ProgramData::Bytecode(_))
    }

    pub(crate) fn leo_abi_json_filename(&self) -> String {
        format!("{}.abi.json", self.package.manifest.program)
    }

    pub(crate) fn load_abi(&self) -> Result<Program> {
        let abi_path = self.build_abi();
        let json = std::fs::read_to_string(&abi_path)
            .with_context(|| format!("failed to read ABI at {}", abi_path.display()))?;
        let abi: Program = serde_json::from_str(&json)
            .with_context(|| format!("failed to parse ABI at {}", abi_path.display()))?;

        let program_id = abi.program.trim_end_matches(".aleo");
        if program_id != self.name() {
            bail!(
                "program name '{}' does not match ABI program '{}' at {}",
                self.name(),
                abi.program,
                abi_path.display()
            );
        }

        Ok(abi)
    }

    pub(crate) fn load_interfaces(&self) -> Result<Vec<Interface>> {
        let interfaces_dir = self
            .package
            .unit_build_directory(&self.bare_name)
            .join("interfaces");
        if !interfaces_dir.exists() {
            return Ok(Vec::new());
        }

        let mut interfaces = Vec::new();
        for entry in std::fs::read_dir(&interfaces_dir)
            .with_context(|| format!("failed to read {}", interfaces_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let json = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read interface at {}", path.display()))?;
            let interface: Interface = serde_json::from_str(&json)
                .with_context(|| format!("failed to parse interface at {}", path.display()))?;
            interfaces.push(interface);
        }
        interfaces.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(interfaces)
    }
}

impl ResolvedWorkspace {
    pub fn units(&self) -> &Units {
        &self.units
    }

    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    pub fn programs(&self) -> Vec<&ResolvedUnit> {
        self.units
            .values()
            .filter(|u| u.kind().is_program())
            .collect()
    }

    pub fn libraries(&self) -> Vec<&ResolvedUnit> {
        self.units
            .values()
            .filter(|u| u.kind().is_library())
            .collect()
    }

    pub(crate) fn leo_root(&self, manifest_path: &Path) -> PathBuf {
        self.workspace_root()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| manifest_path.to_path_buf())
    }
}

pub fn resolve_workspace(manifest_path: &Path) -> Result<ResolvedWorkspace> {
    create_session_if_not_set_then(|_| resolve_workspace_impl(manifest_path))
}

fn resolve_workspace_impl(manifest_path: &Path) -> Result<ResolvedWorkspace> {
    let leo_workspace =
        Workspace::discover(manifest_path).context("failed to resolve Leo workspace")?;
    let workspace_root = leo_workspace.as_ref().map(|ws| ws.root_directory.clone());

    let member_dirs = leo_workspace
        .as_ref()
        .map(|ws| ws.member_paths.clone())
        .unwrap_or_else(|| vec![manifest_path.to_path_buf()]);

    let mut units: Units = IndexMap::new();
    for member_dir in member_dirs {
        let unit = resolve_single_unit(&member_dir)?;
        units.insert(member_dir, unit);
    }

    Ok(ResolvedWorkspace {
        workspace_root,
        units,
    })
}

fn resolve_single_unit(dir: &Path) -> Result<ResolvedUnit> {
    let package = Package::from_directory_no_graph(dir, dir, None, None, 0)
        .with_context(|| format!("failed to resolve Leo package at {}", dir.display()))?;
    let symbol = Symbol::intern(&package.manifest.program);

    let main_aleo = package.source_directory().join("main.aleo");
    let main_aleo_exists = main_aleo.exists();
    let compilation_unit = match CompilationUnit::from_package_path(symbol, dir) {
        Ok(unit) => unit,
        Err(_) if main_aleo_exists => {
            CompilationUnit::from_aleo_path(symbol, &main_aleo, &IndexMap::new()).with_context(
                || {
                    format!(
                        "failed to resolve bytecode-only package at {}",
                        dir.display()
                    )
                },
            )?
        }
        Err(e) => return Err(e.into()),
    };

    let rebuild_source = match &compilation_unit.data {
        ProgramData::SourcePath { source, .. } => Some(source.clone()),
        ProgramData::Bytecode(_) => main_aleo_exists.then_some(main_aleo),
    };
    let bare_name = bare_unit_name(&compilation_unit.name.to_string()).to_string();

    Ok(ResolvedUnit {
        dir: dir.to_path_buf(),
        bare_name,
        unit: compilation_unit,
        package,
        rebuild_source,
    })
}

pub(crate) fn program_imports(unit: &ResolvedUnit, units: &Units) -> Vec<ImportRef> {
    unit.dependencies()
        .iter()
        .filter_map(|dep| match (&dep.location, &dep.path) {
            (Location::Local, Some(dep_path)) => units.get(dep_path).and_then(|dep_unit| {
                if dep_unit.kind().is_program() {
                    Some(ImportRef {
                        name: dep_unit.name().to_string(),
                        same_crate: true,
                    })
                } else {
                    None
                }
            }),
            _ => Some(ImportRef {
                name: bare_unit_name(&dep.name).to_string(),
                same_crate: false,
            }),
        })
        .collect()
}

pub fn cross_crate_imports(program_units: &[&ResolvedUnit]) -> IndexSet<String> {
    let mut names = IndexSet::new();
    for unit in program_units {
        for dep in unit.dependencies() {
            if dep.location != Location::Local {
                names.insert(bare_unit_name(&dep.name).to_string());
            }
        }
    }
    names
}

pub(crate) fn register_rerun_if_changed(workspace: &ResolvedWorkspace) {
    if let Some(workspace_root) = workspace.workspace_root() {
        println!(
            "cargo:rerun-if-changed={}",
            workspace_root.join(WORKSPACE_MANIFEST_FILENAME).display()
        );
    }
    for unit in workspace.units().values() {
        println!(
            "cargo:rerun-if-changed={}",
            unit.dir().join(MANIFEST_FILENAME).display()
        );
        if let Some(source) = unit.rebuild_source() {
            println!("cargo:rerun-if-changed={}", source.display());
        }
    }
}
