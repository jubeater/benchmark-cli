use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Debug, Deserialize)]
pub enum SchemaVersion {
    #[serde(rename = "2.0")]
    V2,
}

#[derive(Debug, Deserialize)]
pub enum Status {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "oom")]
    Oom,
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Debug, Deserialize)]
pub struct EvalRecordV2 {
    pub schema_version: SchemaVersion,
    pub record_id: String,
    pub run_id: String,
    pub project: String,
    pub timestamp: String,
    pub git_commit: String,
    pub git_dirty: bool,
    pub config: Map<String, Value>,

    pub seed: Option<u64>,

    pub metric: String,
    pub value: Value,
    pub status: Status,
    pub notes: String,

    pub dataset: Option<Dataset>,
    pub model: Option<Model>,
    pub evaluator: Option<Evaluator>,
    pub artifacts: Option<Artifacts>,
    pub cost: Option<Cost>,
    pub env: Option<Env>,
    pub comparison: Option<Comparison>,
}

#[derive(Debug, Deserialize)]
pub struct Dataset {
    pub name: String,
    pub version: String,
    pub preprocessing: String,
    pub split: String,
    pub split_manifest: String,
    pub n: u64,
    pub sample_ids_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct Model {
    pub name: String,
    pub version: String,

    pub quantization: Option<Quantization>,

    pub backend: String,
    pub placement: Placement,
    pub decoding_mode: DecodingMode,

    pub adapter: Option<String>,
    pub prompt_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Quantization {
    pub bits: u8,
    pub method: String,
    pub group_size: u32,
    pub calibration: String,
    pub quantizer: String,
    pub artifact_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    GpuResident,
    Offloaded,
    Cpu,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodingMode {
    Greedy,
    Sampling,
}

#[derive(Debug, Deserialize)]
pub struct Evaluator {
    pub kind: EvaluatorKind,
    pub name: String,
    pub version: String,

    pub prompt_version: Option<String>,
    pub temperature: Option<f64>,
    pub order_randomized: Option<bool>,
    pub kappa_vs_human: Option<f64>,
    pub kappa_measured_on: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatorKind {
    ExactMatch,
    EmbeddingSimilarity,
    LlmJudge,
    Human,
    MetricFn,
}

#[derive(Debug, Deserialize)]
pub struct Artifacts {
    pub outputs: Option<String>,
    pub trace: Option<String>,
    pub checkpoint: Option<String>,
    pub config_resolved: Option<String>,
    pub plot: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Cost {
    pub latency_ms_p50: u64,
    pub latency_ms_p95: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub usd: f64,
}

#[derive(Debug, Deserialize)]
pub struct Env {
    pub device: String,
    pub gpu: String,
    pub vram_gb: u64,
    pub torch: String,
    pub driver: String,
}

#[derive(Debug, Deserialize)]
pub struct Comparison {
    pub baseline_run_id: String,
    pub hypothesis: String,
    pub threshold: Threshold,
    pub declared_before_run: bool,
}

#[derive(Debug, Deserialize)]
pub struct Threshold {
    pub metric: String,
    pub direction: Direction,
    pub min_delta_pct: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Increase,
    Decrease,
}
