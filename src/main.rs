use benchmark_cli::aggregate::aggregate_records;
use benchmark_cli::identity::RecordResolver;
use benchmark_cli::input::parse_lines;
use benchmark_cli::report::{self, Rejection, render};
use clap::{Parser, ValueEnum};
use std::fmt::{self, Display};
use std::io;

#[derive(Parser, Debug)]
#[command(version, about = "Validate eval-schema v2 JSONL results")]
struct Args {
    /// Path to the eval-schema v2 JSONL results
    #[arg(short, long)]
    input: String,

    /// Show chosen config keys as columns in the group table
    #[arg(long, value_delimiter = ',')]
    group_by: Vec<String>,

    /// Output format (json is machine-readable for the dashboard)
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

#[derive(Clone, ValueEnum, Debug, PartialEq, Eq)]
enum Format {
    Text,
    Json,
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
    let mut rejections: Vec<Rejection> = parsed_result
        .errors
        .iter()
        .map(|error| Rejection {
            line: error.line_number,
            field: "<json>".to_string(),
            reason: error.source_error.to_string(),
        })
        .collect();
    let mut resolver = RecordResolver::default();
    for record in parsed_result.records {
        match resolver.validate_record(record) {
            Ok(_) => accepted += 1,
            Err(error) => {
                rejections.push(Rejection {
                    line: error.line,
                    field: error.field,
                    reason: error.reason,
                });
            }
        }
    }
    rejections.sort_by_key(|rejection| rejection.line);
    for rejection in &rejections {
        eprintln!("{rejection}");
    }
    let rejected = rejections.len();

    let groups = if rejected == 0 {
        aggregate_records(resolver.effective_measurements())
    } else {
        Vec::new()
    };
    let output_string = if args.format == Format::Json {
        report::render_json(
            accepted,
            rejected,
            resolver.superseded_count(),
            &groups,
            &args.group_by,
            &rejections,
        )
    } else {
        render(
            accepted,
            rejected,
            resolver.superseded_count(),
            &groups,
            &args.group_by,
        )
    };
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
