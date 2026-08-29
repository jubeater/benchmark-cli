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
    Some(values[index])
}
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::input::parse_pricing_model;

    #[test]
    fn percentile_basic() {
        let values = vec![1, 2, 3, 4, 100];

        assert_eq!(percentile(&values, 0.5), Some(3));
        assert_eq!(percentile(&values, 0.95), Some(100));
        assert_eq!(percentile(&values, 0.99), Some(100));
        assert_eq!(percentile(&values, 0.2), Some(1));
    }

    #[test]
    fn percentile_invalid_input() {
        let values = vec![1, 2, 3, 4, 100];
        assert_eq!(percentile(&values, 0.0), None);
        assert_eq!(percentile(&values, -1.0), None);
        assert_eq!(percentile(&values, 1.5), None);
        let values: Vec<u32> = vec![];
        assert_eq!(percentile(&values, 0.5), None);
    }

    #[test]
    fn latency_accumulator_basic() {
        let values: [u32; 6] = [10, 5, 3, 100, 44, 99];
        let mut accumulator = LatencyAccumulator::new();
        for value in values {
            accumulator.update(value);
        }
        let result = accumulator.finish();
        assert_eq!(result.min, Some(3));
        assert_eq!(result.max, Some(100));
        assert_eq!(result.percentiles.p50, Some(10));
        assert_eq!(result.percentiles.p95, Some(100));
        assert_eq!(result.percentiles.p99, Some(100));
        let expected = 43.5;
        let actual = result.avg.unwrap();
        assert!((actual - expected).abs() < 1e-10);
    }

    #[test]
    fn latency_accumulator_empty() {
        let accumulator = LatencyAccumulator::new();
        let result = accumulator.finish();

        assert_eq!(result.min, None);
        assert_eq!(result.max, None);
        assert_eq!(result.avg, None);
        assert_eq!(result.percentiles.p50, None);
        assert_eq!(result.percentiles.p95, None);
        assert_eq!(result.percentiles.p99, None);
    }

    #[test]
    fn token_accumulator_basic() {
        let mut accumulator = TokenAccumulator::new();

        accumulator.update(100, 50);
        accumulator.update(200, 75);

        let result = accumulator.finish();

        assert_eq!(result.input_total, 300);
        assert_eq!(result.output_total, 125);
    }

    #[test]
    fn token_accumulator_empty() {
        let accumulator = TokenAccumulator::new();
        let result = accumulator.finish();

        assert_eq!(result.input_total, 0);
        assert_eq!(result.output_total, 0);
    }

    #[test]
    fn request_stats_accumulator_basic() {
        let mut accumulator = RequestStatsAccumulator::new();

        accumulator.update(&Status::Ok);
        accumulator.update(&Status::Ok);
        accumulator.update(&Status::Cancelled);
        accumulator.update(&Status::RateLimited);
        accumulator.update(&Status::Ok);
        accumulator.update(&Status::Timeout);

        let result = accumulator.finish();

        assert_eq!(result.successful, 3);
        assert_eq!(result.failed, 3);
        assert_eq!(result.total, 6);
    }

    #[test]
    fn request_stats_accumulator_empty() {
        let accumulator = RequestStatsAccumulator::new();
        let result = accumulator.finish();

        assert_eq!(result.successful, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(result.total, 0);
    }

    #[test]
    fn cost_accumulator() {
        let pricing_table = parse_pricing_model("./data/test/valid_pricing.json").unwrap();

        let mut accumulator = CostAccumulator::new(&pricing_table);

        accumulator.update("gpt-5", 2_000_000, 3_000_000);
        accumulator.update("claude-sonnet-5", 3_000_000, 1_000_000);
        accumulator.update("unknown-model-a", 2_000_000, 1_000_000);
        accumulator.update("unknown-model-b", 2_000_000, 1_000_000);

        let result = accumulator.finish();

        let expected = 56.5;
        let actual = result.known_cost;
        assert!((actual - expected).abs() < 1e-10);
        assert_eq!(result.unknown_pricing_requests, 2);
    }
    #[test]
    fn cost_accumulator_empty() {
        let pricing_table = parse_pricing_model("./data/test/valid_pricing.json").unwrap();

        let accumulator = CostAccumulator::new(&pricing_table);
        let result = accumulator.finish();

        assert_eq!(result.known_cost, 0.0);
        assert_eq!(result.unknown_pricing_requests, 0);
    }
}
