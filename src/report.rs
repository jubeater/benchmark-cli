use crate::aggregate::GroupStats;
use serde::Serialize;
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize)]
pub struct Rejection {
    pub line: usize,
    pub field: String,
    pub reason: String,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}: {}", self.line, self.field, self.reason)
    }
}

#[derive(Serialize)]
struct JsonOutput {
    output_version: u32,
    accepted: usize,
    rejected: usize,
    superseded: usize,
    complete: bool,
    rejections: Vec<Rejection>,
    groups: Vec<JsonGroup>,
}

#[derive(Serialize)]
struct JsonGroup {
    run_id: String,
    metric: String,
    unit: String,
    config_hash: String,
    config: serde_json::Map<String, serde_json::Value>,
    dataset_fingerprint: String,
    evaluator_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_by: Option<serde_json::Map<String, serde_json::Value>>,
    n_attempted: usize,
    n_ok: usize,
    median: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    warning: bool,
}

pub fn render_json(
    accepted: usize,
    rejected: usize,
    superseded: usize,
    groups: &[GroupStats],
    group_by: &[String],
    rejections: &[Rejection],
) -> String {
    let payload = JsonOutput {
        output_version: OUTPUT_VERSION,
        accepted,
        rejected,
        superseded,
        complete: rejected == 0,
        rejections: rejections.to_vec(),
        groups: groups
            .iter()
            .map(|g| JsonGroup {
                run_id: g.key.run_id.clone(),
                metric: g.key.metric.clone(),
                unit: g.key.unit.to_string(),
                config_hash: g.key.config_hash.clone(),
                config: g.config.clone(),
                dataset_fingerprint: g.key.dataset_fingerprint.clone(),
                evaluator_fingerprint: g.key.evaluator_fingerprint.clone(),
                group_by: (!group_by.is_empty()).then(|| {
                    group_by
                        .iter()
                        .filter_map(|key| g.config.get(key).cloned().map(|v| (key.clone(), v)))
                        .collect()
                }),
                n_attempted: g.n_attempted,
                n_ok: g.n_ok,
                median: g.median,
                min: g.min,
                max: g.max,
                warning: g.warning,
            })
            .collect(),
    };
    let mut out = serde_json::to_string_pretty(&payload).expect("payload is serializable");
    out.push('\n');
    out
}

const OUTPUT_VERSION: u32 = 2;

pub fn render(
    accepted: usize,
    rejected: usize,
    superseded: usize,
    groups: &[GroupStats],
    group_by: &[String],
) -> String {
    let mut out = format!(
        "Eval-schema v2 validation\nAccepted records: {accepted}\nRejected records: {rejected}\nSuperseded records: {superseded}\n"
    );
    if rejected > 0 {
        out.push_str(&format!(
            "\nIncomplete result: statistics exclude rejected records (count: {rejected}).\n"
        ));
    }
    if !groups.is_empty() {
        out.push('\n');
        out.push_str(&render_table(groups, group_by));
    }
    out
}

