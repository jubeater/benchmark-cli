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

    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 1\nRejected records: 2\n"
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("line 2:"));
    assert!(errors.contains("line 3:"));
}
