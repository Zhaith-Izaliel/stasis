use regex::Regex;
use std::fmt;
use crate::config::model::AppInhibitPattern;

#[derive(Debug)]
pub enum PatternParseError {
    InvalidRegex(String),
}

impl fmt::Display for PatternParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternParseError::InvalidRegex(msg) => write!(f, "Invalid regex in inhibit_apps: {}", msg),
        }
    }
}

impl std::error::Error for PatternParseError {}

/// Parses a string into an AppInhibitPattern, detecting if it's a regex or literal
pub fn parse_app_pattern(s: &str) -> Result<AppInhibitPattern, PatternParseError> {
    let regex_meta = ['.', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\', '^', '$'];
    
    if s.chars().any(|c| regex_meta.contains(&c)) {
        Ok(AppInhibitPattern::Regex(
            Regex::new(s).map_err(|e| PatternParseError::InvalidRegex(e.to_string()))?
        ))
    } else {
        Ok(AppInhibitPattern::Literal(s.to_string()))
    }
}
