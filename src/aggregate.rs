use crate::identity::EffectiveMeasurement;
use crate::model::{Status, Unit};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupKey {
    pub run_id: String,
    pub metric: String,
    pub unit: Unit,
    pub config_hash: String,
    pub dataset_fingerprint: String,
    pub evaluator_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupStats {
    pub key: GroupKey,
    pub config: serde_json::Map<String, serde_json::Value>,
    pub n_attempted: usize,
    pub n_ok: usize,
    pub median: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub warning: bool,
}

#[derive(Default)]
struct GroupAccumulator {
    config: serde_json::Map<String, serde_json::Value>,
    n_attempted: usize,
    n_ok: usize,
    ok_values: Vec<f64>,
}

pub fn aggregate_records<'a>(
    records: impl IntoIterator<Item = &'a EffectiveMeasurement>,
) -> Vec<GroupStats> {
    let mut groups: BTreeMap<GroupKey, GroupAccumulator> = BTreeMap::new();

    for measurement in records {
        if measurement.record.config.contains_key("aggregate") {
            continue;
        }

        let key = GroupKey {
            run_id: measurement.record.run_id.clone(),
            metric: measurement.record.metric.clone(),
            unit: measurement.record.unit,
            config_hash: measurement.config_hash.clone(),
            dataset_fingerprint: measurement.dataset_fingerprint.clone(),
            evaluator_fingerprint: measurement.evaluator_fingerprint.clone(),
        };

        let entry = groups.entry(key).or_default();
        if entry.n_attempted == 0 {
            entry.config = measurement.record.config.clone();
        }
        entry.n_attempted += 1;
        if matches!(measurement.record.status, Status::Ok) {
            entry.n_ok += 1;
            if let Some(val) = measurement.record.value.as_f64() {
                entry.ok_values.push(val);
            }
        }
    }

    groups
        .into_iter()
        .map(|(key, mut acc)| {
            let n_attempted = acc.n_attempted;
            let n_ok = acc.n_ok;
            let warning = n_ok < 3;

            let (median, min, max) = if n_ok == 0 {
                (None, None, None)
            } else {
                acc.ok_values.sort_by(|a, b| a.total_cmp(b));
                let min = Some(acc.ok_values[0]);
                let max = Some(acc.ok_values[acc.ok_values.len() - 1]);
                let len = acc.ok_values.len();
                let median = if len % 2 == 1 {
                    Some(acc.ok_values[len / 2])
                } else {
                    Some((acc.ok_values[len / 2 - 1] + acc.ok_values[len / 2]) / 2.0)
                };
                (median, min, max)
            };

            GroupStats {
                key,
                config: acc.config,
                n_attempted,
                n_ok,
                median,
                min,
                max,
                warning,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_line;
    use crate::validator::validate_record;

    fn make_measurement(
        line: usize,
        status: &str,
        value: serde_json::Value,
        config: serde_json::Value,
    ) -> EffectiveMeasurement {
        let mut raw = parse_line(
            include_str!("../data/v2/valid.jsonl")
                .lines()
                .next()
                .unwrap(),
            line,
        )
        .unwrap();
        raw.value["record_id"] = serde_json::json!(format!("rec-{line}"));
        raw.value["status"] = serde_json::json!(status);
        raw.value["value"] = value;
        raw.value["config"] = config;

        let validated = validate_record(raw).unwrap();
        EffectiveMeasurement {
            first_line: line,
            record: validated,
            config_hash: "hash1".to_string(),
            dataset_fingerprint: "null".to_string(),
            evaluator_fingerprint: "null".to_string(),
        }
    }

    #[test]
    fn all_failed_group_has_zero_ok_and_null_statistics() {
        let m1 = make_measurement(1, "oom", serde_json::Value::Null, serde_json::json!({"batch_size": 1}));
        let m2 = make_measurement(2, "failed", serde_json::Value::Null, serde_json::json!({"batch_size": 1}));
        let m3 = make_measurement(3, "timeout", serde_json::Value::Null, serde_json::json!({"batch_size": 1}));

        let stats = aggregate_records(&[m1, m2, m3]);
        assert_eq!(stats.len(), 1);
        let group = &stats[0];
        assert_eq!(group.n_attempted, 3);
        assert_eq!(group.n_ok, 0);
        assert_eq!(group.median, None);
        assert_eq!(group.min, None);
        assert_eq!(group.max, None);
        assert!(group.warning);
    }

    #[test]
    fn even_count_median_is_mean_of_two_middle_values_and_triggers_warning_when_under_three_ok() {
        let m1 = make_measurement(1, "ok", serde_json::json!(200.0), serde_json::json!({"batch_size": 8}));
        let m2 = make_measurement(2, "ok", serde_json::json!(210.0), serde_json::json!({"batch_size": 8}));
        let m3 = make_measurement(3, "oom", serde_json::Value::Null, serde_json::json!({"batch_size": 8}));

        let stats = aggregate_records(&[m1, m2, m3]);
        assert_eq!(stats.len(), 1);
        let group = &stats[0];
        assert_eq!(group.n_attempted, 3);
        assert_eq!(group.n_ok, 2);
        assert_eq!(group.median, Some(205.0));
        assert_eq!(group.min, Some(200.0));
        assert_eq!(group.max, Some(210.0));
        assert!(group.warning);
    }

    #[test]
    fn odd_count_median_is_middle_value_and_three_ok_does_not_warn() {
        let m1 = make_measurement(1, "ok", serde_json::json!(100.0), serde_json::json!({"batch_size": 1}));
        let m2 = make_measurement(2, "ok", serde_json::json!(105.0), serde_json::json!({"batch_size": 1}));
        let m3 = make_measurement(3, "ok", serde_json::json!(102.0), serde_json::json!({"batch_size": 1}));

        let stats = aggregate_records(&[m1, m2, m3]);
        assert_eq!(stats.len(), 1);
        let group = &stats[0];
        assert_eq!(group.n_attempted, 3);
        assert_eq!(group.n_ok, 3);
        assert_eq!(group.median, Some(102.0));
        assert_eq!(group.min, Some(100.0));
        assert_eq!(group.max, Some(105.0));
        assert!(!group.warning);
    }

    #[test]
    fn records_with_aggregate_key_in_config_are_excluded() {
        let m1 = make_measurement(1, "ok", serde_json::json!(100.0), serde_json::json!({"batch_size": 1}));
        let m2 = make_measurement(2, "ok", serde_json::json!(102.0), serde_json::json!({"batch_size": 1}));
        let m3 = make_measurement(3, "ok", serde_json::json!(105.0), serde_json::json!({"batch_size": 1}));
        let agg = make_measurement(
            4,
            "ok",
            serde_json::json!(102.0),
            serde_json::json!({"batch_size": 1, "aggregate": "median_of_3"}),
        );

        let stats = aggregate_records(&[m1, m2, m3, agg]);
        assert_eq!(stats.len(), 1);
        let group = &stats[0];
        assert_eq!(group.n_attempted, 3);
        assert_eq!(group.n_ok, 3);
        assert_eq!(group.median, Some(102.0));
    }

    #[test]
    fn groups_are_split_by_config_hash_and_sorted_in_stable_order() {
        let mut m1 = make_measurement(1, "ok", serde_json::json!(10.0), serde_json::json!({"batch_size": 1}));
        m1.config_hash = "hash-b".to_string();
        let mut m2 = make_measurement(2, "ok", serde_json::json!(20.0), serde_json::json!({"batch_size": 2}));
        m2.config_hash = "hash-a".to_string();

        let stats = aggregate_records(&[m1, m2]);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].key.config_hash, "hash-a");
        assert_eq!(stats[1].key.config_hash, "hash-b");
    }
}
