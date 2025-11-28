use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use colored::Colorize;
use leo_package::{Dependency, Location, MANIFEST_FILENAME, Manifest};
use leo_span::create_session_if_not_set_then;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
    },
}

fn main() -> Result<()> {
    leo_bindings::utils::init_simple_logger();

    let cli = Cli::parse();
    create_session_if_not_set_then(|_| match cli.command {
        Commands::Update { path, yes } => update_bindings(&path, yes),
    })
}

fn update_bindings(project_path: &Path, auto_yes: bool) -> Result<()> {
    let project_path = project_path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let manifest_path = project_path.join(MANIFEST_FILENAME);
    if !manifest_path.exists() {
        bail!("{} not found.", MANIFEST_FILENAME);
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
        if dep.location == Location::Local && dep.path.is_some() {
            let abs_dep_path = project_path.join(dep.path.as_ref().unwrap());
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
        file_paths.push(program_dir.join(".cargo/config.toml"));
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
        io::stdout().flush()?;
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

    let lib_name = format!("{}_bindings", main_program_name);

    let has_local_deps = manifest
        .dependencies
        .as_ref()
        .is_some_and(|deps| deps.iter().any(|dep| dep.location == Location::Local));

    let mut cargo_toml = String::from("[workspace]");

    if has_local_deps {
        cargo_toml.push_str(
            r#"
members = [".", "imports/*"]
"#,
        );
    } else {
        cargo_toml.push_str(
            r#"
members = ["."]
"#,
        );
    }

    cargo_toml.push_str(
        r#"
[workspace.dependencies]
leo-bindings = { git = "https://github.com/henrikkv/leo-bindings" }
rand = "0.8"
snarkvm = { git = "https://github.com/ProvableHQ/snarkVM.git", tag = "v4.4.0" }
log = "0.4"
"#,
    );

    if let Some(preserved) = preserved_imports {
        cargo_toml.push_str(&format!(
            r#"
# BEGIN IMPORTS
{}
# END IMPORTS
"#,
            preserved
        ));
    } else {
        cargo_toml.push_str(
            r#"
# BEGIN IMPORTS
# credits_bindings = { git = "https://github.com/henrikkv/leo-bindings" }
# END IMPORTS
"#,
        );
    }

    cargo_toml.push_str(&format!(
        r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[lib]
name = "{}"
path = "lib.rs"

[dependencies]
leo-bindings.workspace = true
rand.workspace = true
snarkvm.workspace = true
log.workspace = true
"#,
        lib_name, lib_name
    ));

    for dep in &programs[&project_path].1 {
        let dep_name = dep.name.strip_suffix(".aleo").unwrap_or(&dep.name);
        let lib_name = format!("{}_bindings", dep_name);

        match (&dep.location, &dep.path) {
            (Location::Local, Some(dep_path)) => {
                let relative_path = dep_path
                    .strip_prefix(&project_path)
                    .unwrap_or(dep_path)
                    .to_string_lossy();
                cargo_toml.push_str(&format!(
                    r#"
{} = {{ path = "{}" }}
"#,
                    lib_name, relative_path
                ));
            }
            _ => cargo_toml.push_str(&format!(
                r#"
{}.workspace = true
"#,
                lib_name
            )),
        }
    }

    cargo_toml.push_str(
        r#"
[build-dependencies]
leo-bindings-core = { git = "https://github.com/henrikkv/leo-bindings" }
serde_json = "1"
"#,
    );

    fs::write(&cargo_toml_path, cargo_toml).context("Failed to write Cargo.toml")?;

    for (program_dir, (program_name, deps)) in &programs {
        if !program_dir.exists() {
            fs::create_dir_all(program_dir)?;
        }

        let lib_rs_content = r#"use leo_bindings::generate_bindings;
generate_bindings!("signatures.json");
"#;
        fs::write(program_dir.join("lib.rs"), lib_rs_content)?;

        let gitignore_content = r#"target/
registry/
Cargo.lock

build/*
!build/
!build/main.aleo

outputs/
"#;
        fs::write(program_dir.join(".gitignore"), gitignore_content)?;

        let mut build_rs = format!(
            r#"fn main() {{
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = std::path::Path::new(&manifest_dir);
    let src_main_leo = manifest_path.join("src/main.leo");
    let build_main_aleo = manifest_path.join("build/main.aleo");
    let initial_json = manifest_path.join("outputs/{}.initial.json");
    let signatures_json = manifest_path.join("signatures.json");

    println!("cargo:rerun-if-changed=signatures.json");

    if src_main_leo.exists() {{
        println!("cargo:rerun-if-changed=src/main.leo");
    }}

    if build_main_aleo.exists() {{
        println!("cargo:rerun-if-changed=build/main.aleo");
    }}

    if initial_json.exists() {{
        println!("cargo:rerun-if-changed=outputs/{}.initial.json");
    }}
"#,
            program_name, program_name
        );

        for dep in deps {
            if dep.location == Location::Local && dep.path.is_some() {
                let dep_path = dep.path.as_ref().unwrap();
                let relative_path = dep_path.strip_prefix(program_dir).unwrap_or(dep_path);
                build_rs.push_str(&format!(
                    r#"
    println!("cargo:rerun-if-changed={}/signatures.json");
"#,
                    relative_path.to_string_lossy()
                ));
            }
        }

        build_rs.push_str(r#"
    if src_main_leo.exists() {
        let needs_leo_build = !build_main_aleo.exists()
            || !initial_json.exists()
            || (src_main_leo.metadata().unwrap().modified().unwrap() > build_main_aleo.metadata().unwrap().modified().unwrap());
        if needs_leo_build {
            println!("cargo:warning=Running leo build to create initial ast snapshot");
            let status = std::process::Command::new("leo")
                .arg("build")
                .arg("--enable-initial-ast-snapshot")
                .current_dir(manifest_path)
                .status()
                .expect("Failed to run leo build");
            if !status.success() {
                panic!("leo build failed");
            }
        } else {
            println!("cargo:warning=Leo build up to date, skipping");
        }
    }

    if initial_json.exists() {
        let should_check = !signatures_json.exists() || (initial_json.metadata().unwrap().modified().unwrap() > signatures_json.metadata().unwrap().modified().unwrap());

        if should_check {
            let json = std::fs::read_to_string(&initial_json).expect("Failed to read initial.json");
            let new_signatures = leo_bindings_core::signature::get_signatures(json);

            let should_write = if signatures_json.exists() {
                let existing_signatures = std::fs::read_to_string(&signatures_json).expect("Failed to read existing signatures.json");
                new_signatures != existing_signatures
            } else {
                true
            };

            if should_write {
                println!("cargo:warning=Signatures changed, updating signatures.json (will trigger macro recompilation)");
                std::fs::write(&signatures_json, new_signatures)
                    .expect("Failed to write signatures.json");
            } else {
                println!("cargo:warning=Signatures unchanged, skipping update (macro recompilation avoided)");
            }
        } else {
            println!("cargo:warning=Signatures up-to-date, skipping check");
        }
    }

    if !signatures_json.exists() {
        if src_main_leo.exists() {
            panic!("Failed to generate signatures.json.");
        } else if build_main_aleo.exists() {
            panic!("signatures.json not found. TODO: Make a parser for .aleo files.");
        } else {
            panic!("No Leo source files or build artifacts found.");
        }
    }
}
"#
        );
        fs::write(program_dir.join("build.rs"), build_rs)?;

        let cargo_dir = program_dir.join(".cargo");
        if !cargo_dir.exists() {
            fs::create_dir_all(&cargo_dir)?;
        }
        fs::write(cargo_dir.join("config.toml"), "[workspace]\n")?;

        if program_dir != &project_path {
            let lib_name = format!("{}_bindings", program_name);
            let import_cargo_toml_path = program_dir.join("Cargo.toml");

            let mut import_cargo_toml = format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[lib]
name = "{}"
path = "lib.rs"

[dependencies]
leo-bindings.workspace = true
log.workspace = true

[build-dependencies]
leo-bindings-core = {{ git = "https://github.com/henrikkv/leo-bindings" }}
serde_json = "1"
"#,
                lib_name, lib_name
            );

            for dep in deps {
                let dep_name = dep.name.strip_suffix(".aleo").unwrap_or(&dep.name);
                let lib_name = format!("{}_bindings", dep_name);

                match (&dep.location, &dep.path) {
                    (Location::Local, Some(dep_path)) => {
                        let relative_path = dep_path
                            .strip_prefix(program_dir)
                            .unwrap_or(dep_path)
                            .to_string_lossy();
                        import_cargo_toml.push_str(&format!(
                            r#"
{} = {{ path = "{}" }}
"#,
                            lib_name, relative_path
                        ));
                    }
                    _ => import_cargo_toml.push_str(&format!(
                        r#"
{}.workspace = true
"#,
                        lib_name
                    )),
                }
            }
            fs::write(&import_cargo_toml_path, import_cargo_toml).map_err(|e| {
                anyhow::anyhow!("Failed to write Cargo.toml for {}: {}", program_name, e)
            })?;
        }
    }

    println!("\n{} Cargo setup done!", "✓".green().bold());
    println!("\n{}", "Next step:".yellow().bold());
    println!("  Add external programs to the IMPORTS block in Cargo.toml");

    Ok(())
}

fn collect_local_programs(
    programs: &mut HashMap<PathBuf, (String, Vec<Dependency>)>,
    leo_dep_path: &Path,
    bindings_dep_path: &Path,
) -> Result<()> {
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
            name == check_name || format!("{}.aleo", name) == check_name
        });

        if !already_processed {
            collect_local_programs(programs, &abs_sub_dep, &nested_bindings_path)?;
        }
    }

    Ok(())
}
