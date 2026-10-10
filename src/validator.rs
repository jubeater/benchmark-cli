use crate::input::InputRecord;
use crate::model::{
    Artifacts, Comparison, Cost, Dataset, Env, EvalRecordV2, Evaluator, Model, SchemaVersion,
    Status, Unit,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::fmt::{self, Display};

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidRecord {
    pub line: usize,
    pub field: String,
    pub reason: String,
}

impl Display for InvalidRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}: {}", self.line, self.field, self.reason)
    }
}

pub type ValidationResult = Result<EvalRecordV2, InvalidRecord>;

const REQUIRED_FIELDS: [&str; 14] = [
    "schema_version",
    "record_id",
    "run_id",
    "project",
    "timestamp",
    "git_commit",
    "git_dirty",
    "config",
    "seed",
    "metric",
    "unit",
    "value",
    "status",
    "notes",
];

const OPTIONAL_TOP_LEVEL_FIELDS: [&str; 8] = [
    "supersedes_record_id",
    "dataset",
    "model",
    "evaluator",
    "artifacts",
    "cost",
    "env",
    "comparison",
];

fn missing_field(line: usize, field: &str) -> InvalidRecord {
    InvalidRecord {
        line,
        field: field.to_string(),
        reason: "missing required field".to_string(),
    }
}

fn is_finite_number(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_f64().is_some_and(|f| f.is_finite()),
        _ => false,
    }
}

fn has_negative_zero(val: &Value) -> bool {
    match val {
        Value::Number(n) => n.as_f64().is_some_and(|f| f == 0.0 && f.is_sign_negative()),
        Value::Array(arr) => arr.iter().any(has_negative_zero),
        Value::Object(map) => map.values().any(has_negative_zero),
        _ => false,
    }
}

/// Extract the field name from a serde `missing field `X`` error, if present.
fn extract_missing_field(err: &serde_json::Error) -> Option<String> {
    let msg = err.to_string();
    let marker = "missing field `";
    let start = msg.find(marker)? + marker.len();
    let end = msg[start..].find('`')? + start;
    Some(msg[start..end].to_string())
}

fn check_core_field<T: DeserializeOwned>(obj: &Map<String, Value>, field: &str) -> Option<String> {
    let value = obj.get(field)?;
    serde_json::from_value::<T>(value.clone())
        .err()
        .map(|e| e.to_string())
}

/// On a top-level deserialization failure, find the first core field that
/// does not deserialize to its expected type and report it.
fn core_field_error(line: usize, obj: &Map<String, Value>, fallback: String) -> InvalidRecord {
    let checks: Vec<(&str, Option<String>)> = vec![
        (
            "schema_version",
            check_core_field::<SchemaVersion>(obj, "schema_version"),
        ),
        ("record_id", check_core_field::<String>(obj, "record_id")),
        ("run_id", check_core_field::<String>(obj, "run_id")),
        ("project", check_core_field::<String>(obj, "project")),
        ("timestamp", check_core_field::<String>(obj, "timestamp")),
        (
            "git_commit",
            check_core_field::<String>(obj, "git_commit"),
        ),
        ("git_dirty", check_core_field::<bool>(obj, "git_dirty")),
        (
            "config",
            check_core_field::<Map<String, Value>>(obj, "config"),
        ),
        ("seed", check_core_field::<Option<u64>>(obj, "seed")),
        ("metric", check_core_field::<String>(obj, "metric")),
        ("unit", check_core_field::<Unit>(obj, "unit")),
        ("status", check_core_field::<Status>(obj, "status")),
        ("notes", check_core_field::<String>(obj, "notes")),
        (
            "supersedes_record_id",
            check_core_field::<Option<String>>(obj, "supersedes_record_id"),
        ),
    ];
    for (field, err) in checks {
        if let Some(reason) = err {
            return InvalidRecord {
                line,
                field: field.to_string(),
                reason,
            };
        }
    }
    // `value` accepts any JSON; if nothing else failed, keep the full error.
    InvalidRecord {
        line,
        field: "<record>".to_string(),
        reason: fallback,
    }
}

fn validate_optional_block<T: DeserializeOwned>(
    line: usize,
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), InvalidRecord> {
    let Some(value) = object.get(field) else {
        return Ok(());
    };

    if value.is_null() {
        return Ok(());
    }

    match serde_json::from_value::<T>(value.clone()) {
        Ok(_) => Ok(()),
        Err(e) => {
            let reason = e.to_string();
            let nested = extract_missing_field(&e).map(|n| format!("{field}.{n}"));
            Err(InvalidRecord {
                line,
                field: nested.unwrap_or_else(|| field.to_string()),
                reason,
            })
        }
    }
}

