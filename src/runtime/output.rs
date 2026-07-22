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

impl<T: Serialize> Envelope<T> {
    /// Construct a successful envelope for `command` carrying `data`, with empty
    /// warnings and empty meta. Use the builder methods to attach warnings or
    /// meta when needed.
    pub fn new(command: &'static str, data: T) -> Self {
        Self {
            ok: true,
            command,
            data,
            warnings: Vec::new(),
            meta: serde_json::json!({}),
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    /// Override the `ok` flag. `Envelope::new` defaults to `true`; use this for
    /// commands whose success is data-dependent (e.g. patch-edit reports `ok`
    /// based on per-file failures, rearrange error envelopes report `false`).
    pub fn with_ok(mut self, ok: bool) -> Self {
        self.ok = ok;
        self
    }

    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = meta;
        self
    }
}

pub fn print_json<T: Serialize>(payload: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(payload)?);
    Ok(())
}
