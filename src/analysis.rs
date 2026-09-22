use crate::input::ParseResult;

pub struct PercentileStats {
    pub p50: Option<u32>,
    pub p95: Option<u32>,
    pub p99: Option<u32>,
}

pub struct LatencyStats {
    pub min: Option<u32>,
    pub max: Option<u32>,
    pub avg: Option<f64>,
    pub percentiles: PercentileStats,
}

pub struct TokenStats {
    pub input_total: u64,
    pub output_total: u64,
}

pub struct CostStats {
    pub known_cost: f64,
    pub unknown_pricing_requests: u64,
}

pub struct RequestStats {
    pub successful: u64,
    pub failed: u64,
    pub total: u64,
}

pub struct ParseStats {
    pub invalid_records: u64,
}

pub struct AnalysisResult {
    pub latency_stats: LatencyStats,
    pub token_stats: TokenStats,
    pub cost_stats: CostStats,
    pub request_stats: RequestStats,
    pub parse_stats: ParseStats,
}

/// Analysis is intentionally deferred until validated `EvalRecordV2` values
/// are wired into the pipeline. The generic second argument preserves the
/// current caller shape without coupling this module to the removed pricing
/// model.
pub fn analyze<T>(requests: &ParseResult, _context: &T) -> AnalysisResult {
    AnalysisResult {
        latency_stats: LatencyStats {
            min: None,
            max: None,
            avg: None,
            percentiles: PercentileStats {
                p50: None,
                p95: None,
                p99: None,
            },
        },
        token_stats: TokenStats {
            input_total: 0,
            output_total: 0,
        },
        cost_stats: CostStats {
            known_cost: 0.0,
            unknown_pricing_requests: 0,
        },
        request_stats: RequestStats {
            successful: 0,
            failed: 0,
            total: 0,
        },
        parse_stats: ParseStats {
            invalid_records: requests.errors.len() as u64,
        },
    }
}
