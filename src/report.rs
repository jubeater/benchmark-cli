use crate::analysis::AnalysisResult;


fn format_optional_u32(value: Option<u32>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "N/A".to_string(),
    }
}

fn format_optional_f64(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{:.2}", v),
        None => "N/A".to_string(),
    }
}

fn format_tokens(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.2}M", value as f64 / 1_000_000.0)
    } else {
        value.to_string()
    }
}

pub fn render(result: &AnalysisResult) -> String {
    let request_stats = &result.request_stats;
    let parse_stats = &result.parse_stats;
    let total = request_stats.total;
    let err_rate = if total > 0 {
        Some(request_stats.failed as f64 / total as f64 * 100.0)
    } else {
        None
    };
    let total_tokens = result.token_stats.input_total + result.token_stats.output_total;

    let mut log_analysis = vec!["AI Log Analysis".to_string()];
    log_analysis.push(format!("--------------------------"));
    log_analysis.push(format!(""));
    log_analysis.push(format!("Requests"));
    log_analysis.push(format!("  Total:                 {}", total));
    log_analysis.push(format!("  Successful:            {}", request_stats.successful));
    log_analysis.push(format!("  Failed:                {}", request_stats.failed));
    log_analysis.push(format!("  Error rate:            {}%", format_optional_f64(err_rate)));
    log_analysis.push(format!("------------------------------"));
    log_analysis.push(format!("Data Quality"));
    log_analysis.push(format!("  Invalid records:       {}", parse_stats.invalid_records));
    log_analysis.push(format!("------------------------------"));
    log_analysis.push(format!(""));
    log_analysis.push(format!("Latency"));
    log_analysis.push(format!("  Min:                   {} ms", format_optional_u32(result.latency_stats.min)));
    log_analysis.push(format!("  Avg:                   {} ms", format_optional_f64(result.latency_stats.avg)));
    log_analysis.push(format!("  P50:                   {} ms", format_optional_u32(result.latency_stats.percentiles.p50)));
    log_analysis.push(format!("  P95:                   {} ms", format_optional_u32(result.latency_stats.percentiles.p95)));
    log_analysis.push(format!("  P99:                   {} ms", format_optional_u32(result.latency_stats.percentiles.p99)));
    log_analysis.push(format!("  Max:                   {} ms", format_optional_u32(result.latency_stats.max)));
    log_analysis.push(format!("------------------------------"));
    log_analysis.push(format!(""));
    log_analysis.push(format!("Token Usage"));
    log_analysis.push(format!("  Input:                 {}", format_tokens(result.token_stats.input_total)));
    log_analysis.push(format!("  Output:                {}", format_tokens(result.token_stats.output_total)));
    log_analysis.push(format!("  Total:                 {}", format_tokens(total_tokens)));
    log_analysis.push(format!("------------------------------"));
    log_analysis.push(format!(""));
    log_analysis.push(format!("Cost"));
    log_analysis.push(format!("  Known-priced cost:    ${:.2}", result.cost_stats.known_cost));
    log_analysis.push(format!("  Unpriced requests:     {}", result.cost_stats.unknown_pricing_requests));
    if result.cost_stats.unknown_pricing_requests > 0 {
        log_analysis.push(format!("⚠ Cost is incomplete"));
    }
    log_analysis.push("\n".to_string());

    return log_analysis.join("\n");
}

