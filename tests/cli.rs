use std::path::Path;
use std::process::{Command, Output};

fn run_fixture(name: &str) -> Output {
    run_fixture_with_args(name, &[])
}

fn run_fixture_with_args(name: &str, extra_args: &[&str]) -> Output {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data/v2")
        .join(name);
    run_input_with_args(&fixture, extra_args)
}

fn run_input(path: impl AsRef<Path>) -> Output {
    run_input_with_args(path, &[])
}

fn run_input_with_args(path: impl AsRef<Path>, extra_args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_benchmark-cli"))
        .arg("--input")
        .arg(path.as_ref())
        .args(extra_args)
        .output()
        .expect("CLI should run")
}

#[test]
fn text_uses_short_hashes_for_all_fingerprints_and_warnings() {
    let output = run_fixture("valid.jsonl");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let hashes: Vec<Vec<_>> = stdout
        .lines()
        .filter(|line| line.starts_with("00000000-0000-4000-8000-000000000010 |"))
        .map(|line| line.split('|').skip(3).take(3).map(str::trim).collect())
        .collect();
    assert_eq!(
        hashes,
        vec![
            vec!["297b1e2373ca", "null", "null"],
            vec!["fc046d91d2e2", "null", "null"],
            vec!["fc046d91d2e2", "c29baa9dd373", "09aaf53bdfbd"],
        ]
    );
    let warnings: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect();
    assert_eq!(warnings.len(), 3);
    assert!(warnings[0].contains("config_hash: 297b1e2373ca)"));
    assert!(warnings[1].contains("config_hash: fc046d91d2e2)"));
    assert!(warnings[2].contains("config_hash: fc046d91d2e2)"));
}

