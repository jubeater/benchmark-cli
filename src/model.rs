use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Error,
    Timeout,
    RateLimited,
    Cancelled,
}

#[derive(Deserialize, Debug)]
pub struct Request {
    timestamp: String,
    model: String,
    latency_ms: u32,
    input_tokens: u64,
    output_tokens: u64,
    status: Status,
}

impl Request {
    pub fn latency_ms(&self) -> u32 {
        self.latency_ms
    }

    pub fn input_tokens(&self) -> u64 {
        self.input_tokens
    }

    pub fn output_tokens(&self) -> u64 {
        self.output_tokens
    }

    pub fn status(&self) -> &Status {
        &self.status
    }
    pub fn model_name(&self) -> &str {
        &self.model
    }
}

#[derive(Deserialize, Debug)]
pub struct Pricing {
    input_per_million: f64,
    output_per_million: f64,
}

impl Pricing {
    pub fn input_per_million(&self) -> f64 {
        self.input_per_million
    }
    pub fn output_per_million(&self) -> f64 {
        self.output_per_million
    }
}

pub type PricingTable = HashMap<String, Pricing>;
