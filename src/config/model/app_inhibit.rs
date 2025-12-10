use std::fmt::{Display, Formatter, Result};
use regex::Regex;

#[derive(Debug, Clone)]
pub enum AppInhibitPattern {
    Literal(String),
    Regex(Regex),
}

impl Display for AppInhibitPattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            AppInhibitPattern::Literal(s) => write!(f, "{}", s),
            AppInhibitPattern::Regex(r) => write!(f, "(regex) {}", r.as_str()),
        }
    }
}
