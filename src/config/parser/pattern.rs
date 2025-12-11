use eyre::{Result, WrapErr};
use regex::Regex;

use crate::config::model::AppInhibitPattern;

/// Parses a string into an AppInhibitPattern, detecting if it's a regex or literal
pub fn parse_app_pattern(s: &str) -> Result<AppInhibitPattern> {
    let regex_meta = ['.', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\', '^', '$'];
    if s.chars().any(|c| regex_meta.contains(&c)) {
        Ok(AppInhibitPattern::Regex(Regex::new(s).wrap_err("invalid regex in inhibit_apps")?))
    } else {
        Ok(AppInhibitPattern::Literal(s.to_string()))
    }
}
