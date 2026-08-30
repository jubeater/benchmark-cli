use crate::model::{Pricing, Request};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

#[derive(Debug)]
pub enum SourceError {
    SerdeJsonError(serde_json::Error),
}

impl Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::SerdeJsonError(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub requests: Vec<Request>,
    pub errors: Vec<ParseError>,
}

#[derive(Debug)]
pub struct ParseError {
    pub line_number: usize,
    pub source_error: SourceError,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line_number, self.source_error)
    }
}

#[derive(Debug)]
pub enum PricingLoadError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl Display for PricingLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PricingLoadError::Io(e) => write!(f, "{}", e),
            PricingLoadError::Json(e) => write!(f, "{}", e),
        }
    }
}

impl From<io::Error> for PricingLoadError {
    fn from(value: io::Error) -> Self {
        PricingLoadError::Io(value)
    }
}

impl From<serde_json::Error> for PricingLoadError {
    fn from(value: serde_json::Error) -> Self {
        PricingLoadError::Json(value)
    }
}

pub fn parse_line(line: &str, line_number: usize) -> Result<Request, ParseError> {
    // ...
    let json_parsed_line: Result<Request, serde_json::Error> = serde_json::from_str(line);
    match json_parsed_line {
        Ok(request) => Ok(request),
        Err(e) => Err(ParseError {
            line_number,
            source_error: SourceError::SerdeJsonError(e),
        }),
    }
}

pub fn parse_lines(file_path: &str) -> Result<ParseResult, io::Error> {
    let mut res = ParseResult {
        requests: Vec::new(),
        errors: Vec::new(),
    };

    let file = File::open(file_path)?;

    let reader = BufReader::new(file);

    for (line_cnt, line) in (1..).zip(reader.lines()) {
        let str_line = line?;

        match parse_line(&str_line, line_cnt) {
            Ok(line_parse_ok) => {
                res.requests.push(line_parse_ok);
            }
            Err(e) => {
                res.errors.push(e);
            }
        }
    }

    Ok(res)
}

pub fn parse_pricing_model(file_path: &str) -> Result<HashMap<String, Pricing>, PricingLoadError> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let map = serde_json::from_reader(reader)?;
    Ok(map)
}

#[cfg(test)]
mod tests {

    use crate::model::Status;

    use super::*;

    #[test]
    fn test_parse_line_valid() {
        let json_input = r#"
        {
            "timestamp": "2026-08-27T12:00:01Z",
            "model": "gpt-5",
            "latency_ms": 182,
            "input_tokens": 1200,
            "output_tokens": 350,
            "status": "ok"
        }
        "#;
        let result = parse_line(json_input, 3);
        assert!(result.is_ok());
        let ok_result = result.unwrap();
        assert_eq!(ok_result.input_tokens(), 1200);
        assert_eq!(ok_result.output_tokens(), 350);
        assert_eq!(ok_result.latency_ms(), 182);
        assert_eq!(ok_result.model_name(), "gpt-5");
        assert_eq!(ok_result.status(), &Status::Ok);
    }

    #[test]
    fn test_parse_line_invalid() {
        let json_input = r#"
        {
            "timestamp": "2026-08-27T12:00:01Z",
            "model": "gpt-5",
            "latency_ms": 182,
            "input_tokens": 1200,
            "output_tokens": 350,
            "status": "blahblahblah"
        }
        "#;
        let result = parse_line(json_input, 4);
        assert!(result.is_err());
        let err_result = result.unwrap_err();
        assert_eq!(err_result.line_number, 4);
        assert!(matches!(
            err_result.source_error,
            SourceError::SerdeJsonError(_)
        ));
    }

    #[test]
    fn parse_lines_mixed_records() {
        let result = parse_lines("./data/test/valid_mixed.jsonl");
        assert!(result.is_ok());
        let ok_result = result.unwrap();
        assert_eq!(ok_result.requests.len(), 2);
        assert_eq!(ok_result.errors.len(), 1);
        assert_eq!(ok_result.errors[0].line_number, 2);
    }

    #[test]
    fn test_parse_lines_missing_file() {
        let result = parse_lines("invalid_path.jsonl");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn parse_pricing_model_valid() {
        let result = parse_pricing_model("./data/test/valid_pricing.json");
        assert!(result.is_ok());
        let ok_result = result.unwrap();
        assert!(ok_result.contains_key("gpt-5"));
        let gpt_5_price = ok_result.get("gpt-5").unwrap();
        assert_eq!(gpt_5_price.input_per_million(), 1.25);
        assert_eq!(gpt_5_price.output_per_million(), 10.00);

        assert!(ok_result.contains_key("claude-sonnet-5"));
        let claude_price = ok_result.get("claude-sonnet-5").unwrap();
        assert_eq!(claude_price.input_per_million(), 3.0);
        assert_eq!(claude_price.output_per_million(), 15.0);

        let non_exist_model_price = ok_result.get("unknown-model");
        assert!(non_exist_model_price.is_none());
    }

    #[test]
    fn parse_pricing_model_missing_file() {
        let result = parse_pricing_model("invalid_path.json");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PricingLoadError::Io(_)));
    }

    #[test]
    fn parse_pricing_model_invalid_json() {
        let result = parse_pricing_model("./data/test/invalid_pricing.json");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PricingLoadError::Json(_)));
    }
}