#[test]
fn group_by_sorts_typed_values_then_other_keys_independently_of_input_order() {
    use serde_json::json;
    // Deliberately give numerically larger values lexically earlier run IDs.
    let cases = [
        ("group-negative-fraction", json!({"size": -2.5})),
        ("group-negative-integer", json!({"size": -2})),
        ("group-zero", json!({"size": 0})),
        ("group-secondary-b", json!({"size": 1, "label": 2})),
        ("group-secondary-a", json!({"size": 1, "label": 10})),
        ("group-tie-a", json!({"size": 8.0})),
        ("group-tie-b", json!({"size": 8})),
        ("group-large-b", json!({"size": 9007199254740992_u64})),
        ("group-large-a", json!({"size": 9007199254740993_u64})),
        ("group-string-a", json!({"size": "alpha"})),
        ("group-string-b", json!({"size": "beta"})),
        ("group-bool-false", json!({"size": false})),
        ("group-bool-true", json!({"size": true})),
        ("group-array", json!({"size": [1, 2]})),
        ("group-object", json!({"size": {"nested": 1}})),
        ("group-null", json!({"size": null})),
        ("group-absent", json!({})),
    ];
    let template: serde_json::Value = serde_json::from_str(
        include_str!("../data/v2/identity.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let records: Vec<_> = cases
        .iter()
        .map(|(run_id, config)| {
            let mut record = template.clone();
            record["record_id"] = json!(run_id);
            record["run_id"] = json!(run_id);
            record["config"] = config.clone();
            record.to_string()
        })
        .collect();
    let path = std::env::temp_dir().join(format!(
        "benchmark-cli-group-order-{}.jsonl",
        std::process::id()
    ));
    std::fs::write(&path, records.join("\n")).unwrap();
    let args = ["--group-by", "size", "--group-by", "label"];
    let sorted = run_input_with_args(&path, &args);
    let ungrouped = run_input(&path);
    let repeated = run_input(&path);
    std::fs::write(
        &path,
        records.into_iter().rev().collect::<Vec<_>>().join("\n"),
    )
    .unwrap();
    let reversed_sorted = run_input_with_args(&path, &args);
    let reversed_ungrouped = run_input(&path);
    std::fs::remove_file(path).unwrap();

    assert_eq!(sorted.status.code(), Some(0));
    assert!(sorted.stderr.is_empty());
    assert_eq!(sorted.stdout, reversed_sorted.stdout);
    assert_eq!(ungrouped.stdout, repeated.stdout);
    assert_eq!(ungrouped.stdout, reversed_ungrouped.stdout);
    let stdout = String::from_utf8(sorted.stdout).unwrap();
    let row_order: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with("group-"))
        .map(|line| line.split('|').next().unwrap().trim())
        .collect();
    assert_eq!(
        row_order,
        cases.iter().map(|(run_id, _)| *run_id).collect::<Vec<_>>()
    );
}

#[test]
fn readme_sweep_tables_match_real_cli_output() {
    let readme = include_str!("../readme.md");
    for args in [vec![], vec!["--group-by", "batch_size"]] {
        let output = run_fixture_with_args("sweep.jsonl", &args);
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let (_, table) = stdout.split_once("\n\n").unwrap();
        assert!(readme.contains(&format!("```text\n{table}```")));
    }
}

#[test]
fn blank_lines_are_skipped_in_both_output_formats() {
    for format in ["text", "json"] {
        let output = run_fixture_with_args("blank-lines.jsonl", &["--format", format]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());

        if format == "json" {
            let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(parsed["accepted"], 4);
            assert_eq!(parsed["rejected"], 0);
            assert_eq!(parsed["superseded"], 1);
            assert_eq!(parsed["complete"], true);
            assert_eq!(parsed["rejections"], serde_json::json!([]));
        } else {
            let stdout = String::from_utf8(output.stdout.clone()).unwrap();
            assert!(stdout.starts_with(
                "Eval-schema v2 validation\nAccepted records: 4\nRejected records: 0\nSuperseded records: 1\n"
            ));
            assert!(!stdout.contains("Incomplete result"));
        }

        let clean = run_fixture_with_args("valid.jsonl", &["--format", format]);
        assert_eq!(clean.status.code(), Some(0));
        assert_eq!(output.stdout, clean.stdout);
    }
}

#[test]
fn rejections_after_blank_lines_report_physical_line_numbers() {
    for format in ["text", "json"] {
        let output = run_fixture_with_args("blank-lines-invalid.jsonl", &["--format", format]);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).unwrap();
        let errors: Vec<_> = stderr.lines().collect();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0], "line 6: schema_version: missing required field");
        assert!(errors[1].starts_with("line 9: <json>:"));

        if format == "json" {
            let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(parsed["accepted"], 1);
            assert_eq!(parsed["rejected"], 2);
            assert_eq!(parsed["complete"], false);
            assert_eq!(parsed["rejections"][0]["line"], 6);
            assert_eq!(parsed["rejections"][0]["field"], "schema_version");
            assert_eq!(parsed["rejections"][1]["line"], 9);
            assert_eq!(parsed["rejections"][1]["field"], "<json>");
        } else {
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(stdout.starts_with(
                "Eval-schema v2 validation\nAccepted records: 1\nRejected records: 2\nSuperseded records: 0\n"
            ));
        }
    }
}

#[test]
fn identity_fixture_rejects_invalid_corrections_and_reports_superseded_records() {
    let output = run_fixture("identity.jsonl");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "Eval-schema v2 validation\nAccepted records: 3\nRejected records: 3\nSuperseded records: 1\n\n\
         Incomplete result: statistics exclude rejected records (count: 3).\n\n"
    ));
    assert!(stdout.contains("2 |    2 |    0.8 | 0.7 | 0.9"));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "line 2: measurement_key: duplicate measurement (first seen on line 1)\n\
         line 4: supersedes_record_id: dangling reference: record 'missing-record' was not accepted earlier\n\
         line 6: supersedes_record_id: target record 'identity-original' has a different measurement_key\n"
    );
}

