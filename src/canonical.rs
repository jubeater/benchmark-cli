use serde_json::Value;
use sha2::{Digest, Sha256};

/// Compact JSON with recursively sorted object keys. Keep serde_json's
/// `preserve_order` feature disabled so its maps retain this ordering.
pub fn canonical_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("JSON values can be serialized")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn config_hash(config: &Value) -> String {
    sha256(&canonical_bytes(config))
}

fn fingerprint(block: Option<&Value>, fields: &[&str]) -> String {
    let Some(block) = block.filter(|value| !value.is_null()) else {
        return "null".to_string();
    };
    let selected = fields
        .iter()
        .filter_map(|field| {
            block
                .get(*field)
                .map(|value| ((*field).to_string(), value.clone()))
        })
        .collect();
    sha256(&canonical_bytes(&Value::Object(selected)))
}

/// Fingerprint the original JSON block, preserving absent versus null fields.
pub fn evaluator_fingerprint(block: Option<&Value>) -> String {
    fingerprint(block, &["kind", "name", "version", "prompt_version"])
}

/// Fingerprint only the dataset fields that define measurement identity.
pub fn dataset_fingerprint(block: Option<&Value>) -> String {
    fingerprint(
        block,
        &[
            "name",
            "version",
            "preprocessing",
            "split",
            "sample_ids_hash",
        ],
    )
}

/// Canonical identity bytes from an original, schema-validated JSON record.
pub fn measurement_key_bytes(record: &Value) -> Vec<u8> {
    canonical_bytes(&serde_json::json!([
        record["run_id"],
        record["metric"],
        config_hash(&record["config"]),
        record["seed"],
        dataset_fingerprint(record.get("dataset")),
        evaluator_fingerprint(record.get("evaluator"))
    ]))
}

