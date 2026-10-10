use std::path::Path;
use std::process::{Command, Output};

fn run_fixture(name: &str) -> Output {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data/v2")
        .join(name);
    Command::new(env!("CARGO_BIN_EXE_AI-log-analysis"))
        .arg("--input")
        .arg(fixture)
        .output()
        .expect("CLI should run")
}

fn run_input(path: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_AI-log-analysis"))
        .arg("--input")
        .arg(path)
        .output()
        .expect("CLI should run")
}

#[test]
fn valid_v2_records_are_accepted() {
    let output = run_fixture("valid.jsonl");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 3\nRejected records: 0\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn parse_and_validation_failures_are_counted_as_rejected_records() {
    let output = run_fixture("mixed.jsonl");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 1\nRejected records: 2\n"
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("line 2:"));
    assert!(errors.contains("line 3:"));
}

#[test]
fn invalid_records_report_line_field_and_reason() {
    let output = run_fixture("invalid.jsonl");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 0\nRejected records: 5\n"
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<&str> = errors.lines().collect();
    assert_eq!(lines.len(), 5, "expected one rejection per line, got:\n{errors}");
    assert!(lines[0].starts_with("line 1: metric:"), "got: {}", lines[0]);
    assert!(lines[1].starts_with("line 2: seed:"), "got: {}", lines[1]);
    assert!(lines[2].starts_with("line 3: status:"), "got: {}", lines[2]);
    assert!(
        lines[3].starts_with("line 4: dataset.split:"),
        "got: {}",
        lines[3]
    );
    assert!(lines[4].starts_with("line 5: <json>:"), "got: {}", lines[4]);
}

#[test]
fn missing_input_file_exits_2_with_clear_message() {
    let output = run_input("data/v2/does-not-exist.jsonl");

    assert_eq!(output.status.code(), Some(2));
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(
        errors.contains("failed to load input file"),
        "got: {errors}"
    );
    assert!(errors.contains("does-not-exist.jsonl"), "got: {errors}");
}
