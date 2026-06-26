//! Shared data model for `data-toc`: CLI arguments, budgets, output tree, and
//! JSONL grouping helpers.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// CLI surface
// ---------------------------------------------------------------------------

#[derive(Args, Debug)]
#[command(
    long_about = "Pre-scan JSON, JSONL, and YAML files and print an agent-facing structural table of contents.\n\nUse this before reading raw structured data into context. The output is a bounded structure map, not a JSON Schema, validator, or query language. YAML support uses external yq and is approximate. Values are hidden by default.",
    after_help = "Examples:\n  squire data-toc result.json\n  squire data-toc logs.jsonl --format jsonl\n  squire data-toc compose.yaml --format yaml\n  squire data-toc result.json --budget large\n  squire --print json data-toc result.json\n  squire data-toc --prompt"
)]
pub struct DataTocArgs {
    #[arg(help = "JSON, JSONL, or YAML file to inspect; not required with --prompt")]
    pub path: Option<PathBuf>,

    #[arg(
        long,
        value_enum,
        default_value_t = DataFormat::Auto,
        help = "Input format: auto, json, jsonl, yaml"
    )]
    pub format: DataFormat,

    #[arg(
        long,
        value_enum,
        default_value_t = Budget::Normal,
        help = "Scan budget: small, normal, large"
    )]
    pub budget: Budget,

    #[arg(long, help = "Print limited truncated/redacted example values")]
    pub examples: bool,

    #[arg(long, help = "Print the agent-facing data-toc guide")]
    pub prompt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DataFormat {
    Auto,
    Json,
    Jsonl,
    Yaml,
}

impl std::fmt::Display for DataFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(formatter, "auto"),
            Self::Json => write!(formatter, "json"),
            Self::Jsonl => write!(formatter, "jsonl"),
            Self::Yaml => write!(formatter, "yaml"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Budget {
    Small,
    Normal,
    Large,
}

impl std::fmt::Display for Budget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Small => write!(formatter, "small"),
            Self::Normal => write!(formatter, "normal"),
            Self::Large => write!(formatter, "large"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
// ---------------------------------------------------------------------------
// Budget and scan limits
// ---------------------------------------------------------------------------

pub(crate) struct BudgetProfile {
    pub(crate) max_depth: usize,
    pub(crate) max_children: usize,
    pub(crate) max_array_items: usize,
    pub(crate) max_json_bytes: u64,
    pub(crate) max_jsonl_lines: usize,
    pub(crate) max_groups: usize,
    pub(crate) max_signature_depth: usize,
    pub(crate) max_examples: usize,
}

impl Budget {
    pub(crate) fn profile(self) -> BudgetProfile {
        match self {
            Self::Small => BudgetProfile {
                max_depth: 4,
                max_children: 24,
                max_array_items: 32,
                max_json_bytes: 2 * 1024 * 1024,
                max_jsonl_lines: 200,
                max_groups: 4,
                max_signature_depth: 3,
                max_examples: 1,
            },
            Self::Normal => BudgetProfile {
                max_depth: 6,
                max_children: 64,
                max_array_items: 256,
                max_json_bytes: 10 * 1024 * 1024,
                max_jsonl_lines: 1000,
                max_groups: 8,
                max_signature_depth: 4,
                max_examples: 2,
            },
            Self::Large => BudgetProfile {
                max_depth: 10,
                max_children: 256,
                max_array_items: 2000,
                max_json_bytes: 64 * 1024 * 1024,
                max_jsonl_lines: 10_000,
                max_groups: 20,
                max_signature_depth: 6,
                max_examples: 3,
            },
        }
    }
}

#[derive(Debug, Serialize)]
// ---------------------------------------------------------------------------
// Output data model
// ---------------------------------------------------------------------------

pub(crate) struct DataTocData {
    pub(crate) path: String,
    pub(crate) format: DataFormat,
    pub(crate) mode: TocMode,
    pub(crate) complete: bool,
    pub(crate) root: TocNode,
    pub(crate) summary: DataSummary,
    pub(crate) notes: Vec<String>,
    pub(crate) suggested_reads: Vec<String>,
    pub(crate) record_groups: Vec<RecordGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parsed_as: Option<DataFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TocMode {
    StructureToc,
    RecordStreamToc,
}

impl TocMode {
    pub(crate) fn compact_name(self) -> &'static str {
        match self {
            Self::StructureToc => "structure-toc",
            Self::RecordStreamToc => "record-stream-toc",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct DataSummary {
    pub(crate) node_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sampled_lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sampled_records: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TocNode {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: NodeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) presence: Option<Presence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) observed_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) shape_count: Option<usize>,
    pub(crate) children: Vec<TocNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) examples: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NodeKind {
    Null,
    Boolean,
    Number,
    String,
    Object,
    Array,
    Mixed,
}

impl NodeKind {
    pub(crate) fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(_) => Self::Boolean,
            Value::Number(_) => Self::Number,
            Value::String(_) => Self::String,
            Value::Array(_) => Self::Array,
            Value::Object(_) => Self::Object,
        }
    }

    pub(crate) fn compact_name(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean => "boolean",
            Self::Number => "number",
            Self::String => "string",
            Self::Object => "object",
            Self::Array => "array",
            Self::Mixed => "mixed",
        }
    }

    pub(crate) fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::Null | Self::Boolean | Self::Number | Self::String
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Presence {
    pub(crate) observed: usize,
    pub(crate) total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordGroup {
    pub(crate) label: String,
    pub(crate) rows: usize,
    pub(crate) first_line: usize,
    pub(crate) shape: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct BuildState {
    /// Monotonic: once `true`, stays `true`. Drives `DataTocData.complete`.
    pub(crate) truncated: bool,
    /// Deduplicated warning strings appended to output notes.
    pub(crate) warnings: Vec<String>,
}

impl BuildState {
    pub(crate) fn new() -> Self {
        Self {
            truncated: false,
            warnings: Vec::new(),
        }
    }

    /// Record a warning without marking the scan as truncated.
    /// Used for non-lossy observations like dynamic key compression.
    pub(crate) fn warn(&mut self, warning: impl Into<String>) {
        let warning = warning.into();
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }

    /// Record a warning AND mark the scan as truncated.
    /// Used when budget limits cause structural information loss
    /// (depth limit, child limit, array item limit, line limit, group limit).
    pub(crate) fn truncate(&mut self, warning: impl Into<String>) {
        self.truncated = true;
        self.warn(warning);
    }
}

#[derive(Debug)]
// ---------------------------------------------------------------------------
// JSONL grouping helpers
// ---------------------------------------------------------------------------

pub(crate) struct JsonlRecord {
    pub(crate) line: usize,
    pub(crate) value: Value,
}

#[derive(Debug)]
pub(crate) struct ShapeGroup {
    pub(crate) records: Vec<usize>,
    pub(crate) first_line: usize,
    pub(crate) shape: Vec<String>,
}

pub(crate) struct ObjectFieldValues<'a> {
    pub(crate) key: String,
    pub(crate) values: Vec<&'a Value>,
    pub(crate) total: usize,
}

pub(crate) struct LabeledShapeGroup {
    pub(crate) label: Option<String>,
    pub(crate) records: Vec<usize>,
    pub(crate) first_line: usize,
    pub(crate) shape: Vec<String>,
}
