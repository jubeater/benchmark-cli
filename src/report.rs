use crate::aggregate::GroupStats;

pub fn render(
    accepted: usize,
    rejected: usize,
    superseded: usize,
    groups: &[GroupStats],
) -> String {
    let mut out = format!(
        "Eval-schema v2 validation\nAccepted records: {accepted}\nRejected records: {rejected}\nSuperseded records: {superseded}\n"
    );
    if !groups.is_empty() {
        out.push('\n');
        out.push_str(&render_table(groups));
    }
    out
}

const NUMERIC_COL_START: usize = 6;
const NUMERIC_COL_END: usize = 10;

fn format_stat(val: Option<f64>) -> String {
    val.map(|v| v.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

pub fn render_table(groups: &[GroupStats]) -> String {
    if groups.is_empty() {
        return String::new();
    }

    let headers = [
        "run_id",
        "metric",
        "unit",
        "config_hash",
        "dataset_fingerprint",
        "evaluator_fingerprint",
        "n_attempted",
        "n_ok",
        "median",
        "min",
        "max",
    ];

    let mut rows: Vec<[String; 11]> = Vec::new();
    for g in groups {
        rows.push([
            g.key.run_id.clone(),
            g.key.metric.clone(),
            g.key.unit.to_string(),
            g.key.config_hash.clone(),
            g.key.dataset_fingerprint.clone(),
            g.key.evaluator_fingerprint.clone(),
            g.n_attempted.to_string(),
            g.n_ok.to_string(),
            format_stat(g.median),
            format_stat(g.min),
            format_stat(g.max),
        ]);
    }

    let mut widths: [usize; 11] = std::array::from_fn(|i| headers[i].len());
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

    for row in &rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if (NUMERIC_COL_START..=NUMERIC_COL_END).contains(&i) {
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
    for g in groups {
        if g.warning {
            warnings.push(format!(
                "warning: group (run_id: {}, metric: {}, config_hash: {}) has fewer than 3 ok seeds (n_ok = {})",
                g.key.run_id, g.key.metric, g.key.config_hash, g.n_ok
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
            n_attempted: 3,
            n_ok: 0,
            median: None,
            min: None,
            max: None,
            warning: true,
        };

        let rendered = render_table(&[group_ok, group_warn, group_null]);
        assert!(rendered.contains("run_id"));
        assert!(rendered.contains("n_attempted"));
        assert!(rendered.contains("median"));
        assert!(rendered.contains("102"));
        assert!(rendered.contains("205"));
        assert!(rendered.contains("N/A"));
        assert!(rendered.contains("fewer than 3 ok seeds"));
        assert!(rendered.contains("warning: group (run_id: run-1, metric: accuracy, config_hash: hash-warn) has fewer than 3 ok seeds (n_ok = 2)"));
    }
}