#[test]
fn reused_record_id_fixture_rejects_reuse_and_accepts_the_originals_correction() {
    let output = run_fixture("record-id-reuse.jsonl");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "Eval-schema v2 validation\nAccepted records: 2\nRejected records: 1\nSuperseded records: 1\n\n\
         Incomplete result: statistics exclude rejected records (count: 1).\n\n"
    ));
    assert!(stdout.contains("1 |    1 |    0.9 | 0.9 | 0.9"));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "line 2: record_id: duplicate record_id (first seen on line 1)\n"
    );

    let output = run_fixture_with_args("record-id-reuse.jsonl", &["--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "line 2: record_id: duplicate record_id (first seen on line 1)\n"
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        parsed,
        serde_json::json!({
            "output_version": 2,
            "accepted": 2,
            "rejected": 1,
            "superseded": 1,
            "complete": false,
            "rejections": [
                { "line": 2, "field": "record_id", "reason": "duplicate record_id (first seen on line 1)" }
            ],
            "groups": [{
                "run_id": "00000000-0000-4000-8000-000000000010",
                "metric": "accuracy",
                "unit": "ratio",
                "config_hash": "fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf",
                "config": {"batch_size": 8},
                "dataset_fingerprint": "null",
                "evaluator_fingerprint": "null",
                "n_attempted": 1,
                "n_ok": 1,
                "median": 0.9,
                "min": 0.9,
                "max": 0.9,
                "warning": true
            }]
        })
    );
}

