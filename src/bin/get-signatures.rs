use clap::Parser;
use leo_bindings::signature::get_signatures;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "get-signatures")]
#[command(about = "Get function signatures from Leo dev.initial.json files")]
struct Args {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let input_content = std::fs::read_to_string(&args.input)?;
    let signatures_json = get_signatures(&input_content)?;

    if let Some(output_path) = args.output {
        std::fs::write(&output_path, &signatures_json)?;
        println!("Generated: {}", output_path.display());
    } else {
        println!("{}", signatures_json);
    }

    Ok(())
}
