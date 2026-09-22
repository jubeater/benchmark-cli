mod analysis;
mod input;
mod model;
mod report;
mod validator;

use analysis::analyze;
use clap::Parser;
use input::parse_lines;
use report::render;
use std::fmt::{self, Display};
use std::io;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to the JSONL input
    #[arg(short, long)]
    input: String,
}

#[derive(Debug)]
enum AppError {
    Input(io::Error),
}

impl Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Input(e) => {
                write!(f, "failed to load input file: {}", e)
            }
        }
    }
}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        AppError::Input(value)
    }
}

fn run(args: Args) -> Result<(), AppError> {
    let parsed_result = parse_lines(&args.input)?;
    for error in &parsed_result.errors {
        eprintln!("Warning: {error}");
    }

    let analysis_result = analyze(&parsed_result, &());
    let output_string = render(&analysis_result);
    print!("{}", output_string);

    Ok(())
}
fn main() {
    let args = Args::parse();

    if let Err(e) = run(args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
