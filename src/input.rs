use serde_json::Value;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

#[derive(Debug)]
pub struct InputRecord {
    pub line: usize,
    pub value: Value,
}

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
    pub records: Vec<InputRecord>,
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

pub fn parse_line(line: &str, line_number: usize) -> Result<InputRecord, ParseError> {
    match serde_json::from_str::<Value>(line) {
        Ok(value) => Ok(InputRecord {
            line: line_number,
            value,
        }),
        Err(e) => Err(ParseError {
            line_number,
            source_error: SourceError::SerdeJsonError(e),
        }),
    }
}

pub fn parse_lines(file_path: &str) -> Result<ParseResult, io::Error> {
    let mut res = ParseResult {
        records: Vec::new(),
        errors: Vec::new(),
    };

    let file = File::open(file_path)?;

    let reader = BufReader::new(file);

    for (line_cnt, line) in (1..).zip(reader.lines()) {
        let str_line = line?;

        match parse_line(&str_line, line_cnt) {
            Ok(input_record) => {
                res.records.push(input_record);
            }
            Err(e) => {
                res.errors.push(e);
            }
        }
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_valid_json() {
        let json_input = r#"{"model":"gpt-5","status":"not_yet_validated"}"#;

        let result = parse_line(json_input, 3).unwrap();

        assert_eq!(result.line, 3);
        assert_eq!(result.value["model"], "gpt-5");
        assert_eq!(result.value["status"], "not_yet_validated");
    }

    #[test]
    fn parse_line_invalid_json() {
        let json_input = r#"{"model":"gpt-5","status":"ok""#;

        let result = parse_line(json_input, 4);

        let error = result.unwrap_err();
        assert_eq!(error.line_number, 4);
        assert!(matches!(error.source_error, SourceError::SerdeJsonError(_)));
    }

    #[test]
    fn parse_lines_keeps_all_valid_json_records() {
        let result = parse_lines("./data/test/valid_mixed.jsonl");

        let parsed = result.unwrap();
        assert_eq!(parsed.records.len(), 3);
        assert!(parsed.errors.is_empty());
    }

    #[test]
    fn test_parse_lines_missing_file() {
        let result = parse_lines("invalid_path.jsonl");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }
}
