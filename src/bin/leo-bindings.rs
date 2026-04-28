use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use colored::Colorize;
use leo_package::{Dependency, Location, MANIFEST_FILENAME, Manifest};
use leo_span::create_session_if_not_set_then;
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Parser)]
#[command(name = "leo-bindings")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Update {
        #[arg(default_value = ".")]
        path: PathBuf,

        #[arg(short, long)]
        yes: bool,

        /// Import leo-bindings from workspace instead of github
        #[arg(long)]
        workspace: bool,
    },
}

fn main() -> Result<()> {
    leo_bindings::utils::init_simple_logger();

    let cli = Cli::parse();
    create_session_if_not_set_then(|_| match cli.command {
        Commands::Update {
            path,
            yes,
            workspace,
        } => update_bindings(&path, yes, workspace),
    })
}

fn update_bindings(project_path: &Path, auto_yes: bool, workspace: bool) -> Result<()> {
    let project_path = project_path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let manifest_path = project_path.join(MANIFEST_FILENAME);
    if !manifest_path.exists() {
        bail!("{MANIFEST_FILENAME} not found.");
    }

    let manifest = Manifest::read_from_file(&manifest_path).context("Failed to read manifest")?;
    let main_program_name = manifest
        .program
        .strip_suffix(".aleo")
        .unwrap_or(&manifest.program);

    let mut programs: HashMap<PathBuf, (String, Vec<Dependency>)> = HashMap::new();
    programs.insert(
        project_path.clone(),
        (
            main_program_name.to_string(),
            manifest.dependencies.clone().unwrap_or_default(),
        ),
    );

    for dep in manifest.dependencies.as_ref().unwrap_or(&vec![]) {
        if dep.location == Location::Local
            && let Some(ref dep_path) = dep.path
        {
            let abs_dep_path = project_path.join(dep_path);
            let dep_name = abs_dep_path.file_name().unwrap();
            let bindings_path = project_path.join("imports").join(dep_name);
            collect_local_programs(&mut programs, &abs_dep_path, &bindings_path)?;
        }
    }

    println!("{} Found {} programs", "✓".green(), programs.len());

    let mut file_paths = vec![project_path.join("Cargo.toml")];
    for program_dir in programs.keys() {
        file_paths.push(program_dir.join("lib.rs"));
        file_paths.push(program_dir.join("build.rs"));
        file_paths.push(program_dir.join(".gitignore"));
        if program_dir != &project_path {
            file_paths.push(program_dir.join("Cargo.toml"));
        }
    }

    println!("\n{}", "Files to replace:".yellow().bold());
    for path in &file_paths {
        let relative_path = path.strip_prefix(&project_path).unwrap_or(path);
        println!("  {} {}", "✓".green(), relative_path.display());
    }

    if !auto_yes {
        print!("\n{} ", "Replace? [y/N]".yellow().bold());
        std::io::Write::flush(&mut io::stdout())?;
        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        if !matches!(response.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("{} Cancelled.", "✗".red());
            return Ok(());
        }
    }

    println!();

    let cargo_toml_path = project_path.join("Cargo.toml");
    let preserved_imports = if cargo_toml_path.exists() {
        let existing_content = fs::read_to_string(&cargo_toml_path)?;
        let re = Regex::new(r"(?s)# BEGIN IMPORTS\n(.*?)# END IMPORTS").unwrap();
        re.captures(&existing_content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
    } else {
        None
    };

    let lib_name = format!("{main_program_name}_bindings");

    let root_package_meta = if workspace {
        "version.workspace = true\nedition.workspace = true\n".to_string()
    } else {
        "version = \"0.1.0\"\nedition = \"2024\"\n".to_string()
    };

    let mut cargo_toml = String::new();

    let _ = write!(
        cargo_toml,
        r#"
[package]
name = "{lib_name}"
{root_package_meta}[lib]
name = "{lib_name}"
path = "lib.rs"

[dependencies]
"#
    );

    if workspace {
        cargo_toml.push_str("leo-bindings.workspace = true\n");
    } else {
        cargo_toml.push_str(
            r#"leo-bindings = { git = "https://github.com/henrikkv/leo-bindings" }
"#,
        );
    }

    for dep in &programs[&project_path].1 {
        let dep_name = dep.name.strip_suffix(".aleo").unwrap_or(&dep.name);
        let lib_name = format!("{dep_name}_bindings");

        match (&dep.location, &dep.path) {
            (Location::Local, Some(dep_path)) => {
                let abs_dep_path = project_path.join(dep_path);
                if is_library(&abs_dep_path) {
                    continue;
                }
                let relative_path = dep_path
                    .strip_prefix(&project_path)
                    .unwrap_or(dep_path)
                    .to_string_lossy();
                let _ = writeln!(cargo_toml, r#"{lib_name} = {{ path = "{relative_path}" }}"#);
            }
            _ => {
                if workspace {
                    let _ = writeln!(cargo_toml, r"{lib_name}.workspace = true");
                }
            }
        }
    }

    if let Some(preserved) = preserved_imports {
        if preserved.is_empty() {
            cargo_toml.push_str(default_imports_block());
        } else {
            let _ = write!(
                cargo_toml,
                r#"
# BEGIN IMPORTS
{preserved}
# END IMPORTS
"#
            );
        }
    } else {
        cargo_toml.push_str(default_imports_block());
    }

    fs::write(&cargo_toml_path, cargo_toml).context("Failed to write Cargo.toml")?;

    for (program_dir, (program_name, deps)) in &programs {
        if !program_dir.exists() {
            fs::create_dir_all(program_dir)?;
        }

        let lib_rs_content = r"use leo_bindings::generate_bindings;
generate_bindings!();
";
        fs::write(program_dir.join("lib.rs"), lib_rs_content)?;

        let gitignore_content = r"target/
registry/
Cargo.lock

build/*
!build/
!build/main.aleo
!build/abi.json
!build/imports/
build/imports/*
!build/imports/*.abi.json

outputs/
";
        fs::write(program_dir.join(".gitignore"), gitignore_content)?;

        let mut build_rs = String::from(
            r#"fn main() {
    use std::path::Path;

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = Path::new(&manifest_dir);
    let src_main_leo = manifest_path.join("src/main.leo");
    let src_main_aleo = manifest_path.join("src/main.aleo");
    let build_main_aleo = manifest_path.join("build/main.aleo");
    let build_abi = manifest_path.join("build/abi.json");

    if src_main_leo.exists() && src_main_aleo.exists() {
        panic!("Cannot have both src/main.leo and src/main.aleo; remove one.");
    }

    println!("cargo:rerun-if-changed=build/abi.json");
    if src_main_leo.exists() {
        println!("cargo:rerun-if-changed=src/main.leo");
    } else if src_main_aleo.exists() {
        println!("cargo:rerun-if-changed=src/main.aleo");
    }

"#,
        );

        for dep in deps {
            if dep.location == Location::Local
                && let Some(dep_path) = dep.path.as_ref()
            {
                let abs_dep_path = program_dir.join(dep_path);
                let relative_path = dep_path.strip_prefix(program_dir).unwrap_or(dep_path);
                if is_library(&abs_dep_path) {
                    let _ = write!(
                        build_rs,
                        r#"
    println!("cargo:rerun-if-changed={}/src/lib.leo");
"#,
                        relative_path.to_string_lossy()
                    );
                } else {
                    let _ = write!(
                        build_rs,
                        r#"
    println!("cargo:rerun-if-changed={}/build/abi.json");
"#,
                        relative_path.to_string_lossy()
                    );
                }
            }
        }

        build_rs.push_str(
            r#"
    let needs_refresh = if src_main_leo.exists() {
        !build_main_aleo.exists()
            || !build_abi.exists()
            || match (
                src_main_leo.metadata().ok().and_then(|m| m.modified().ok()),
                build_main_aleo.metadata().ok().and_then(|m| m.modified().ok()),
            ) {
                (Some(s), Some(d)) => s > d,
                _ => false,
            }
    } else if src_main_aleo.exists() {
        !build_abi.exists()
            || match (
                src_main_aleo.metadata().ok().and_then(|m| m.modified().ok()),
                build_abi.metadata().ok().and_then(|m| m.modified().ok()),
            ) {
                (Some(s), Some(d)) => s > d,
                _ => false,
            }
    } else {
        panic!("Expected main.leo or main.aleo in {}.", manifest_path.display());
    };

    if needs_refresh {
        std::fs::create_dir_all(manifest_path.join("build")).expect("create build directory");
        if src_main_leo.exists() {
            println!("cargo:warning=Running leo build");
            let status = std::process::Command::new("leo")
                .arg("build")
                .current_dir(manifest_path)
                .status()
                .expect("Failed to run leo build");
            if !status.success() {
                panic!("leo build failed");
            }
        } else {
            println!("cargo:warning=Running leo abi");
            let status = std::process::Command::new("leo")
                .arg("abi")
                .arg(&src_main_aleo)
                .arg("-o")
                .arg(&build_abi)
                .current_dir(manifest_path)
                .status()
                .expect("Failed to run leo abi");
            if !status.success() {
                panic!("leo abi failed");
            }
        }
    } else {
        println!("cargo:warning=ABI up to date, skipping");
    }

    if !build_abi.exists() {
        panic!("Expected abi.json in {}.", manifest_path.display());
    }
}
"#,
        );
        fs::write(program_dir.join("build.rs"), build_rs)?;

        if program_dir != &project_path {
            let lib_name = format!("{program_name}_bindings");
            let import_cargo_toml_path = program_dir.join("Cargo.toml");

            let import_pkg_meta = if workspace {
                "version.workspace = true\nedition.workspace = true\n".to_string()
            } else {
                "version = \"0.1.0\"\nedition = \"2024\"\n".to_string()
            };
            let import_deps = if workspace {
                "leo-bindings.workspace = true\n".to_string()
            } else {
                "leo-bindings = { git = \"https://github.com/henrikkv/leo-bindings\" }\n"
                    .to_string()
            };
            let mut import_cargo_toml = format!(
                r#"[package]
name = "{lib_name}"
{import_pkg_meta}[lib]
name = "{lib_name}"
path = "lib.rs"

[dependencies]
{import_deps}"#
            );

            for dep in deps {
                let dep_name = dep.name.strip_suffix(".aleo").unwrap_or(&dep.name);
                let lib_name = format!("{dep_name}_bindings");

                match (&dep.location, &dep.path) {
                    (Location::Local, Some(dep_path)) => {
                        let abs_dep_path = program_dir.join(dep_path);
                        if is_library(&abs_dep_path) {
                            continue;
                        }
                        let relative_path = dep_path
                            .strip_prefix(program_dir)
                            .unwrap_or(dep_path)
                            .to_string_lossy();
                        let _ = writeln!(
                            import_cargo_toml,
                            r#"{lib_name} = {{ path = "{relative_path}" }}"#
                        );
                    }
                    _ => {
                        if workspace {
                            let _ = writeln!(import_cargo_toml, r"{lib_name}.workspace = true");
                        }
                    }
                }
            }
            fs::write(&import_cargo_toml_path, import_cargo_toml).map_err(|e| {
                anyhow::anyhow!("Failed to write Cargo.toml for {program_name}: {e}")
            })?;
        }
    }

    println!("\n{} Cargo setup done!", "✓".green().bold());

    Ok(())
}

fn default_imports_block() -> &'static str {
    r#"
# BEGIN IMPORTS
# credits_bindings = { git = "https://github.com/henrikkv/leo-bindings" }
# END IMPORTS
"#
}

fn is_library(path: &Path) -> bool {
    path.join("src/lib.leo").exists()
        && !path.join("src/main.leo").exists()
        && !path.join("src/main.aleo").exists()
}

fn collect_local_programs(
    programs: &mut HashMap<PathBuf, (String, Vec<Dependency>)>,
    leo_dep_path: &Path,
    bindings_dep_path: &Path,
) -> Result<()> {
    if is_library(leo_dep_path) {
        return Ok(());
    }

    let dep_manifest_path = leo_dep_path.join(MANIFEST_FILENAME);
    if !dep_manifest_path.exists() {
        return Ok(());
    }

    let dep_manifest = Manifest::read_from_file(&dep_manifest_path).context(format!(
        "Failed to read manifest at {}",
        dep_manifest_path.display()
    ))?;

    let dep_program_name = dep_manifest
        .program
        .strip_suffix(".aleo")
        .unwrap_or(&dep_manifest.program)
        .to_string();

    programs.insert(
        bindings_dep_path.to_path_buf(),
        (
            dep_program_name,
            dep_manifest.dependencies.clone().unwrap_or_default(),
        ),
    );

    let Some(deps) = &dep_manifest.dependencies else {
        return Ok(());
    };

    for sub_dep in deps {
        if sub_dep.location != Location::Local {
            continue;
        }

        let Some(sub_dep_path) = &sub_dep.path else {
            continue;
        };

        let abs_sub_dep = leo_dep_path.join(sub_dep_path);
        let sub_dep_name = abs_sub_dep.file_name().unwrap();
        let nested_bindings_path = bindings_dep_path.join("imports").join(sub_dep_name);

        let already_processed = programs.values().any(|(name, _)| {
            let check_name = abs_sub_dep
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            name == check_name || format!("{name}.aleo") == check_name
        });

        if !already_processed {
            collect_local_programs(programs, &abs_sub_dep, &nested_bindings_path)?;
        }
    }

    Ok(())
}
