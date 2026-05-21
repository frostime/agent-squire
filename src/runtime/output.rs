use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PrintMode {
    Compact,
    Json,
    Ndjson,
    Text,
    Raw,
}

impl std::fmt::Display for PrintMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compact => write!(f, "compact"),
            Self::Json => write!(f, "json"),
            Self::Ndjson => write!(f, "ndjson"),
            Self::Text => write!(f, "text"),
            Self::Raw => write!(f, "raw"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub ok: bool,
    pub command: &'static str,
    pub data: T,
    pub warnings: Vec<String>,
    pub meta: serde_json::Value,
}

pub fn print_json<T: Serialize>(payload: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(payload)?);
    Ok(())
}
