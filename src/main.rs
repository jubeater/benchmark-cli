use benchmark_cli::identity::RecordResolver;
use benchmark_cli::input::parse_lines;
use benchmark_cli::report::render;
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
    Input { path: String, source: io::Error },
}

impl Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Input { path, source } => {
                write!(f, "failed to load input file '{path}': {source}")
            }
        }
    }
}

fn run(args: Args) -> Result<i32, AppError> {
    let parsed_result = parse_lines(&args.input).map_err(|source| AppError::Input {
        path: args.input.clone(),
        source,
    })?;
    let mut accepted = 0;
    let mut rejections: Vec<(usize, String)> = parsed_result
        .errors
        .iter()
        .map(|error| {
            (
                error.line_number,
                format!("line {}: <json>: {}", error.line_number, error.source_error),
            )
        })
        .collect();
    let mut resolver = RecordResolver::default();
    for record in parsed_result.records {
        match resolver.validate_record(record) {
            Ok(_) => accepted += 1,
            Err(error) => {
                rejections.push((error.line, error.to_string()));
            }
        }
    }
    rejections.sort_by_key(|(line, _)| *line);
    for (_, message) in &rejections {
        eprintln!("{message}");
    }
    let rejected = rejections.len();

    let output_string = render(accepted, rejected, resolver.superseded_count());
    print!("{output_string}");

    if rejected > 0 { Ok(1) } else { Ok(0) }
}
fn main() {
    let args = Args::parse();

    match run(args) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(2);
        }
    }
}
