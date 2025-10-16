fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = std::path::Path::new(&manifest_dir);
    let src_main_leo = manifest_path.join("src/main.leo");
    let build_main_aleo = manifest_path.join("build/main.aleo");
    let initial_json = manifest_path.join("outputs/dev.initial.json");
    let signatures_json = manifest_path.join("signatures.json");

    println!("cargo:rerun-if-changed=signatures.json");

    if src_main_leo.exists() {
        println!("cargo:rerun-if-changed=src/main.leo");
    }

    if build_main_aleo.exists() {
        println!("cargo:rerun-if-changed=build/main.aleo");
    }

    if initial_json.exists() {
        println!("cargo:rerun-if-changed=outputs/dev.initial.json");
    }

    if src_main_leo.exists() {
        let needs_leo_build = !build_main_aleo.exists() || (src_main_leo.metadata().unwrap().modified().unwrap() > build_main_aleo.metadata().unwrap().modified().unwrap());
        if needs_leo_build {
            let status = std::process::Command::new("leo").arg("build").arg("--enable-initial-ast-snapshot").status().expect("Failed to run leo build");
            if !status.success() {
                panic!("leo build failed");
            }
        }
    }

    if initial_json.exists() {
        let needs_update = !signatures_json.exists() || (initial_json.metadata().unwrap().modified().unwrap() > signatures_json.metadata().unwrap().modified().unwrap());
        if needs_update {
            let json = std::fs::read_to_string(&initial_json).expect("Failed to read initial.json");
            let signatures = leo_bindings_core::signature::get_signatures(json);
            std::fs::write(&signatures_json, signatures).expect("Failed to write signatures.json");
        }
    }
}
