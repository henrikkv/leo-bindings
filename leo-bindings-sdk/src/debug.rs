use crate::error::{Error, Result};
use leo_debugger::ProgramSource;
use snarkvm::prelude::{Program, TestnetV0};

use std::path::{Path, PathBuf};

pub fn collect_debug_sources(crate_dir: &Path) -> Result<Vec<ProgramSource>> {
    let build_dir = crate_dir.join("build");
    let mut sources = Vec::new();

    if build_dir.is_dir() {
        let mut unit_dirs: Vec<PathBuf> = std::fs::read_dir(&build_dir)
            .map_err(|e| Error::Other(format!("failed to read {}: {e}", build_dir.display())))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        unit_dirs.sort();

        for unit_dir in unit_dirs {
            let Some(unit) = unit_dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let bytecode_path = unit_dir.join(format!("{unit}.aleo"));
            if !bytecode_path.exists() {
                continue;
            }
            let bytecode = std::fs::read_to_string(&bytecode_path).map_err(|e| {
                Error::Other(format!("failed to read {}: {e}", bytecode_path.display()))
            })?;
            let debug_info = crate::load_debug_info(&unit_dir.join(format!("{unit}.dbg.json")))
                .map_err(|e| {
                    Error::Other(format!("failed to load debug info for '{unit}': {e}"))
                })?;
            sources.push(ProgramSource {
                name: format!("{unit}.aleo"),
                bytecode,
                debug_info,
            });
        }
    }

    let src_main = crate_dir.join("src/main.aleo");
    if src_main.exists() {
        let bytecode = std::fs::read_to_string(&src_main)
            .map_err(|e| Error::Other(format!("failed to read {}: {e}", src_main.display())))?;
        let name = bytecode
            .parse::<Program<TestnetV0>>()
            .map_err(|e| Error::Other(format!("failed to parse {}: {e}", src_main.display())))?
            .id()
            .to_string();

        if !sources.iter().any(|source| source.name == name) {
            log::warn!("No debug info for '{name}'");
            sources.push(ProgramSource {
                name,
                bytecode,
                debug_info: None,
            });
        }
    }

    Ok(sources)
}
