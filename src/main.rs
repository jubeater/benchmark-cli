use benchmark_cli::input::parse_lines;
use benchmark_cli::report::render;
use benchmark_cli::validator::validate_record;
use clap::Parser;
use std::fmt::{self, Display};
use std::io;

#[derive(Parser, Debug)]
#[command(version, about = "Validate eval-schema v2 JSONL results")]
struct Args {
    /// Path to the eval-schema v2 JSONL results
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

    let mut accepted = 0;
    let mut rejected = parsed_result.errors.len();
    for record in parsed_result.records {
        match validate_record(record) {
            Ok(_) => accepted += 1,
            Err(error) => {
                rejected += 1;
                eprintln!("Warning: line {}: {:?}", error.line, error.error_type);
            }
        }
    }

    let output_string = render(accepted, rejected);
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
