use std::fmt::Display;

use chrono::NaiveDateTime;

pub struct AnyDateTime(NaiveDateTime);

impl Display for AnyDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for AnyDateTime {
    type Error = chrono::ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let fmt = "%Y-%m-%d %H:%M:%S%.f";
        let value = NaiveDateTime::parse_from_str(&value, fmt)?;

        Ok(AnyDateTime(value))
    }
}
