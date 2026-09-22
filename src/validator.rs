use crate::input::InputRecord;
use crate::model::{Artifacts, Comparison, Cost, Dataset, Env, EvalRecordV2, Evaluator, Model};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

#[derive(Debug)]
pub enum ValidationErrorType {
    MissingRequiredField,
    InvalidField,
    InvalidOptionalBlock,
}

#[derive(Debug)]
pub struct InvalidRecord {
    pub line: usize,
    pub error_type: ValidationErrorType,
}

pub type ValidationResult = Result<EvalRecordV2, InvalidRecord>;

const REQUIRED_FIELDS: [&str; 13] = [
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
    "value",
    "status",
    "notes",
];

fn validate_optional_block<T: DeserializeOwned>(
    line: usize,
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), InvalidRecord> {
    let Some(value) = object.get(field) else {
        return Ok(());
    };

    if value.is_null() || serde_json::from_value::<T>(value.clone()).is_ok() {
        Ok(())
    } else {
        Err(InvalidRecord {
            line,
            error_type: ValidationErrorType::InvalidOptionalBlock,
        })
    }
}

pub fn validate_record(record: InputRecord) -> ValidationResult {
    let InputRecord { line, value } = record;
    let obj = match value {
        Value::Object(map) => map,
        _ => {
            return Err(InvalidRecord {
                line,
                error_type: ValidationErrorType::InvalidField,
            });
        }
    };

    for field in REQUIRED_FIELDS {
        if !obj.contains_key(field) {
            return Err(InvalidRecord {
                line,
                error_type: ValidationErrorType::MissingRequiredField,
            });
        }
    }

    validate_optional_block::<Dataset>(line, &obj, "dataset")?;
    validate_optional_block::<Model>(line, &obj, "model")?;
    validate_optional_block::<Evaluator>(line, &obj, "evaluator")?;
    validate_optional_block::<Artifacts>(line, &obj, "artifacts")?;
    validate_optional_block::<Cost>(line, &obj, "cost")?;
    validate_optional_block::<Env>(line, &obj, "env")?;
    validate_optional_block::<Comparison>(line, &obj, "comparison")?;

    serde_json::from_value(Value::Object(obj)).map_err(|_| InvalidRecord {
        line,
        error_type: ValidationErrorType::InvalidField,
    })
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
            "value": 0.95,
            "status": "ok",
            "notes": ""
        })
    }

    #[test]
    fn missing_required_fields_are_invalid() {
        for field in REQUIRED_FIELDS {
            let mut value = core_record(Value::from(1337));
            value.as_object_mut().unwrap().remove(field);

            let result = validate_record(InputRecord { line: 4, value });

            assert!(
                matches!(
                    result,
                    Err(InvalidRecord {
                        line: 4,
                        error_type: ValidationErrorType::MissingRequiredField,
                    })
                ),
                "field {field} should be required"
            );
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
    fn invalid_seed_type_is_invalid_field() {
        let result = validate_record(InputRecord {
            line: 7,
            value: core_record(json!("1337")),
        });

        assert!(matches!(
            result,
            Err(InvalidRecord {
                line: 7,
                error_type: ValidationErrorType::InvalidField,
            })
        ));
    }

    #[test]
    fn invalid_field_types_and_schema_versions_are_invalid_field() {
        let mut invalid_type = core_record(Value::from(1337));
        invalid_type["record_id"] = json!(42);
        let result = validate_record(InputRecord {
            line: 8,
            value: invalid_type,
        });
        assert!(matches!(
            result,
            Err(InvalidRecord {
                line: 8,
                error_type: ValidationErrorType::InvalidField,
            })
        ));

        let mut invalid_version = core_record(Value::from(1337));
        invalid_version["schema_version"] = json!("3.0");
        let result = validate_record(InputRecord {
            line: 9,
            value: invalid_version,
        });
        assert!(matches!(
            result,
            Err(InvalidRecord {
                line: 9,
                error_type: ValidationErrorType::InvalidField,
            })
        ));
    }

    #[test]
    fn non_object_root_is_invalid_field() {
        let result = validate_record(InputRecord {
            line: 10,
            value: json!(["not", "an", "object"]),
        });

        assert!(matches!(
            result,
            Err(InvalidRecord {
                line: 10,
                error_type: ValidationErrorType::InvalidField,
            })
        ));
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
    fn malformed_optional_blocks_are_invalid_optional_blocks() {
        for field in OPTIONAL_FIELDS {
            let mut value = core_record(Value::from(1337));
            value[field] = json!("malformed");

            let result = validate_record(InputRecord { line: 12, value });

            assert!(
                matches!(
                    result,
                    Err(InvalidRecord {
                        line: 12,
                        error_type: ValidationErrorType::InvalidOptionalBlock,
                    })
                ),
                "field {field} should be an invalid optional block"
            );
        }
    }

    #[test]
    fn unknown_top_level_and_nested_fields_are_accepted() {
        let mut value = core_record(Value::from(1337));
        value["unknown_top_level"] = json!(true);
        value["dataset"] = valid_optional_block("dataset");
        value["dataset"]["unknown_nested"] = json!({"accepted": true});

        assert!(validate_record(InputRecord { line: 13, value }).is_ok());
    }
}
