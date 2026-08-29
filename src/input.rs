use crate::model::{Pricing, Request};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub enum SourceError {
    SerdeJsonError(serde_json::Error),
}

pub struct ParseResult {
    pub requests: Vec<Request>,
    pub errors: Vec<ParseError>,
}

pub struct ParseError {
    pub line_number: usize,
    pub raw_input: String,
    pub source_error: SourceError,
}

#[derive(Debug)]
pub enum PricingLoadError {
    Io(io::Error),
    Json(serde_json::Error),
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
            raw_input: line.to_string(),
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
    let mut line_cnt: usize = 1;

    for line in reader.lines() {
        match line {
            Ok(str_line) => {
                let line_parse_result = parse_line(&str_line, line_cnt);
                match line_parse_result {
                    Ok(line_parse_ok) => {
                        res.requests.push(line_parse_ok);
                    }
                    Err(e) => {
                        res.errors.push(e);
                    }
                }
            }
            Err(e) => return Err(e),
        }
        line_cnt += 1;
    }

    return Ok(res);
}

pub fn parse_pricing_model(file_path: &str) -> Result<HashMap<String, Pricing>, PricingLoadError> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let map = serde_json::from_reader(reader)?;
    Ok(map)
}