#[test]
fn valid_v2_records_are_accepted() {
    let output = run_fixture("valid.jsonl");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 4\nRejected records: 0\nSuperseded records: 1\n\n\
         run_id                               | metric   | unit  | config_hash  | dataset_fingerprint | evaluator_fingerprint | n_attempted | n_ok | median | min  | max \n\
         -------------------------------------+----------+-------+--------------+---------------------+-----------------------+-------------+------+--------+------+-----\n\
         00000000-0000-4000-8000-000000000010 | accuracy | ratio | 297b1e2373ca | null                | null                  |           1 |    0 |    N/A |  N/A |  N/A\n\
         00000000-0000-4000-8000-000000000010 | accuracy | ratio | fc046d91d2e2 | null                | null                  |           1 |    1 |   0.96 | 0.96 | 0.96\n\
         00000000-0000-4000-8000-000000000010 | latency  | ms    | fc046d91d2e2 | c29baa9dd373        | 09aaf53bdfbd          |           1 |    1 |   12.5 | 12.5 | 12.5\n\n\
         warning: group (run_id: 00000000-0000-4000-8000-000000000010, metric: accuracy, config_hash: 297b1e2373ca) has fewer than 3 ok seeds (n_ok = 0)\n\
         warning: group (run_id: 00000000-0000-4000-8000-000000000010, metric: accuracy, config_hash: fc046d91d2e2) has fewer than 3 ok seeds (n_ok = 1)\n\
         warning: group (run_id: 00000000-0000-4000-8000-000000000010, metric: latency, config_hash: fc046d91d2e2) has fewer than 3 ok seeds (n_ok = 1)\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn parse_and_validation_failures_are_counted_as_rejected_records() {
    let output = run_fixture("mixed.jsonl");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "Eval-schema v2 validation\nAccepted records: 1\nRejected records: 2\nSuperseded records: 0\n\n\
         Incomplete result: statistics exclude rejected records (count: 2).\n\n"
    ));
    assert!(stdout.contains("1 |    0 |    N/A | N/A | N/A"));
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
    assert!(stdout.contains("warning: group (run_id: 2026-10-09T14:00:00Z-sweep-001, metric: tokens_per_sec, config_hash: fc046d91d2e2) has fewer than 3 ok seeds (n_ok = 2)"));

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
fn partial_sweep_json_keeps_accepted_groups_and_marks_the_result_incomplete() {
    let args = ["--format", "json", "--group-by", "batch_size"];
    let output = run_fixture_with_args("sweep-partial.jsonl", &args);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("line 11: <json>:")
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["accepted"], 10);
    assert_eq!(parsed["rejected"], 1);
    assert_eq!(parsed["superseded"], 0);
    assert_eq!(parsed["complete"], false);
    assert_eq!(parsed["rejections"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["rejections"][0]["line"], 11);
    assert_eq!(parsed["rejections"][0]["field"], "<json>");

    let groups = parsed["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 3);
    // Sorted by config hash: batch sizes 32, 1, 8. Expected stats are worked
    // out next to the clean sweep fixture, including the OOM and aggregate exclusions.
    for (group, (batch_size, n_ok, median, min, max)) in groups.iter().zip([
        (32, 3, 320.0, 300.0, 350.0),
        (1, 3, 102.0, 100.0, 105.0),
        (8, 2, 205.0, 200.0, 210.0),
    ]) {
        assert_eq!(group["group_by"]["batch_size"], batch_size);
        assert_eq!(group["n_attempted"], 3);
        assert_eq!(group["n_ok"], n_ok);
        assert_eq!(group["median"], median);
        assert_eq!(group["min"], min);
        assert_eq!(group["max"], max);
        assert_eq!(group["warning"], n_ok < 3);
    }
    let clean = run_fixture_with_args("sweep.jsonl", &args);
    let clean: serde_json::Value = serde_json::from_slice(&clean.stdout).unwrap();
    assert_eq!(parsed["groups"], clean["groups"]);
}

#[test]
fn partial_sweep_text_shows_a_notice_above_the_unchanged_group_table() {
    let output = run_fixture_with_args("sweep-partial.jsonl", &["--group-by", "batch_size"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("line 11: <json>:")
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let (summary, table) = stdout.split_once("run_id").unwrap();
    assert_eq!(
        summary,
        "Eval-schema v2 validation\nAccepted records: 10\nRejected records: 1\nSuperseded records: 0\n\n\
         Incomplete result: statistics exclude rejected records (count: 1).\n\n"
    );
    assert!(table.contains("32         |           3 |    3 |    320 | 300 | 350"));
    assert!(table.contains("1          |           3 |    3 |    102 | 100 | 105"));
    assert!(table.contains("8          |           3 |    2 |    205 | 200 | 210"));
    assert_eq!(
        table
            .lines()
            .filter(|line| line.starts_with("2026-10-09T14:00:00Z-sweep-001"))
            .count(),
        3
    );

    let clean = run_fixture_with_args("sweep.jsonl", &["--group-by", "batch_size"]);
    let clean = String::from_utf8(clean.stdout).unwrap();
    assert!(!clean.contains("Incomplete result"));
    assert_eq!(table, clean.split_once("run_id").unwrap().1);
}

#[test]
fn schema_rejected_correction_leaves_its_target_in_the_partial_statistics() {
    let original_line = include_str!("../data/v2/identity.jsonl")
        .lines()
        .next()
        .unwrap();
    let mut correction: serde_json::Value = serde_json::from_str(original_line).unwrap();
    correction["record_id"] = serde_json::json!("invalid-correction");
    correction["supersedes_record_id"] = serde_json::json!("identity-original");
    correction["unit"] = serde_json::json!("kilograms");
    correction["value"] = serde_json::json!(0.1);
    let temp_file = std::env::temp_dir().join(format!(
        "benchmark-cli-rejected-correction-{}.jsonl",
        std::process::id()
    ));
    std::fs::write(&temp_file, format!("{original_line}\n{correction}\n")).unwrap();
    let output = run_input_with_args(&temp_file, &["--format", "json"]);
    std::fs::remove_file(temp_file).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("line 2: unit:")
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["accepted"], 1);
    assert_eq!(parsed["rejected"], 1);
    assert_eq!(parsed["superseded"], 0);
    assert_eq!(parsed["complete"], false);
    assert_eq!(parsed["rejections"][0]["field"], "unit");
    let groups = parsed["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["n_attempted"], 1);
    assert_eq!(groups[0]["n_ok"], 1);
    assert_eq!(groups[0]["median"], 0.95);
    assert_eq!(groups[0]["min"], 0.95);
    assert_eq!(groups[0]["max"], 0.95);
}

#[test]
fn invalid_records_report_line_field_and_reason() {
    let output = run_fixture("invalid.jsonl");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Eval-schema v2 validation\nAccepted records: 0\nRejected records: 11\nSuperseded records: 0\n\n\
         Incomplete result: statistics exclude rejected records (count: 11).\n"
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
fn valid_fixture_json_snapshot_matches_text_output() {
    let output = run_fixture_with_args("valid.jsonl", &["--format", "json"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON output must parse");
    assert_eq!(parsed["complete"], serde_json::json!(true));
    assert_eq!(
        stdout,
        "{\n\
         \x20 \"output_version\": 2,\n\
         \x20 \"accepted\": 4,\n\
         \x20 \"rejected\": 0,\n\
         \x20 \"superseded\": 1,\n\
         \x20 \"complete\": true,\n\
         \x20 \"rejections\": [],\n\
         \x20 \"groups\": [\n\
         \x20   {\n\
         \x20     \"run_id\": \"00000000-0000-4000-8000-000000000010\",\n\
         \x20     \"metric\": \"accuracy\",\n\
         \x20     \"unit\": \"ratio\",\n\
         \x20     \"config_hash\": \"297b1e2373ca8181a16df4aaa7b57e6f27a232e43aa658d798d97e9246dbe749\",\n\
         \x20     \"config\": {\n\
         \x20       \"batch_size\": 32\n\
         \x20     },\n\
         \x20     \"dataset_fingerprint\": \"null\",\n\
         \x20     \"evaluator_fingerprint\": \"null\",\n\
         \x20     \"n_attempted\": 1,\n\
         \x20     \"n_ok\": 0,\n\
         \x20     \"median\": null,\n\
         \x20     \"min\": null,\n\
         \x20     \"max\": null,\n\
         \x20     \"warning\": true\n\
         \x20   },\n\
         \x20   {\n\
         \x20     \"run_id\": \"00000000-0000-4000-8000-000000000010\",\n\
         \x20     \"metric\": \"accuracy\",\n\
         \x20     \"unit\": \"ratio\",\n\
         \x20     \"config_hash\": \"fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf\",\n\
         \x20     \"config\": {\n\
         \x20       \"batch_size\": 8\n\
         \x20     },\n\
         \x20     \"dataset_fingerprint\": \"null\",\n\
         \x20     \"evaluator_fingerprint\": \"null\",\n\
         \x20     \"n_attempted\": 1,\n\
         \x20     \"n_ok\": 1,\n\
         \x20     \"median\": 0.96,\n\
         \x20     \"min\": 0.96,\n\
         \x20     \"max\": 0.96,\n\
         \x20     \"warning\": true\n\
         \x20   },\n\
         \x20   {\n\
         \x20     \"run_id\": \"00000000-0000-4000-8000-000000000010\",\n\
         \x20     \"metric\": \"latency\",\n\
         \x20     \"unit\": \"ms\",\n\
         \x20     \"config_hash\": \"fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf\",\n\
         \x20     \"config\": {\n\
         \x20       \"batch_size\": 8\n\
         \x20     },\n\
         \x20     \"dataset_fingerprint\": \"c29baa9dd3734637cf7ee495e5748ebf6867ce199a5a111959eb6fb00bef9439\",\n\
         \x20     \"evaluator_fingerprint\": \"09aaf53bdfbd9e26c3b81aad829ed47647944c34ad24bf10f5919936a910f7db\",\n\
         \x20     \"n_attempted\": 1,\n\
         \x20     \"n_ok\": 1,\n\
         \x20     \"median\": 12.5,\n\
         \x20     \"min\": 12.5,\n\
         \x20     \"max\": 12.5,\n\
         \x20     \"warning\": true\n\
         \x20   }\n\
         \x20 ]\n\
         }\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_fixture_json_reports_complete_false_and_structured_rejections() {
    let output = run_fixture_with_args("invalid.jsonl", &["--format", "json"]);

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON output must parse");

    assert_eq!(
        parsed,
        serde_json::json!({
            "output_version": 2,
            "accepted": 0,
            "rejected": 11,
            "superseded": 0,
            "complete": false,
            "rejections": [
                { "line": 1, "field": "metric", "reason": "missing required field" },
                { "line": 2, "field": "seed", "reason": "invalid type: string \"1337\", expected u64" },
                { "line": 3, "field": "status", "reason": "unknown variant `bogus`, expected one of `ok`, `failed`, `timeout`, `oom`, `cancelled`" },
                { "line": 4, "field": "dataset.split", "reason": "missing field `split`" },
                { "line": 5, "field": "<json>", "reason": "EOF while parsing a value at line 1 column 18" },
                { "line": 6, "field": "unit", "reason": "missing required field" },
                { "line": 7, "field": "unit", "reason": "unknown variant `kilograms`, expected one of `ms`, `s`, `tokens_per_sec`, `items_per_sec`, `mb`, `gb`, `ratio`, `count`, `usd`, `score`" },
                { "line": 8, "field": "env", "reason": "unit requires env block" },
                { "line": 9, "field": "value", "reason": "value must be a finite number when status is ok" },
                { "line": 10, "field": "unknown_key", "reason": "unknown top-level field" },
                { "line": 11, "field": "config", "reason": "-0.0 is not allowed in config" }
            ],
            "groups": []
        })
    );
}

#[test]
fn identity_fixture_json_snapshot_reports_corrections_and_rejections() {
    let output = run_fixture_with_args("identity.jsonl", &["--format", "json"]);

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON output must parse");

    assert_eq!(
        parsed,
        serde_json::json!({
            "output_version": 2,
            "accepted": 3,
            "rejected": 3,
            "superseded": 1,
            "complete": false,
            "rejections": [
                { "line": 2, "field": "measurement_key", "reason": "duplicate measurement (first seen on line 1)" },
                { "line": 4, "field": "supersedes_record_id", "reason": "dangling reference: record 'missing-record' was not accepted earlier" },
                { "line": 6, "field": "supersedes_record_id", "reason": "target record 'identity-original' has a different measurement_key" }
            ],
            "groups": [{
                "run_id": "00000000-0000-4000-8000-000000000010",
                "metric": "accuracy",
                "unit": "ratio",
                "config_hash": "fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf",
                "config": {"batch_size": 8},
                "dataset_fingerprint": "null",
                "evaluator_fingerprint": "null",
                "n_attempted": 2,
                "n_ok": 2,
                "median": 0.8,
                "min": 0.7,
                "max": 0.9,
                "warning": true
            }]
        })
    );
}

#[test]
fn sweep_fixture_json_snapshot_omits_group_by_unless_requested() {
    let output = run_fixture_with_args("sweep.jsonl", &["--format", "json"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON output must parse");

    assert_eq!(
        parsed,
        serde_json::json!({
            "output_version": 2,
            "accepted": 10,
            "rejected": 0,
            "superseded": 0,
            "complete": true,
            "rejections": [],
            "groups": [
                {
                    "run_id": "2026-10-09T14:00:00Z-sweep-001",
                    "metric": "tokens_per_sec",
                    "unit": "tokens_per_sec",
                    "config_hash": "297b1e2373ca8181a16df4aaa7b57e6f27a232e43aa658d798d97e9246dbe749",
                    "config": {"batch_size": 32},
                    "dataset_fingerprint": "null",
                    "evaluator_fingerprint": "null",
                    "n_attempted": 3,
                    "n_ok": 3,
                    "median": 320.0,
                    "min": 300.0,
                    "max": 350.0,
                    "warning": false
                },
                {
                    "run_id": "2026-10-09T14:00:00Z-sweep-001",
                    "metric": "tokens_per_sec",
                    "unit": "tokens_per_sec",
                    "config_hash": "db8be9fbe8f84ff4d2e61f9cf1bfa233deaabd47ecca04dbc7d065c66a175395",
                    "config": {"batch_size": 1},
                    "dataset_fingerprint": "null",
                    "evaluator_fingerprint": "null",
                    "n_attempted": 3,
                    "n_ok": 3,
                    "median": 102.0,
                    "min": 100.0,
                    "max": 105.0,
                    "warning": false
                },
                {
                    "run_id": "2026-10-09T14:00:00Z-sweep-001",
                    "metric": "tokens_per_sec",
                    "unit": "tokens_per_sec",
                    "config_hash": "fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf",
                    "config": {"batch_size": 8},
                    "dataset_fingerprint": "null",
                    "evaluator_fingerprint": "null",
                    "n_attempted": 3,
                    "n_ok": 2,
                    "median": 205.0,
                    "min": 200.0,
                    "max": 210.0,
                    "warning": true
                }
            ]
        })
    );

    let output = run_fixture_with_args("sweep.jsonl", &["--format", "json", "--group-by", "batch_size"]);
    assert_eq!(output.status.code(), Some(0));
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).unwrap())
            .expect("grouped JSON output must parse");
    let groups = parsed["groups"].as_array().unwrap();
    assert_eq!(groups[0]["group_by"], serde_json::json!({ "batch_size": 32 }));
    assert_eq!(groups[1]["group_by"], serde_json::json!({ "batch_size": 1 }));
    assert_eq!(groups[2]["group_by"], serde_json::json!({ "batch_size": 8 }));
}

#[test]
fn json_groups_preserve_full_config_and_integer_float_distinction() {
    let mut record: serde_json::Value = serde_json::from_str(
        include_str!("../data/v2/identity.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let config = serde_json::json!({
        "batch_size": 8.0,
        "count": 8,
        "model": "llama-3",
        "adapter": null,
        "enabled": true,
        "nested": {"values": [8, 8.0, null, "label"]}
    });
    record["config"] = config.clone();
    let temp_file = std::env::temp_dir().join(format!(
        "benchmark-cli-json-full-config-{}.jsonl",
        std::process::id()
    ));
    std::fs::write(&temp_file, format!("{record}\n")).unwrap();
    let outputs = [
        run_input_with_args(&temp_file, &["--format", "json"]),
        run_input_with_args(
            &temp_file,
            &["--format", "json", "--group-by", "batch_size,adapter,missing"],
        ),
    ];
    std::fs::remove_file(temp_file).unwrap();

    for (index, output) in outputs.into_iter().enumerate() {
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let groups = parsed["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        let group = &groups[0];
        assert_eq!(group["config"], config);
        assert_eq!(group["config"]["batch_size"].to_string(), "8.0");
        assert_eq!(group["config"]["count"].to_string(), "8");
        assert_eq!(
            group["config"]["nested"]["values"].to_string(),
            "[8,8.0,null,\"label\"]"
        );
        if index == 0 {
            assert!(group.get("group_by").is_none());
        } else {
            assert_eq!(
                group["group_by"],
                serde_json::json!({"batch_size": 8.0, "adapter": null})
            );
        }
    }
}

#[test]
fn json_group_by_keeps_null_and_absent_config_keys_distinct() {
    let dir = std::env::temp_dir();
    let temp_file = dir.join("test_json_group_by_distinct.jsonl");
    let line1 = serde_json::json!({
        "schema_version": "2.0",
        "record_id": "00000000-0000-4000-8000-000000000902",
        "run_id": "run-test",
        "project": "benchmark",
        "timestamp": "2026-10-09T12:00:00Z",
        "git_commit": "79e18e325900d533d0e90ccc6ec469a03ece1cb7",
        "git_dirty": false,
        "config": {"model": "llama-3", "adapter": null},
        "seed": 1,
        "metric": "score",
        "unit": "score",
        "value": 0.85,
        "status": "ok",
        "notes": "test"
    });
    std::fs::write(&temp_file, format!("{line1}\n")).unwrap();

    let output = run_input_with_args(
        &temp_file,
        &["--format", "json", "--group-by", "model,adapter,unknown_key"],
    );

    let _ = std::fs::remove_file(temp_file);

    assert_eq!(output.status.code(), Some(0));
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output.stdout).unwrap())
            .expect("JSON output must parse");
    let group_by = parsed["groups"][0]["group_by"]
        .as_object()
        .expect("group_by columns present when requested");
    assert_eq!(group_by.get("model"), Some(&serde_json::json!("llama-3")));
    assert_eq!(group_by.get("adapter"), Some(&serde_json::Value::Null));
    assert_eq!(group_by.get("unknown_key"), None, "absent key must be omitted");
}

#[test]
fn missing_input_file_exits_2_with_clear_message() {
    let output = run_input("data/v2/does-not-exist.jsonl");

    assert_eq!(output.status.code(), Some(2));
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("failed to load input file"), "got: {errors}");
    assert!(errors.contains("does-not-exist.jsonl"), "got: {errors}");
}

#[test]
fn sweep_fixture_with_group_by_batch_size_sorts_rows_and_keeps_statistics() {
    let output = run_fixture_with_args("sweep.jsonl", &["--group-by", "batch_size"]);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "Eval-schema v2 validation\nAccepted records: 10\nRejected records: 0\nSuperseded records: 0\n"
    ));
    assert!(output.stderr.is_empty());

    // Header contains batch_size column between evaluator_fingerprint and n_attempted
    let header_line = stdout.lines().nth(5).unwrap();
    assert!(header_line.contains("evaluator_fingerprint | batch_size | n_attempted"));

    let rows: Vec<Vec<_>> = stdout
        .lines()
        .filter(|line| line.starts_with("2026-10-09T14:00:00Z-sweep-001 |"))
        .map(|line| line.split('|').map(str::trim).collect())
        .collect();
    assert_eq!(
        rows.iter().map(|row| row[6]).collect::<Vec<_>>(),
        ["1", "8", "32"]
    );
    assert_eq!(
        rows.iter().map(|row| row[3]).collect::<Vec<_>>(),
        ["db8be9fbe8f8", "fc046d91d2e2", "297b1e2373ca"]
    );

    // Check each row contains its batch_size value and the exact same statistics as without the flag
    assert!(stdout.contains("32         |           3 |    3 |    320 | 300 | 350"));
    assert!(stdout.contains("1          |           3 |    3 |    102 | 100 | 105"));
    assert!(stdout.contains("8          |           3 |    2 |    205 | 200 | 210"));

    // Warning is still printed
    assert!(stdout.contains("warning: group (run_id: 2026-10-09T14:00:00Z-sweep-001, metric: tokens_per_sec, config_hash: fc046d91d2e2) has fewer than 3 ok seeds (n_ok = 2)"));
}

#[test]
fn group_by_supports_multiple_keys_and_renders_absent_and_null() {
    let dir = std::env::temp_dir();
    let temp_file = dir.join("test_group_by_multiple_keys.jsonl");
    let line1 = serde_json::json!({
        "schema_version": "2.0",
        "record_id": "00000000-0000-4000-8000-000000000901",
        "run_id": "run-test",
        "project": "benchmark",
        "timestamp": "2026-10-09T12:00:00Z",
        "git_commit": "79e18e325900d533d0e90ccc6ec469a03ece1cb7",
        "git_dirty": false,
        "config": {"model": "llama-3", "adapter": null},
        "seed": 1,
        "metric": "score",
        "unit": "score",
        "value": 0.85,
        "status": "ok",
        "notes": "test"
    });
    std::fs::write(&temp_file, format!("{line1}\n")).unwrap();

    let output = run_input_with_args(
        &temp_file,
        &["--group-by", "model", "--group-by", "adapter,unknown_key"],
    );

    let _ = std::fs::remove_file(temp_file);

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let header_line = stdout.lines().nth(5).unwrap();
    assert!(header_line.contains("evaluator_fingerprint | model   | adapter | unknown_key | n_attempted"));
    let row_line = stdout.lines().nth(7).unwrap();
    assert!(row_line.contains("llama-3 | null    | absent      |"));
}