fn format_stat(val: Option<f64>) -> String {
    val.map(|v| v.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn format_config_value(value: Option<&serde_json::Value>) -> String {
    match value {
        None => "absent".to_string(),
        Some(serde_json::Value::Null) => "null".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn short_hash(hash: &str) -> &str {
    // Fingerprints are ASCII SHA-256 hex strings or the sentinel "null".
    &hash[..hash.len().min(12)]
}

fn config_value_rank(value: Option<&serde_json::Value>) -> u8 {
    use serde_json::Value;
    match value {
        Some(Value::Number(_)) => 0,
        Some(Value::String(_)) => 1,
        Some(Value::Bool(_)) => 2,
        Some(Value::Array(_)) => 3,
        Some(Value::Object(_)) => 4,
        Some(Value::Null) => 5,
        None => 6,
    }
}

fn compare_numbers(a: &serde_json::Number, b: &serde_json::Number) -> Ordering {
    let integer = |n: &serde_json::Number| {
        n.as_i64()
            .map(i128::from)
            .or_else(|| n.as_u64().map(i128::from))
    };
    let compare_integer_float = |integer: i128, float: f64| {
        // Truncate the float, not the integer: casting integers to f64 can
        // collapse distinct 64-bit values. i128 covers every JSON integer.
        integer.cmp(&(float as i128)).then_with(|| {
            0.0_f64
                .partial_cmp(&float.fract())
                .expect("JSON numbers are finite")
        })
    };
    match (integer(a), integer(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(a), None) => compare_integer_float(a, b.as_f64().unwrap()),
        (None, Some(b)) => compare_integer_float(b, a.as_f64().unwrap()).reverse(),
        (None, None) => a
            .as_f64()
            .unwrap()
            .partial_cmp(&b.as_f64().unwrap())
            .expect("JSON numbers are finite"),
    }
}

fn compare_config_values(a: Option<&serde_json::Value>, b: Option<&serde_json::Value>) -> Ordering {
    use serde_json::Value;
    match (a, b) {
        (Some(Value::Number(a)), Some(Value::Number(b))) => compare_numbers(a, b),
        (Some(Value::String(a)), Some(Value::String(b))) => a.cmp(b),
        _ => config_value_rank(a)
            .cmp(&config_value_rank(b))
            .then_with(|| format_config_value(a).cmp(&format_config_value(b))),
    }
}

pub fn render_table(groups: &[GroupStats], group_by: &[String]) -> String {
    if groups.is_empty() {
        return String::new();
    }

    let mut ordered_groups: Vec<_> = groups.iter().collect();
    if !group_by.is_empty() {
        ordered_groups.sort_by(|a, b| {
            group_by
                .iter()
                .map(|key| compare_config_values(a.config.get(key), b.config.get(key)))
                .find(|order| *order != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.key.cmp(&b.key))
        });
    }

    let mut headers: Vec<String> = vec![
        "run_id".to_string(),
        "metric".to_string(),
        "unit".to_string(),
        "config_hash".to_string(),
        "dataset_fingerprint".to_string(),
        "evaluator_fingerprint".to_string(),
    ];
    for key in group_by {
        headers.push(key.clone());
    }
    headers.extend([
        "n_attempted".to_string(),
        "n_ok".to_string(),
        "median".to_string(),
        "min".to_string(),
        "max".to_string(),
    ]);

    let mut rows: Vec<Vec<String>> = Vec::new();
    for g in &ordered_groups {
        let mut row = vec![
            g.key.run_id.clone(),
            g.key.metric.clone(),
            g.key.unit.to_string(),
            short_hash(&g.key.config_hash).to_string(),
            short_hash(&g.key.dataset_fingerprint).to_string(),
            short_hash(&g.key.evaluator_fingerprint).to_string(),
        ];
        for key in group_by {
            row.push(format_config_value(g.config.get(key)));
        }
        row.extend([
            g.n_attempted.to_string(),
            g.n_ok.to_string(),
            format_stat(g.median),
            format_stat(g.min),
            format_stat(g.max),
        ]);
        rows.push(row);
    }

    let num_cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    let mut out = String::new();

    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<w$}", h, w = widths[i]))
        .collect::<Vec<_>>()
        .join(" | ");
    out.push_str(&header_line);
    out.push('\n');

    let divider_line = widths
        .iter()
        .map(|w| "-".repeat(*w))
        .collect::<Vec<_>>()
        .join("-+-");
    out.push_str(&divider_line);
    out.push('\n');

    let numeric_col_start = num_cols - 5;
    for row in &rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if i >= numeric_col_start {
                    format!("{:>w$}", cell, w = widths[i])
                } else {
                    format!("{:<w$}", cell, w = widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join(" | ");
        out.push_str(&line);
        out.push('\n');
    }

    let mut warnings = Vec::new();
    for g in &ordered_groups {
        if g.warning {
            warnings.push(format!(
                "warning: group (run_id: {}, metric: {}, config_hash: {}) has fewer than 3 ok seeds (n_ok = {})",
                g.key.run_id, g.key.metric, short_hash(&g.key.config_hash), g.n_ok
            ));
        }
    }
    if !warnings.is_empty() {
        out.push('\n');
        for w in warnings {
            out.push_str(&w);
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::GroupKey;
    use crate::model::Unit;

    #[test]
    fn render_table_displays_columns_nulls_and_warnings() {
        let group_ok = GroupStats {
            key: GroupKey {
                run_id: "run-1".to_string(),
                metric: "accuracy".to_string(),
                unit: Unit::Score,
                config_hash: "hash-ok".to_string(),
                dataset_fingerprint: "null".to_string(),
                evaluator_fingerprint: "null".to_string(),
            },
            config: serde_json::Map::new(),
            n_attempted: 3,
            n_ok: 3,
            median: Some(102.0),
            min: Some(100.0),
            max: Some(105.0),
            warning: false,
        };

        let group_warn = GroupStats {
            key: GroupKey {
                run_id: "run-1".to_string(),
                metric: "accuracy".to_string(),
                unit: Unit::Score,
                config_hash: "hash-warn".to_string(),
                dataset_fingerprint: "null".to_string(),
                evaluator_fingerprint: "null".to_string(),
            },
            config: serde_json::Map::new(),
            n_attempted: 3,
            n_ok: 2,
            median: Some(205.0),
            min: Some(200.0),
            max: Some(210.0),
            warning: true,
        };

        let group_null = GroupStats {
            key: GroupKey {
                run_id: "run-1".to_string(),
                metric: "accuracy".to_string(),
                unit: Unit::Score,
                config_hash: "hash-failed".to_string(),
                dataset_fingerprint: "null".to_string(),
                evaluator_fingerprint: "null".to_string(),
            },
            config: serde_json::Map::new(),
            n_attempted: 3,
            n_ok: 0,
            median: None,
            min: None,
            max: None,
            warning: true,
        };

        let rendered = render_table(&[group_ok, group_warn, group_null], &[]);
        assert!(rendered.contains("run_id"));
        assert!(rendered.contains("n_attempted"));
        assert!(rendered.contains("median"));
        assert!(rendered.contains("102"));
        assert!(rendered.contains("205"));
        assert!(rendered.contains("N/A"));
        assert!(rendered.contains("fewer than 3 ok seeds"));
        assert!(rendered.contains("warning: group (run_id: run-1, metric: accuracy, config_hash: hash-warn) has fewer than 3 ok seeds (n_ok = 2)"));
    }

    #[test]
    fn render_table_with_group_by_displays_columns_in_order() {
        let mut config = serde_json::Map::new();
        config.insert("batch_size".to_string(), serde_json::json!(8));
        let group = GroupStats {
            key: GroupKey {
                run_id: "run-1".to_string(),
                metric: "tokens_per_sec".to_string(),
                unit: Unit::TokensPerSec,
                config_hash: "hash-8".to_string(),
                dataset_fingerprint: "null".to_string(),
                evaluator_fingerprint: "null".to_string(),
            },
            config,
            n_attempted: 3,
            n_ok: 3,
            median: Some(205.0),
            min: Some(200.0),
            max: Some(210.0),
            warning: false,
        };

        let rendered = render_table(&[group], &["batch_size".to_string()]);
        assert!(rendered.contains("batch_size"));
        let header_line = rendered.lines().next().unwrap();
        assert!(header_line.contains("evaluator_fingerprint | batch_size | n_attempted"));
    }

    #[test]
    fn render_table_multiple_keys_renders_absent_distinct_from_null() {
        let mut config = serde_json::Map::new();
        config.insert("model".to_string(), serde_json::json!("gpt-4"));
        config.insert("temperature".to_string(), serde_json::Value::Null);
        // Note: "missing_key" is omitted from config

        let group = GroupStats {
            key: GroupKey {
                run_id: "run-1".to_string(),
                metric: "score".to_string(),
                unit: Unit::Score,
                config_hash: "hash-model".to_string(),
                dataset_fingerprint: "null".to_string(),
                evaluator_fingerprint: "null".to_string(),
            },
            config,
            n_attempted: 3,
            n_ok: 3,
            median: Some(0.95),
            min: Some(0.90),
            max: Some(0.99),
            warning: false,
        };

        let keys = vec![
            "model".to_string(),
            "temperature".to_string(),
            "missing_key".to_string(),
        ];
        let rendered = render_table(&[group], &keys);
        let header_line = rendered.lines().next().unwrap();
        assert!(header_line.contains("evaluator_fingerprint | model | temperature | missing_key | n_attempted"));

        let row_line = rendered.lines().nth(2).unwrap();
        assert!(row_line.contains("gpt-4 | null        | absent      |"));
    }
}