pub fn validate_record(record: InputRecord) -> ValidationResult {
    let InputRecord { line, value } = record;
    let obj = match value {
        Value::Object(map) => map,
        _ => {
            return Err(InvalidRecord {
                line,
                field: "<json>".to_string(),
                reason: "expected JSON object".to_string(),
            });
        }
    };

    for field in REQUIRED_FIELDS {
        if !obj.contains_key(field) {
            return Err(missing_field(line, field));
        }
    }

    for key in obj.keys() {
        if !REQUIRED_FIELDS.contains(&key.as_str())
            && !OPTIONAL_TOP_LEVEL_FIELDS.contains(&key.as_str())
        {
            return Err(InvalidRecord {
                line,
                field: key.clone(),
                reason: "unknown top-level field".to_string(),
            });
        }
    }

    if serde_json::from_value::<Unit>(obj["unit"].clone())
        .is_ok_and(|unit| unit.requires_env())
    {
        match obj.get("env") {
            None | Some(Value::Null) => {
                return Err(InvalidRecord {
                    line,
                    field: "env".to_string(),
                    reason: "unit requires env block".to_string(),
                });
            }
            _ => {}
        }
    }

    if let Ok(status) = serde_json::from_value::<Status>(obj["status"].clone()) {
        let value = &obj["value"];
        match status {
            Status::Ok => {
                if !is_finite_number(value) {
                    return Err(InvalidRecord {
                        line,
                        field: "value".to_string(),
                        reason: "value must be a finite number when status is ok".to_string(),
                    });
                }
            }
            _ => {
                if !value.is_null() && !is_finite_number(value) {
                    return Err(InvalidRecord {
                        line,
                        field: "value".to_string(),
                        reason: "value must be a finite number or null".to_string(),
                    });
                }
            }
        }
    }

    if obj.get("config").is_some_and(has_negative_zero) {
        return Err(InvalidRecord {
            line,
            field: "config".to_string(),
            reason: "-0.0 is not allowed in config".to_string(),
        });
    }

    validate_optional_block::<Dataset>(line, &obj, "dataset")?;
    validate_optional_block::<Model>(line, &obj, "model")?;
    validate_optional_block::<Evaluator>(line, &obj, "evaluator")?;
    validate_optional_block::<Artifacts>(line, &obj, "artifacts")?;
    validate_optional_block::<Cost>(line, &obj, "cost")?;
    validate_optional_block::<Env>(line, &obj, "env")?;
    validate_optional_block::<Comparison>(line, &obj, "comparison")?;

    match serde_json::from_value::<EvalRecordV2>(Value::Object(obj.clone())) {
        Ok(record) => Ok(record),
        Err(e) => {
            if let Some(nested) = extract_missing_field(&e) {
                return Err(InvalidRecord {
                    line,
                    field: nested,
                    reason: e.to_string(),
                });
            }
            Err(core_field_error(line, &obj, e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const OPTIONAL_FIELDS: [&str; 7] = [
        "dataset",
        "model",
        "evaluator",
        "artifacts",
        "cost",
        "env",
        "comparison",
    ];

    fn core_record(seed: Value) -> Value {
        json!({
            "schema_version": "2.0",
            "record_id": "record-1",
            "run_id": "run-1",
            "project": "benchmark",
            "timestamp": "2026-09-21T12:00:00Z",
            "git_commit": "abc123",
            "git_dirty": false,
            "config": {},
            "seed": seed,
            "metric": "accuracy",
            "unit": "ratio",
            "value": 0.95,
            "status": "ok",
            "notes": ""
        })
    }

    #[test]
    fn unit_is_required_and_enforces_closed_vocabulary() {
        let mut without_unit = core_record(Value::from(1337));
        without_unit.as_object_mut().unwrap().remove("unit");
        let Err(err) = validate_record(InputRecord {
            line: 20,
            value: without_unit,
        }) else {
            panic!("missing unit should be rejected");
        };
        assert_eq!(err.line, 20);
        assert_eq!(err.field, "unit");
        assert_eq!(err.reason, "missing required field");

        let mut invalid_unit = core_record(Value::from(1337));
        invalid_unit["unit"] = json!("kilograms");
        let Err(err) = validate_record(InputRecord {
            line: 21,
            value: invalid_unit,
        }) else {
            panic!("unknown unit should be rejected");
        };
        assert_eq!(err.line, 21);
        assert_eq!(err.field, "unit");
    }

    #[test]
    fn time_throughput_memory_units_require_env_block() {
        let env_requiring_units = ["ms", "s", "tokens_per_sec", "items_per_sec", "mb", "gb"];
        for unit in env_requiring_units {
            let mut record = core_record(Value::from(1337));
            record["unit"] = json!(unit);
            let Err(err) = validate_record(InputRecord {
                line: 30,
                value: record.clone(),
            }) else {
                panic!("unit {unit} without env should be rejected");
            };
            assert_eq!(err.line, 30);
            assert_eq!(err.field, "env");

            record["env"] = Value::Null;
            let Err(err_null) = validate_record(InputRecord {
                line: 31,
                value: record.clone(),
            }) else {
                panic!("unit {unit} with null env should be rejected");
            };
            assert_eq!(err_null.line, 31);
            assert_eq!(err_null.field, "env");

            record["env"] = valid_optional_block("env");
            assert!(
                validate_record(InputRecord {
                    line: 32,
                    value: record,
                })
                .is_ok(),
                "unit {unit} with valid env should pass"
            );
        }
    }

    #[test]
    fn supersedes_record_id_is_optional_string() {
        let mut with_str = core_record(Value::from(1337));
        with_str["supersedes_record_id"] = json!("00000000-0000-4000-8000-000000000001");
        let res = validate_record(InputRecord {
            line: 35,
            value: with_str,
        });
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap().supersedes_record_id,
            Some("00000000-0000-4000-8000-000000000001".to_string())
        );

        let mut with_null = core_record(Value::from(1337));
        with_null["supersedes_record_id"] = Value::Null;
        let res = validate_record(InputRecord {
            line: 36,
            value: with_null,
        });
        assert!(res.is_ok());
        assert_eq!(res.unwrap().supersedes_record_id, None);

        let mut with_num = core_record(Value::from(1337));
        with_num["supersedes_record_id"] = json!(12345);
        let Err(err) = validate_record(InputRecord {
            line: 37,
            value: with_num,
        }) else {
            panic!("numeric supersedes_record_id should be rejected");
        };
        assert_eq!(err.line, 37);
        assert_eq!(err.field, "supersedes_record_id");
    }

    #[test]
    fn value_must_be_finite_number_when_status_is_ok_and_may_be_null_otherwise() {
        let mut string_val = core_record(Value::from(1337));
        string_val["status"] = json!("ok");
        string_val["value"] = json!("not-a-number");
        let Err(err) = validate_record(InputRecord {
            line: 40,
            value: string_val,
        }) else {
            panic!("string value with status ok should be rejected");
        };
        assert_eq!(err.line, 40);
        assert_eq!(err.field, "value");

        let mut null_val = core_record(Value::from(1337));
        null_val["status"] = json!("ok");
        null_val["value"] = Value::Null;
        let Err(err) = validate_record(InputRecord {
            line: 41,
            value: null_val,
        }) else {
            panic!("null value with status ok should be rejected");
        };
        assert_eq!(err.line, 41);
        assert_eq!(err.field, "value");

        let mut valid_val = core_record(Value::from(1337));
        valid_val["status"] = json!("ok");
        valid_val["value"] = json!(12.34);
        assert!(
            validate_record(InputRecord {
                line: 42,
                value: valid_val,
            })
            .is_ok()
        );

        let mut oom_null = core_record(Value::from(1337));
        oom_null["status"] = json!("oom");
        oom_null["value"] = Value::Null;
        assert!(
            validate_record(InputRecord {
                line: 43,
                value: oom_null,
            })
            .is_ok()
        );

        let mut oom_num = core_record(Value::from(1337));
        oom_num["status"] = json!("oom");
        oom_num["value"] = json!(0.0);
        assert!(
            validate_record(InputRecord {
                line: 44,
                value: oom_num,
            })
            .is_ok()
        );

        let mut oom_str = core_record(Value::from(1337));
        oom_str["status"] = json!("oom");
        oom_str["value"] = json!("failed");
        let Err(err) = validate_record(InputRecord {
            line: 45,
            value: oom_str,
        }) else {
            panic!("string value with status oom should be rejected");
        };
        assert_eq!(err.line, 45);
        assert_eq!(err.field, "value");
    }

    #[test]
    fn negative_zero_in_config_is_rejected() {
        let raw = r#"{
            "schema_version": "2.0",
            "record_id": "record-1",
            "run_id": "run-1",
            "project": "benchmark",
            "timestamp": "2026-09-21T12:00:00Z",
            "git_commit": "abc123",
            "git_dirty": false,
            "config": { "lr": -0.0 },
            "seed": 1337,
            "metric": "accuracy",
            "unit": "ratio",
            "value": 0.95,
            "status": "ok",
            "notes": ""
        }"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let Err(err) = validate_record(InputRecord { line: 50, value }) else {
            panic!("-0.0 in config should be rejected");
        };
        assert_eq!(err.line, 50);
        assert_eq!(err.field, "config");

        // Nested in array or object
        let raw_nested = r#"{
            "schema_version": "2.0",
            "record_id": "record-1",
            "run_id": "run-1",
            "project": "benchmark",
            "timestamp": "2026-09-21T12:00:00Z",
            "git_commit": "abc123",
            "git_dirty": false,
            "config": { "nested": { "arr": [-0.0] } },
            "seed": 1337,
            "metric": "accuracy",
            "unit": "ratio",
            "value": 0.95,
            "status": "ok",
            "notes": ""
        }"#;
        let nested_value: Value = serde_json::from_str(raw_nested).unwrap();
        let Err(err_nested) = validate_record(InputRecord {
            line: 51,
            value: nested_value,
        }) else {
            panic!("nested -0.0 in config should be rejected");
        };
        assert_eq!(err_nested.line, 51);
        assert_eq!(err_nested.field, "config");

        // Positive 0.0 is allowed
        let raw_pos = r#"{
            "schema_version": "2.0",
            "record_id": "record-1",
            "run_id": "run-1",
            "project": "benchmark",
            "timestamp": "2026-09-21T12:00:00Z",
            "git_commit": "abc123",
            "git_dirty": false,
            "config": { "lr": 0.0 },
            "seed": 1337,
            "metric": "accuracy",
            "unit": "ratio",
            "value": 0.95,
            "status": "ok",
            "notes": ""
        }"#;
        let pos_value: Value = serde_json::from_str(raw_pos).unwrap();
        assert!(
            validate_record(InputRecord {
                line: 52,
                value: pos_value,
            })
            .is_ok()
        );
    }

    #[test]
    fn missing_required_fields_report_field_and_reason() {
        for field in REQUIRED_FIELDS {
            let mut value = core_record(Value::from(1337));
            value.as_object_mut().unwrap().remove(field);

            let result = validate_record(InputRecord { line: 4, value });

            let Err(err) = result else {
                panic!("field {field} should be required");
            };
            assert_eq!(err.line, 4);
            assert_eq!(err.field, field);
            assert_eq!(err.reason, "missing required field");
        }
    }

    #[test]
    fn null_seed_is_valid() {
        let result = validate_record(InputRecord {
            line: 5,
            value: core_record(Value::Null),
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap().seed, None);
    }

    #[test]
    fn integer_seed_is_valid() {
        let result = validate_record(InputRecord {
            line: 6,
            value: core_record(Value::from(1337)),
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap().seed, Some(1337));
    }

    #[test]
    fn invalid_seed_type_reports_seed_field() {
        let result = validate_record(InputRecord {
            line: 7,
            value: core_record(json!("1337")),
        });

        let Err(err) = result else {
            panic!("string seed should be rejected");
        };
        assert_eq!(err.line, 7);
        assert_eq!(err.field, "seed");
        assert!(!err.reason.is_empty());
    }

    #[test]
    fn invalid_field_types_and_schema_versions_report_field() {
        let mut invalid_type = core_record(Value::from(1337));
        invalid_type["record_id"] = json!(42);
        let Err(err) = validate_record(InputRecord {
            line: 8,
            value: invalid_type,
        })
        else {
            panic!("numeric record_id should be rejected");
        };
        assert_eq!(err.line, 8);
        assert_eq!(err.field, "record_id");

        let mut invalid_version = core_record(Value::from(1337));
        invalid_version["schema_version"] = json!("3.0");
        let Err(err) = validate_record(InputRecord {
            line: 9,
            value: invalid_version,
        })
        else {
            panic!("unknown schema version should be rejected");
        };
        assert_eq!(err.line, 9);
        assert_eq!(err.field, "schema_version");
    }

    #[test]
    fn bad_enum_value_reports_status_field() {
        let mut value = core_record(Value::from(1));
        value["status"] = json!("bogus");
        let Err(err) = validate_record(InputRecord { line: 10, value }) else {
            panic!("bogus status should be rejected");
        };
        assert_eq!(err.line, 10);
        assert_eq!(err.field, "status");
        assert!(err.reason.contains("bogus"));
    }

    #[test]
    fn non_object_root_reports_json_field() {
        let result = validate_record(InputRecord {
            line: 10,
            value: json!(["not", "an", "object"]),
        });

        let Err(err) = result else {
            panic!("array root should be rejected");
        };
        assert_eq!(err.line, 10);
        assert_eq!(err.field, "<json>");
    }

    fn valid_optional_block(field: &str) -> Value {
        match field {
            "dataset" => json!({
                "name": "data",
                "version": "1",
                "preprocessing": "none",
                "split": "test",
                "split_manifest": "manifest.json",
                "n": 10,
                "sample_ids_hash": "hash"
            }),
            "model" => json!({
                "name": "model",
                "version": "1",
                "backend": "backend",
                "placement": "gpu_resident",
                "decoding_mode": "greedy"
            }),
            "evaluator" => json!({
                "kind": "exact_match",
                "name": "evaluator",
                "version": "1"
            }),
            "artifacts" => json!({}),
            "cost" => json!({
                "latency_ms_p50": 10,
                "latency_ms_p95": 20,
                "input_tokens": 30,
                "output_tokens": 40,
                "usd": 0.5
            }),
            "env" => json!({
                "device": "cuda",
                "gpu": "A100",
                "vram_gb": 80,
                "torch": "2.0",
                "driver": "550"
            }),
            "comparison" => json!({
                "baseline_run_id": "baseline",
                "hypothesis": "better",
                "threshold": {
                    "metric": "accuracy",
                    "direction": "increase",
                    "min_delta_pct": 1.0
                },
                "declared_before_run": true
            }),
            _ => unreachable!("unknown optional field {field}"),
        }
    }

    #[test]
    fn optional_blocks_may_be_absent_null_or_valid() {
        assert!(
            validate_record(InputRecord {
                line: 11,
                value: core_record(Value::from(1337)),
            })
            .is_ok()
        );

        for field in OPTIONAL_FIELDS {
            let mut value = core_record(Value::from(1337));
            value[field] = Value::Null;
            assert!(
                validate_record(InputRecord { line: 11, value }).is_ok(),
                "null {field} should be accepted"
            );

            let mut value = core_record(Value::from(1337));
            value[field] = valid_optional_block(field);
            assert!(
                validate_record(InputRecord { line: 11, value }).is_ok(),
                "valid {field} should be accepted"
            );
        }
    }

    #[test]
    fn malformed_optional_blocks_report_block_field() {
        for field in OPTIONAL_FIELDS {
            let mut value = core_record(Value::from(1337));
            value[field] = json!("malformed");

            let result = validate_record(InputRecord { line: 12, value });

            let Err(err) = result else {
                panic!("field {field} should be an invalid optional block");
            };
            assert_eq!(err.line, 12);
            assert_eq!(err.field, field, "field {field} should name the block");
            assert!(!err.reason.is_empty());
        }
    }

    #[test]
    fn missing_nested_field_reports_dotted_path() {
        let mut value = core_record(Value::from(1337));
        let mut dataset = valid_optional_block("dataset");
        dataset.as_object_mut().unwrap().remove("split");
        value["dataset"] = dataset;

        let Err(err) = validate_record(InputRecord { line: 13, value }) else {
            panic!("dataset without split should be rejected");
        };
        assert_eq!(err.line, 13);
        assert_eq!(err.field, "dataset.split");
        assert!(!err.reason.is_empty());
    }

    #[test]
    fn unknown_top_level_key_is_rejected_but_nested_unknown_keys_are_accepted() {
        let mut value = core_record(Value::from(1337));
        value["unknown_top_level"] = json!(true);
        let Err(err) = validate_record(InputRecord { line: 14, value }) else {
            panic!("unknown top level key should be rejected");
        };
        assert_eq!(err.line, 14);
        assert_eq!(err.field, "unknown_top_level");

        let mut nested_value = core_record(Value::from(1337));
        nested_value["dataset"] = valid_optional_block("dataset");
        nested_value["dataset"]["unknown_nested"] = json!({"accepted": true});
        assert!(validate_record(InputRecord {
            line: 15,
            value: nested_value
        })
        .is_ok());
    }

    #[test]
    fn rejection_displays_line_field_and_reason() {
        let err = InvalidRecord {
            line: 2,
            field: "metric".to_string(),
            reason: "missing required field".to_string(),
        };
        assert_eq!(format!("{err}"), "line 2: metric: missing required field");
    }
}
