use crate::input::ParseResult;
use crate::model::{PricingTable, Status};

// assume input slice sorted already
fn percentile(values: &[u32], percentile: f64) -> Option<u32> {
    if percentile <= 0.0 || percentile > 1.0 {
        return None;
    }
    let len = values.len();
    if len == 0 {
        return None;
    }
    let one_based_rank = (len as f64 * percentile).ceil();
    let index = (one_based_rank - 1_f64) as usize;
    return Some(values[index]);
}
struct PercentileStats {
    p50: Option<u32>,
    p95: Option<u32>,
    p99: Option<u32>,
}
struct LatencyStats {
    min: Option<u32>,
    max: Option<u32>,
    avg: Option<f64>,
    percentiles: PercentileStats,
}

struct TokenStats {
    input_total: u64,
    output_total: u64,
}

struct CostStats {
    known_cost: f64,
    unknown_pricing_requests: u64,
}

struct RequestStats {
    successful: u64,
    failed: u64,
    total: u64,
}

struct ParseStats {
    invalid_records: u64,
}

pub struct AnalysisResult {
    latency_stats: LatencyStats,
    token_stats: TokenStats,
    cost_stats: CostStats,
    request_stats: RequestStats,
    parse_stats: ParseStats,
}

struct LatencyAccumulator {
    min: Option<u32>,
    max: Option<u32>,
    sum: u64,
    count: u64,
    values: Vec<u32>,
}

impl LatencyAccumulator {
    fn new() -> Self {
        Self {
            min: None,
            max: None,
            sum: 0,
            count: 0,
            values: Vec::new(),
        }
    }
    fn update(&mut self, latency: u32) {
        match self.min {
            Some(x) => {
                if x > latency {
                    self.min = Some(latency)
                }
            }
            None => {
                self.min = Some(latency);
            }
        }
        match self.max {
            Some(x) => {
                if x < latency {
                    self.max = Some(latency);
                }
            }
            None => {
                self.max = Some(latency);
            }
        }
        self.sum += latency as u64;
        self.count += 1;
        self.values.push(latency);
    }
    fn finish(mut self) -> LatencyStats {
        let avg = if self.count != 0 {
            Some(self.sum as f64 / self.count as f64)
        } else {
            None
        };
        self.values.sort();
        let percentile_stats = PercentileStats {
            p50: percentile(&self.values, 0.5),
            p95: percentile(&self.values, 0.95),
            p99: percentile(&self.values, 0.99),
        };
        LatencyStats {
            min: self.min,
            max: self.max,
            avg,
            percentiles: percentile_stats,
        }
    }
}

struct TokenAccumulator {
    input_total: u64,
    output_total: u64,
}

impl TokenAccumulator {
    fn new() -> Self {
        Self {
            input_total: 0,
            output_total: 0,
        }
    }
    fn update(&mut self, input_token: u64, output_token: u64) {
        self.input_total += input_token;
        self.output_total += output_token;
    }

    fn finish(self) -> TokenStats {
        TokenStats {
            input_total: self.input_total,
            output_total: self.output_total,
        }
    }
}

struct CostAccumulator<'a> {
    pricing: &'a PricingTable,
    known_cost: f64,
    unknown_pricing_requests: u64,
}

impl<'a> CostAccumulator<'a> {
    fn new(pricing: &'a PricingTable) -> Self {
        Self {
            pricing,
            known_cost: 0.0,
            unknown_pricing_requests: 0,
        }
    }

    fn update(&mut self, model_name: &str, input_tokens: u64, output_tokens: u64) {
        let model_price = self.pricing.get(model_name);
        match model_price {
            Some(price) => {
                let cur_cost = (input_tokens as f64 / 1_000_000_f64) * price.input_per_million()
                    + (output_tokens as f64 / 1_000_000_f64) * price.output_per_million();

                self.known_cost += cur_cost;
            }
            None => {
                self.unknown_pricing_requests += 1;
            }
        }
    }

    fn finish(self) -> CostStats {
        CostStats {
            known_cost: self.known_cost,
            unknown_pricing_requests: self.unknown_pricing_requests,
        }
    }
}

struct RequestStatsAccumulator {
    successful: u64,
    failed: u64,
}

impl RequestStatsAccumulator {
    fn new() -> Self {
        Self {
            successful: 0,
            failed: 0,
        }
    }

    fn update(&mut self, status: &Status) {
        match status {
            Status::Ok => {
                self.successful += 1;
            }
            _ => {
                self.failed += 1;
            }
        }
    }

    fn finish(self) -> RequestStats {
        RequestStats {
            successful: self.successful,
            failed: self.failed,
            total: self.successful + self.failed,
        }
    }
}

pub fn analyze(requests: &ParseResult, price_table: &PricingTable) -> AnalysisResult {
    let mut latency_accumulator = LatencyAccumulator::new();
    let mut token_accumulator = TokenAccumulator::new();
    let mut request_stats_accumulator = RequestStatsAccumulator::new();
    let mut cost_accumulator = CostAccumulator::new(price_table);
    for request in &requests.requests {
        latency_accumulator.update(request.latency_ms());
        token_accumulator.update(request.input_tokens(), request.output_tokens());
        cost_accumulator.update(
            request.model_name(),
            request.input_tokens(),
            request.output_tokens(),
        );
        request_stats_accumulator.update(request.status());
    }

    let latency_stats = latency_accumulator.finish();
    let token_stats = token_accumulator.finish();
    let cost_stats = cost_accumulator.finish();
    let request_stats = request_stats_accumulator.finish();
    AnalysisResult {
        latency_stats,
        token_stats,
        cost_stats,
        request_stats,
        parse_stats: ParseStats {
            invalid_records: requests.errors.len() as u64,
        },
    }
}
