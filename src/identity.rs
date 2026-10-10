use crate::canonical::measurement_key;
use crate::input::InputRecord;
use crate::model::EvalRecordV2;
use crate::validator::{InvalidRecord, validate_record};
use std::collections::HashMap;

/// Resolve duplicates and corrections in file order, keeping one effective record per key.
#[derive(Default)]
pub struct RecordResolver {
    measurements: HashMap<String, Measurement>,
    record_keys: HashMap<String, String>,
    superseded: usize,
}

struct Measurement {
    first_line: usize,
    record: EvalRecordV2,
}

impl RecordResolver {
    pub fn validate_record(&mut self, record: InputRecord) -> Result<(), InvalidRecord> {
        let validated = validate_record(InputRecord {
            line: record.line,
            value: record.value.clone(),
        })?;
        // Use the original JSON: typed optional fields lose absent/null distinctions.
        let key = measurement_key(&record.value);
        if let Some(target) = &validated.supersedes_record_id {
            let target_key = self.record_keys.get(target).ok_or_else(|| InvalidRecord {
                line: record.line,
                field: "supersedes_record_id".to_string(),
                reason: format!("dangling reference: record '{target}' was not accepted earlier"),
            })?;
            if !self.measurements.contains_key(&key) {
                return Err(InvalidRecord {
                    line: record.line,
                    field: "supersedes_record_id".to_string(),
                    reason: "nothing to correct: measurement_key has not been seen".to_string(),
                });
            }
            if target_key != &key {
                return Err(InvalidRecord {
                    line: record.line,
                    field: "supersedes_record_id".to_string(),
                    reason: format!("target record '{target}' has a different measurement_key"),
                });
            }
        }
        if validated.supersedes_record_id.is_none()
            && let Some(measurement) = self.measurements.get(&key)
        {
            return Err(InvalidRecord {
                line: record.line,
                field: "measurement_key".to_string(),
                reason: format!(
                    "duplicate measurement (first seen on line {})",
                    measurement.first_line
                ),
            });
        }
        self.record_keys
            .insert(validated.record_id.clone(), key.clone());
        if let Some(measurement) = self.measurements.get_mut(&key) {
            measurement.record = validated;
            self.superseded += 1;
        } else {
            self.measurements.insert(
                key,
                Measurement {
                    first_line: record.line,
                    record: validated,
                },
            );
        }
        Ok(())
    }

    /// Only these records should be passed on to aggregation.
    pub fn effective_records(&self) -> impl Iterator<Item = &EvalRecordV2> {
        self.measurements
            .values()
            .map(|measurement| &measurement.record)
    }

    /// Accepted records displaced by later corrections, including forks.
    pub fn superseded_count(&self) -> usize {
        self.superseded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_line;

    fn record(line: usize) -> InputRecord {
        let mut record = parse_line(
            include_str!("../data/v2/valid.jsonl")
                .lines()
                .next()
                .unwrap(),
            line,
        )
        .unwrap();
        record.value["record_id"] = serde_json::json!(format!("record-{line}"));
        record
    }

    #[test]
    fn ordinary_unseen_measurement_is_accepted() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        assert_eq!(resolver.effective_records().count(), 1);
        assert_eq!(resolver.superseded_count(), 0);
    }

    #[test]
    fn later_measurements_with_the_same_key_are_rejected() {
        let mut resolver = RecordResolver::default();
        assert!(resolver.validate_record(record(1)).is_ok());
        let mut duplicate = record(4);
        duplicate.value["record_id"] = serde_json::json!("another-record");
        duplicate.value["value"] = serde_json::json!(0.5);
        let error = resolver.validate_record(duplicate).unwrap_err();
        assert_eq!(error.line, 4);
        assert_eq!(error.field, "measurement_key");
        assert!(error.reason.contains("duplicate measurement"));
        assert!(error.reason.contains("line 1"));
        assert!(
            resolver
                .validate_record(record(5))
                .unwrap_err()
                .reason
                .contains("line 1")
        );
    }

    #[test]
    fn invalid_records_do_not_reserve_measurement_keys() {
        let mut resolver = RecordResolver::default();
        let mut invalid = record(1);
        invalid.value.as_object_mut().unwrap().remove("unit");
        assert_eq!(resolver.validate_record(invalid).unwrap_err().field, "unit");
        assert!(resolver.validate_record(record(2)).is_ok());
    }

    #[test]
    fn correction_with_a_missing_target_is_rejected() {
        let mut resolver = RecordResolver::default();
        assert!(resolver.validate_record(record(1)).is_ok());
        let mut correction = record(2);
        correction.value["supersedes_record_id"] = serde_json::json!("missing-record");
        let error = resolver.validate_record(correction).unwrap_err();
        assert_eq!(error.line, 2);
        assert_eq!(error.field, "supersedes_record_id");
        assert!(error.reason.contains("dangling reference"));
    }

    #[test]
    fn correction_targeting_a_different_measurement_is_rejected() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        let mut other = record(2);
        other.value["seed"] = serde_json::json!(42);
        resolver.validate_record(other).unwrap();

