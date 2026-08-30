mod analysis;
mod input;
mod model;
mod report;

use analysis::analyze;
use clap::Parser;
use input::{parse_lines, parse_pricing_model};
use report::render;
use std::fmt::{self, Display};
use std::io;

use crate::input::PricingLoadError;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to the JSONL request log
    #[arg(short, long)]
    input: String,

    /// Path to the pricing JSON
    #[arg(short, long)]
    pricing: String,
}

#[derive(Debug)]
enum AppError {
    Input(io::Error),
    Pricing(PricingLoadError),
}

impl Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Input(e) => {
                write!(f, "failed to load input file: {}", e)
            }
            AppError::Pricing(e) => {
                write!(f, "failed to load pricing file: {}", e)
            }
        }
    }
}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        AppError::Input(value)
    }
}

impl From<PricingLoadError> for AppError {
    fn from(value: PricingLoadError) -> Self {
        AppError::Pricing(value)
    }
}

fn run(args: Args) -> Result<(), AppError> {
    let parsed_result = parse_lines(&args.input)?;
    for error in &parsed_result.errors {
        eprintln!("Warning: {error}");
    }
    let pricing_table = parse_pricing_model(&args.pricing)?;

    let analysis_result = analyze(&parsed_result, &pricing_table);
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
