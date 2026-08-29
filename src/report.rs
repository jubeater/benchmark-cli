use crate::analysis::AnalysisResult;

pub fn render(result: &AnalysisResult) -> String {
    let request_stats = &result.request_stats;
    let parse_stats = &result.parse_stats;
    let total = request_stats.total;
    let mut err_rate: Option<f64> = None;
    if total > 0 {
        err_rate = Some((request_stats.failed as f64 / request_stats.total as f64) * 100.0);
    }

    let mut log_analysis = vec!["AI Log Analysis".to_string()];
    log_analysis.push(format!("--------------------------"));
    log_analysis.push(format!(""));
    log_analysis.push(format!("Requests"));
    log_analysis.push(format!("  Total:            {}", total));
    log_analysis.push(format!("  Successful:       {}", request_stats.successful));
    log_analysis.push(format!("  Failed:           {}", request_stats.failed));
    log_analysis.push(format!(
        "  Error rate:           {}%",
        err_rate
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    ));
    log_analysis.push(format!("--------------------------"));
    log_analysis.push(format!("Data Quality"));
    log_analysis.push(format!(
        "  Invalid records:      {}\n",
        parse_stats.invalid_records
    ));

    return log_analysis.join("\n");
}