pub fn measurement_key(record: &Value) -> String {
    sha256(&measurement_key_bytes(record))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reference_record() -> Value {
        json!({
            "run_id": "2026-09-16T14:22:01Z-tinygpt-a3f19c",
            "metric": "accuracy",
            "config": {"kv_cache": true, "batch_size": 8},
            "seed": 1337,
            "evaluator": {"kind": "exact_match", "name": "squad-em", "version": "1.0"}
        })
    }

    #[test]
    fn measurement_matches_schema_reference_bytes_and_hash() {
        let record = reference_record();
        assert_eq!(
            measurement_key_bytes(&record),
            br#"["2026-09-16T14:22:01Z-tinygpt-a3f19c","accuracy","93afd630e53e552b4d9f2dbd70b01dc6facef11913bd0cb0377da3d7ded73ef9",1337,"null","699a317181a5136a9c92b0dc41ea9512838f8869ce017392feca821d7f396b3e"]"#
        );
        assert_eq!(
            measurement_key(&record),
            "afc77d8a7a799b6d3a0488138e5920481a96df4b6f7250aeb5e78d8dcee6ee2d"
        );
    }

    #[test]
    fn object_key_order_is_irrelevant_recursively_in_configs_and_blocks() {
        let first: Value = serde_json::from_str(
            r#"{
            "run_id":"run", "metric":"score", "seed":1,
            "config":{"z":[{"b":2,"a":1}],"a":true},
            "dataset":{"name":"data","preprocessing":{"z":2,"a":1},"version":"1"},
            "evaluator":{"kind":"exact_match","name":"em","version":"1"}
        }"#,
        )
        .unwrap();
        let reordered: Value = serde_json::from_str(
            r#"{
            "evaluator":{"version":"1","name":"em","kind":"exact_match"},
            "dataset":{"version":"1","preprocessing":{"a":1,"z":2},"name":"data"},
            "config":{"a":true,"z":[{"a":1,"b":2}]},
            "seed":1, "metric":"score", "run_id":"run"
        }"#,
        )
        .unwrap();
        assert_eq!(
            canonical_bytes(&first["config"]),
            br#"{"a":true,"z":[{"a":1,"b":2}]}"#
        );
        assert_eq!(
            config_hash(&first["config"]),
            config_hash(&reordered["config"])
        );
        assert_eq!(
            dataset_fingerprint(first.get("dataset")),
            dataset_fingerprint(reordered.get("dataset"))
        );
        assert_eq!(
            evaluator_fingerprint(first.get("evaluator")),
            evaluator_fingerprint(reordered.get("evaluator"))
        );
        assert_eq!(measurement_key(&first), measurement_key(&reordered));
    }

    #[test]
    fn config_preserves_null_missing_and_integer_float_distinctions() {
        assert_ne!(config_hash(&json!({"key": null})), config_hash(&json!({})));
        assert_eq!(
            canonical_bytes(&json!({"batch_size": 8.0})),
            br#"{"batch_size":8.0}"#
        );
        assert_ne!(
            config_hash(&json!({"batch_size": 8})),
            config_hash(&json!({"batch_size": 8.0}))
        );
    }

    #[test]
    fn fingerprints_omit_absent_fields_preserve_null_and_ignore_unlisted_fields() {
        for fingerprint in [dataset_fingerprint, evaluator_fingerprint] {
            assert_eq!(fingerprint(None), "null");
            assert_eq!(fingerprint(Some(&Value::Null)), "null");
            let mut block = json!({"name": "reference", "version": "1"});
            let original = fingerprint(Some(&block));
            // These fields belong to blocks but are excluded from identity.
            block["n"] = json!(999);
            block["split_manifest"] = json!("other.json");
            block["temperature"] = json!(0.7);
            block["unknown"] = json!(true);
            assert_eq!(fingerprint(Some(&block)), original);
            block["version"] = Value::Null;
            let explicit_null = fingerprint(Some(&block));
            block.as_object_mut().unwrap().remove("version");
            assert_ne!(fingerprint(Some(&block)), explicit_null);
        }
        let mut evaluator = reference_record()["evaluator"].clone();
        let omitted = evaluator_fingerprint(Some(&evaluator));
        evaluator["prompt_version"] = Value::Null;
        assert_ne!(evaluator_fingerprint(Some(&evaluator)), omitted);
    }

    #[test]
    fn each_identity_component_changes_the_measurement_key() {
        use std::collections::HashSet;

        for (field, variants) in [
            ("seed", vec![Value::Null, json!(42), json!(1337)]),
            (
                "dataset",
                vec![
                    Value::Null,
                    json!({"name":"first"}),
                    json!({"name":"second"}),
                ],
            ),
            (
                "evaluator",
                vec![
                    Value::Null,
                    json!({"name":"first"}),
                    json!({"name":"second"}),
                ],
            ),
        ] {
            let keys: HashSet<_> = variants
                .into_iter()
                .map(|variant| {
                    let mut record = reference_record();
                    record[field] = variant;
                    measurement_key(&record)
                })
                .collect();
            assert_eq!(
                keys.len(),
                3,
                "distinct {field} values must remain distinct"
            );
        }
        let record = reference_record();
        for (field, value) in [
            ("run_id", json!("other-run")),
            ("metric", json!("loss")),
            ("config", json!({})),
        ] {
            let mut changed = record.clone();
            changed[field] = value;
            assert_ne!(measurement_key(&changed), measurement_key(&record));
        }
        let mut null_blocks = record.clone();
        null_blocks["dataset"] = Value::Null;
        assert_eq!(measurement_key(&null_blocks), measurement_key(&record));
        null_blocks.as_object_mut().unwrap().remove("evaluator");
        let absent = measurement_key(&null_blocks);
        null_blocks["evaluator"] = Value::Null;
        assert_eq!(measurement_key(&null_blocks), absent);
    }

    #[test]
    fn evaluator_matches_schema_reference_without_prompt_version() {
        let evaluator = json!({"kind": "exact_match", "name": "squad-em", "version": "1.0"});
        assert_eq!(
            canonical_bytes(&evaluator),
            br#"{"kind":"exact_match","name":"squad-em","version":"1.0"}"#
        );
        assert_eq!(
            evaluator_fingerprint(Some(&evaluator)),
            "699a317181a5136a9c92b0dc41ea9512838f8869ce017392feca821d7f396b3e"
        );
    }

    #[test]
    fn config_matches_schema_reference_bytes_and_hash() {
        let config = json!({"kv_cache": true, "batch_size": 8});
        assert_eq!(
            canonical_bytes(&config),
            br#"{"batch_size":8,"kv_cache":true}"#
        );
        assert_eq!(
            config_hash(&config),
            "93afd630e53e552b4d9f2dbd70b01dc6facef11913bd0cb0377da3d7ded73ef9"
        );
    }
}
