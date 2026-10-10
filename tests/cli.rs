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
fn identity_fixture_rejects_invalid_corrections_and_reports_superseded_records() {
    let output = run_fixture("identity.jsonl");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 3\nRejected records: 3\nSuperseded records: 1\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "line 2: measurement_key: duplicate measurement (first seen on line 1)\n\
         line 4: supersedes_record_id: dangling reference: record 'missing-record' was not accepted earlier\n\
         line 6: supersedes_record_id: target record 'identity-original' has a different measurement_key\n"
    );
}

#[test]
fn valid_v2_records_are_accepted() {
    let output = run_fixture("valid.jsonl");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 4\nRejected records: 0\nSuperseded records: 1\n\n\
         run_id                               | metric   | unit  | config_hash                                                      | dataset_fingerprint                                              | evaluator_fingerprint                                            | n_attempted | n_ok | median | min  | max \n\
         -------------------------------------+----------+-------+------------------------------------------------------------------+------------------------------------------------------------------+------------------------------------------------------------------+-------------+------+--------+------+-----\n\
         00000000-0000-4000-8000-000000000010 | accuracy | ratio | 297b1e2373ca8181a16df4aaa7b57e6f27a232e43aa658d798d97e9246dbe749 | null                                                             | null                                                             |           1 |    0 |    N/A |  N/A |  N/A\n\
         00000000-0000-4000-8000-000000000010 | accuracy | ratio | fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf | null                                                             | null                                                             |           1 |    1 |   0.96 | 0.96 | 0.96\n\
         00000000-0000-4000-8000-000000000010 | latency  | ms    | fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf | c29baa9dd3734637cf7ee495e5748ebf6867ce199a5a111959eb6fb00bef9439 | 09aaf53bdfbd9e26c3b81aad829ed47647944c34ad24bf10f5919936a910f7db |           1 |    1 |   12.5 | 12.5 | 12.5\n\n\
         warning: group (run_id: 00000000-0000-4000-8000-000000000010, metric: accuracy, config_hash: 297b1e2373ca8181a16df4aaa7b57e6f27a232e43aa658d798d97e9246dbe749) has fewer than 3 ok seeds (n_ok = 0)\n\
         warning: group (run_id: 00000000-0000-4000-8000-000000000010, metric: accuracy, config_hash: fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf) has fewer than 3 ok seeds (n_ok = 1)\n\
         warning: group (run_id: 00000000-0000-4000-8000-000000000010, metric: latency, config_hash: fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf) has fewer than 3 ok seeds (n_ok = 1)\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn parse_and_validation_failures_are_counted_as_rejected_records() {
    let output = run_fixture("mixed.jsonl");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 1\nRejected records: 2\nSuperseded records: 0\n"
    );
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("line 2:"));
    assert!(errors.contains("line 3:"));
}

#[test]
fn sweep_fixture_aggregates_groups_with_hand_computed_medians_and_warning() {
    let output = run_fixture("sweep.jsonl");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "Eval-schema v2 validation\nAccepted records: 10\nRejected records: 0\nSuperseded records: 0\n"
    ));
    assert!(output.stderr.is_empty());

    // Three groups are displayed with hand-computed medians
    // batch_size 1: n_attempted = 3, n_ok = 3, median = 102
    assert!(stdout.contains("3 |    3 |    102 | 100 | 105"));
    // batch_size 8: n_attempted = 3 (oom counted in attempted), n_ok = 2, median = 205 (mean of middle two)
    assert!(stdout.contains("3 |    2 |    205 | 200 | 210"));
    // batch_size 32: n_attempted = 3, n_ok = 3, median = 320
    assert!(stdout.contains("3 |    3 |    320 | 300 | 350"));

    // Group with 2 ok seeds triggers warning
    assert!(stdout.contains("warning: group (run_id: 2026-10-09T14:00:00Z-sweep-001, metric: tokens_per_sec, config_hash: fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf) has fewer than 3 ok seeds (n_ok = 2)"));

    // Aggregate record appears nowhere (exactly 3 group rows)
    let group_rows: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("2026-10-09T14:00:00Z-sweep-001"))
        .collect();
    assert_eq!(
        group_rows.len(),
        3,
        "expected exactly 3 groups, aggregate record must not appear"
    );
}

#[test]
fn invalid_records_report_line_field_and_reason() {
    let output = run_fixture("invalid.jsonl");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 0\nRejected records: 11\nSuperseded records: 0\n"
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
