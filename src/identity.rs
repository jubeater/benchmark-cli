use crate::canonical::measurement_key;
use crate::input::InputRecord;
use crate::validator::{InvalidRecord, ValidationResult, validate_record};
use std::collections::HashMap;

/// Track measurement keys in file order, admitting only schema-valid records.
#[derive(Default)]
pub struct DuplicateFilter {
    first_lines: HashMap<String, usize>,
}

impl DuplicateFilter {
    pub fn validate_record(&mut self, record: InputRecord) -> ValidationResult {
        let validated = validate_record(InputRecord {
            line: record.line,
            value: record.value.clone(),
        })?;
        // Use the original JSON: typed optional fields lose absent/null distinctions.
        let key = measurement_key(&record.value);
        if validated.supersedes_record_id.is_none()
            && let Some(first_line) = self.first_lines.get(&key)
        {
            return Err(InvalidRecord {
                line: record.line,
                field: "measurement_key".to_string(),
                reason: format!("duplicate measurement (first seen on line {first_line})"),
            });
        }
        self.first_lines.entry(key).or_insert(record.line);
        Ok(validated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_line;

    fn record(line: usize) -> InputRecord {
        parse_line(
            include_str!("../data/v2/valid.jsonl")
                .lines()
                .next()
                .unwrap(),
            line,
        )
        .unwrap()
    }

    #[test]
    fn later_measurements_with_the_same_key_are_rejected() {
        let mut filter = DuplicateFilter::default();
        assert!(filter.validate_record(record(1)).is_ok());
        let mut duplicate = record(4);
        duplicate.value["record_id"] = serde_json::json!("another-record");
        duplicate.value["value"] = serde_json::json!(0.5);
        let error = filter.validate_record(duplicate).unwrap_err();
        assert_eq!(error.line, 4);
        assert_eq!(error.field, "measurement_key");
        assert!(error.reason.contains("duplicate measurement"));
        assert!(error.reason.contains("line 1"));
        assert!(
            filter
                .validate_record(record(5))
                .unwrap_err()
                .reason
                .contains("line 1")
        );
    }

    #[test]
    fn invalid_records_do_not_reserve_measurement_keys() {
        let mut filter = DuplicateFilter::default();
        let mut invalid = record(1);
        invalid.value.as_object_mut().unwrap().remove("unit");
        assert_eq!(filter.validate_record(invalid).unwrap_err().field, "unit");
        assert!(filter.validate_record(record(2)).is_ok());
    }

    #[test]
    fn corrections_pass_through_and_still_mark_their_keys_as_seen() {
        let mut filter = DuplicateFilter::default();
        assert!(filter.validate_record(record(1)).is_ok());
        for target in ["original", "missing-record"] {
            let mut correction = record(2);
            correction.value["supersedes_record_id"] = serde_json::json!(target);
            assert!(filter.validate_record(correction).is_ok());
        }
        let mut null_correction = record(3);
        null_correction.value["supersedes_record_id"] = serde_json::Value::Null;
        assert!(filter.validate_record(null_correction).is_err());

        let mut filter = DuplicateFilter::default();
        let mut correction = record(1);
        correction.value["supersedes_record_id"] = serde_json::json!("missing-record");
        assert!(filter.validate_record(correction).is_ok());
        assert!(filter.validate_record(record(2)).is_err());
    }

    #[test]
    fn fingerprints_are_built_from_original_json_not_deserialized_options() {
        let mut filter = DuplicateFilter::default();
        let mut absent = record(1);
        absent.value["evaluator"] =
            serde_json::json!({"kind":"exact_match", "name":"em", "version":"1"});
        let mut explicit_null = record(2);
        explicit_null.value["evaluator"] = absent.value["evaluator"].clone();
        explicit_null.value["evaluator"]["prompt_version"] = serde_json::Value::Null;
        assert!(filter.validate_record(absent).is_ok());
        assert!(filter.validate_record(explicit_null).is_ok());
    }
}