        let mut correction = record(3);
        correction.value["supersedes_record_id"] = serde_json::json!("record-2");
        let error = resolver.validate_record(correction).unwrap_err();
        assert_eq!(error.field, "supersedes_record_id");
        assert!(error.reason.contains("different measurement_key"));
    }

    #[test]
    fn valid_correction_replaces_the_effective_measurement() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        let mut correction = record(2);
        correction.value["supersedes_record_id"] = serde_json::json!("record-1");
        correction.value["value"] = serde_json::json!(0.8);
        resolver.validate_record(correction).unwrap();

        let effective: Vec<_> = resolver.effective_records().collect();
        assert_eq!(effective.len(), 1);
        assert_eq!(effective[0].record_id, "record-2");
        assert_eq!(effective[0].value, serde_json::json!(0.8));
        assert_eq!(resolver.superseded_count(), 1);
    }

    #[test]
    fn correction_of_an_unseen_measurement_is_rejected() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        let mut correction = record(2);
        correction.value["seed"] = serde_json::json!(42);
        correction.value["supersedes_record_id"] = serde_json::json!("record-1");
        let error = resolver.validate_record(correction).unwrap_err();
        assert_eq!(error.field, "supersedes_record_id");
        assert!(error.reason.contains("nothing to correct"));

        // Rejected corrections must not reserve their measurement key.
        let mut ordinary = record(3);
        ordinary.value["seed"] = serde_json::json!(42);
        resolver.validate_record(ordinary).unwrap();
        assert_eq!(resolver.effective_records().count(), 2);
        assert_eq!(resolver.superseded_count(), 0);
    }

    #[test]
    fn two_link_correction_chain_keeps_only_the_last_record() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        for (line, target, value) in [(2, "record-1", 0.8), (3, "record-2", 0.7)] {
            let mut correction = record(line);
            correction.value["supersedes_record_id"] = serde_json::json!(target);
            correction.value["value"] = serde_json::json!(value);
            resolver.validate_record(correction).unwrap();
        }
        let effective: Vec<_> = resolver.effective_records().collect();
        assert_eq!(effective.len(), 1);
        assert_eq!(effective[0].record_id, "record-3");
        assert_eq!(effective[0].value, serde_json::json!(0.7));
        assert_eq!(resolver.superseded_count(), 2);
    }

    #[test]
    fn later_fork_wins_even_when_the_target_is_already_superseded() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        for line in [2, 3] {
            let mut correction = record(line);
            correction.value["supersedes_record_id"] = serde_json::json!("record-1");
            resolver.validate_record(correction).unwrap();
        }
        assert_eq!(resolver.effective_records().count(), 1);
        assert_eq!(
            resolver.effective_records().next().unwrap().record_id,
            "record-3"
        );
        assert_eq!(resolver.superseded_count(), 2);
    }

    #[test]
    fn correction_before_its_target_is_rejected_without_reserving_identity() {
        let mut resolver = RecordResolver::default();
        let mut correction = record(1);
        correction.value["supersedes_record_id"] = serde_json::json!("record-2");
        assert!(
            resolver
                .validate_record(correction)
                .unwrap_err()
                .reason
                .contains("dangling reference")
        );
        resolver.validate_record(record(2)).unwrap();
        assert_eq!(
            resolver.effective_records().next().unwrap().record_id,
            "record-2"
        );
        assert_eq!(resolver.superseded_count(), 0);
    }

    #[test]
    fn rejected_records_cannot_be_correction_targets() {
        let mut resolver = RecordResolver::default();
        resolver.validate_record(record(1)).unwrap();
        assert!(resolver.validate_record(record(2)).is_err());
        let mut correction = record(3);
        correction.value["supersedes_record_id"] = serde_json::json!("record-2");
        assert!(
            resolver
                .validate_record(correction)
                .unwrap_err()
                .reason
                .contains("dangling reference")
        );
        assert_eq!(
            resolver.effective_records().next().unwrap().record_id,
            "record-1"
        );
        assert_eq!(resolver.superseded_count(), 0);
    }

    #[test]
    fn null_supersedes_is_an_ordinary_record_and_still_rejects_duplicates() {
        let mut resolver = RecordResolver::default();
        let mut ordinary = record(1);
        ordinary.value["supersedes_record_id"] = serde_json::Value::Null;
        resolver.validate_record(ordinary).unwrap();
        let mut duplicate = record(2);
        duplicate.value["supersedes_record_id"] = serde_json::Value::Null;
        assert_eq!(
            resolver.validate_record(duplicate).unwrap_err().field,
            "measurement_key"
        );
    }

    #[test]
    fn fingerprints_are_built_from_original_json_not_deserialized_options() {
        let mut resolver = RecordResolver::default();
        let mut absent = record(1);
        absent.value["evaluator"] =
            serde_json::json!({"kind":"exact_match", "name":"em", "version":"1"});
        let mut explicit_null = record(2);
        explicit_null.value["evaluator"] = absent.value["evaluator"].clone();
        explicit_null.value["evaluator"]["prompt_version"] = serde_json::Value::Null;
        assert!(resolver.validate_record(absent).is_ok());
        assert!(resolver.validate_record(explicit_null).is_ok());
    }
}
