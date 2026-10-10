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
        "Eval-schema v2 validation\nAccepted records: 4\nRejected records: 0\n"
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
        "Eval-schema v2 validation\nAccepted records: 0\nRejected records: 11\n"
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<&str> = errors.lines().collect();
    let expected_prefixes = [
        "line 1: metric:",
        "line 2: seed:",
        "line 3: status:",
        "line 4: dataset.split:",
        "line 5: <json>:",
        "line 6: unit:",
        "line 7: unit:",
        "line 8: env:",
        "line 9: value:",
        "line 10: unknown_key:",
        "line 11: config:",
    ];
    for (i, prefix) in expected_prefixes.iter().enumerate() {
        assert!(
            lines[i].starts_with(prefix),
            "line {}: expected prefix '{}', got '{}'",
            i + 1,
            prefix,
            lines[i]
        );
    }
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
