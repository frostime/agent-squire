use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::Value;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

const DATA_TOC_PROMPT: &str = r#"# Squire data-toc guide

`asq data-toc` summarizes JSON, JSONL, and YAML structure before an agent reads raw data content.

## When to use

- Unknown JSON / JSONL / YAML files where structure matters more than values.
- Large arrays with repeated objects.
- JSONL logs or event streams that may contain multiple record shapes.
- YAML configuration files when `yq` is available.
- Before choosing precise `jq`, `sed`, or `read-range` follow-up reads.

## Commands

```bash
asq data-toc result.json
asq data-toc logs.jsonl --format jsonl
asq data-toc compose.yaml --format yaml
asq data-toc result.json --budget large
asq --print json data-toc result.json
```

## Output interpretation

- `[]` means array indexes are collapsed.
- `?` means observed in only part of the sample.
- `complete=false` means budget limits or sampling affected the scan.
- JSONL record groups are approximate structural clusters.
- Values are hidden by default; use follow-up reads for representative samples.

## Follow-up reads

```bash
jq '.runs[0:5]' result.json
sed -n '37p' logs.jsonl | jq .
```
"#;

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
struct BudgetProfile {
    max_depth: usize,
    max_children: usize,
    max_array_items: usize,
    max_json_bytes: u64,
    max_jsonl_lines: usize,
    max_groups: usize,
    max_signature_depth: usize,
}

impl Budget {
    fn profile(self) -> BudgetProfile {
        match self {
            Self::Small => BudgetProfile {
                max_depth: 4,
                max_children: 24,
                max_array_items: 32,
                max_json_bytes: 2 * 1024 * 1024,
                max_jsonl_lines: 200,
                max_groups: 4,
                max_signature_depth: 3,
            },
            Self::Normal => BudgetProfile {
                max_depth: 6,
                max_children: 64,
                max_array_items: 256,
                max_json_bytes: 10 * 1024 * 1024,
                max_jsonl_lines: 1000,
                max_groups: 8,
                max_signature_depth: 4,
            },
            Self::Large => BudgetProfile {
                max_depth: 10,
                max_children: 256,
                max_array_items: 2000,
                max_json_bytes: 64 * 1024 * 1024,
                max_jsonl_lines: 10_000,
                max_groups: 20,
                max_signature_depth: 6,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct DataTocData {
    path: String,
    format: DataFormat,
    mode: TocMode,
    complete: bool,
    root: TocNode,
    summary: DataSummary,
    notes: Vec<String>,
    suggested_reads: Vec<String>,
    record_groups: Vec<RecordGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parsed_as: Option<DataFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TocMode {
    StructureToc,
    RecordStreamToc,
}

impl TocMode {
    fn compact_name(self) -> &'static str {
        match self {
            Self::StructureToc => "structure-toc",
            Self::RecordStreamToc => "record-stream-toc",
        }
    }
}

#[derive(Debug, Serialize)]
struct DataSummary {
    node_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    sampled_lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sampled_records: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct TocNode {
    name: String,
    path: String,
    kind: NodeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence: Option<Presence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shape_count: Option<usize>,
    children: Vec<TocNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    Null,
    Boolean,
    Number,
    String,
    Object,
    Array,
    Mixed,
}

impl NodeKind {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(_) => Self::Boolean,
            Value::Number(_) => Self::Number,
            Value::String(_) => Self::String,
            Value::Array(_) => Self::Array,
            Value::Object(_) => Self::Object,
        }
    }

    fn compact_name(self) -> &'static str {
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

    fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::Null | Self::Boolean | Self::Number | Self::String
        )
    }
}

#[derive(Debug, Clone, Serialize)]
struct Presence {
    observed: usize,
    total: usize,
}

#[derive(Debug, Clone, Serialize)]
struct RecordGroup {
    label: String,
    rows: usize,
    first_line: usize,
    shape: Vec<String>,
}

#[derive(Debug)]
struct BuildState {
    truncated: bool,
    warnings: Vec<String>,
}

impl BuildState {
    fn new() -> Self {
        Self {
            truncated: false,
            warnings: Vec::new(),
        }
    }

    fn truncate(&mut self, warning: impl Into<String>) {
        self.truncated = true;
        let warning = warning.into();
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }
}

#[derive(Debug)]
struct JsonlRecord {
    line: usize,
    value: Value,
}

#[derive(Debug)]
struct ShapeGroup {
    records: Vec<usize>,
    first_line: usize,
    shape: Vec<String>,
}

pub fn run(args: DataTocArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{DATA_TOC_PROMPT}");
        return Ok(0);
    }

    let path = args
        .path
        .as_deref()
        .context("missing path; use --prompt for the agent-facing guide")?;
    if !path.is_file() {
        bail!("path is not a file: {}", path.display());
    }

    let format = resolve_format(path, args.format)?;
    let profile = args.budget.profile();
    let (data, warnings) = match format {
        DataFormat::Auto => unreachable!("auto format should be resolved"),
        DataFormat::Json => analyze_json(path, profile)?,
        DataFormat::Jsonl => analyze_jsonl(path, profile)?,
        DataFormat::Yaml => analyze_yaml(path, profile)?,
    };

    match ctx.print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "data-toc",
                data,
                warnings,
                meta: serde_json::json!({
                    "budget": args.budget,
                    "schema_version": 1,
                }),
            };
            output::print_json(&payload)?;
        }
        _ => print_compact(&data, &warnings, args.budget),
    }

    Ok(0)
}

fn resolve_format(path: &Path, requested: DataFormat) -> Result<DataFormat> {
    if requested != DataFormat::Auto {
        return Ok(requested);
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "json" => Ok(DataFormat::Json),
        "jsonl" | "ndjson" => Ok(DataFormat::Jsonl),
        "yaml" | "yml" => Ok(DataFormat::Yaml),
        _ => bail!(
            "format could not be detected; pass --format json, --format jsonl, or --format yaml"
        ),
    }
}

fn analyze_json(path: &Path, profile: BudgetProfile) -> Result<(DataTocData, Vec<String>)> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    if metadata.len() > profile.max_json_bytes {
        bail!(
            "JSON file exceeds {limit} byte {budget} budget; retry with a larger --budget",
            limit = profile.max_json_bytes,
            budget = budget_name(profile)
        );
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {} as UTF-8", path.display()))?;
    let value: Value = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON: {}", path.display()))?;

    Ok(analyze_json_value(
        path,
        DataFormat::Json,
        None,
        value,
        profile,
        Vec::new(),
    ))
}

fn analyze_yaml(path: &Path, profile: BudgetProfile) -> Result<(DataTocData, Vec<String>)> {
    let value = yaml_to_json(path)?;
    Ok(analyze_json_value(
        path,
        DataFormat::Yaml,
        Some(DataFormat::Json),
        value,
        profile,
        vec![
            "format=yaml parsed_as=json".to_string(),
            "YAML comments, anchors, aliases, tags, and formatting are not preserved.".to_string(),
        ],
    ))
}

fn yaml_to_json(path: &Path) -> Result<Value> {
    let attempts: &[&[&str]] = &[&["-o=json", "."], &["."]];
    let mut errors = Vec::new();

    for args in attempts {
        let output = Command::new("yq")
            .args(*args)
            .arg(path)
            .output()
            .context("YAML support requires yq")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if !stderr.is_empty() {
                errors.push(stderr);
            }
            continue;
        }
        match serde_json::from_slice::<Value>(&output.stdout) {
            Ok(value) => return Ok(value),
            Err(err) => errors.push(format!("yq output was not valid JSON: {err}")),
        }
    }

    if errors.is_empty() {
        bail!("yq failed to parse YAML input");
    }
    bail!("yq failed to parse YAML input: {}", errors.join("; "))
}

fn analyze_json_value(
    path: &Path,
    format: DataFormat,
    parsed_as: Option<DataFormat>,
    value: Value,
    profile: BudgetProfile,
    extra_notes: Vec<String>,
) -> (DataTocData, Vec<String>) {
    let mut state = BuildState::new();
    let root = summarize_values("$", "$", &[&value], None, profile, 0, &mut state);
    let suggested_reads = suggested_reads_for_json(path, &root, format);
    let mut notes = default_notes();
    notes.extend(extra_notes);
    notes.extend(state.warnings.iter().cloned());

    let data = DataTocData {
        path: display_path(path),
        format,
        mode: TocMode::StructureToc,
        complete: !state.truncated,
        summary: DataSummary {
            node_count: count_nodes(&root),
            sampled_lines: None,
            sampled_records: None,
        },
        root,
        notes,
        suggested_reads,
        record_groups: Vec::new(),
        parsed_as,
    };

    (data, Vec::new())
}

fn analyze_jsonl(path: &Path, profile: BudgetProfile) -> Result<(DataTocData, Vec<String>)> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    let mut sampled_lines = 0usize;
    let mut truncated_lines = false;

    for (line_index, line_result) in reader.lines().enumerate() {
        if sampled_lines >= profile.max_jsonl_lines {
            truncated_lines = true;
            break;
        }
        let line_no = line_index + 1;
        let line = line_result.with_context(|| format!("failed to read line {line_no}"))?;
        sampled_lines += 1;
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(&line)
            .with_context(|| format!("invalid JSONL at line {line_no}"))?;
        records.push(JsonlRecord {
            line: line_no,
            value,
        });
    }

    if records.is_empty() {
        bail!("JSONL contains no records in sampled lines");
    }

    let mut state = BuildState::new();
    if truncated_lines {
        state.truncate(format!(
            "Stopped after sampled_lines={sampled_lines} due to budget."
        ));
    }

    let values = records
        .iter()
        .map(|record| &record.value)
        .collect::<Vec<_>>();
    let element = summarize_values("[]", "$[]", &values, None, profile, 0, &mut state);
    let root = TocNode {
        name: "$".to_string(),
        path: "$".to_string(),
        kind: NodeKind::Array,
        presence: None,
        observed_items: Some(records.len()),
        shape_count: Some(exact_shape_count(&records, profile)),
        children: vec![element],
    };

    let record_groups = group_jsonl_records(&records, profile, &mut state);
    let mut notes = default_notes();
    notes.push(if record_groups.len() > 1 {
        "JSONL records appear heterogeneous.".to_string()
    } else {
        "JSONL records appear homogeneous in the observed sample.".to_string()
    });
    notes.push("Groups are approximate structural clusters.".to_string());
    notes.extend(state.warnings.iter().cloned());

    let suggested_reads = record_groups
        .iter()
        .filter(|group| group.label != "other")
        .take(3)
        .map(|group| {
            format!(
                "sed -n '{}p' {} | jq .",
                group.first_line,
                display_path(path)
            )
        })
        .collect::<Vec<_>>();

    let data = DataTocData {
        path: display_path(path),
        format: DataFormat::Jsonl,
        mode: TocMode::RecordStreamToc,
        complete: !state.truncated,
        summary: DataSummary {
            node_count: count_nodes(&root),
            sampled_lines: Some(sampled_lines),
            sampled_records: Some(records.len()),
        },
        root,
        notes,
        suggested_reads,
        record_groups,
        parsed_as: None,
    };

    Ok((data, Vec::new()))
}

fn summarize_values(
    name: &str,
    path: &str,
    values: &[&Value],
    presence: Option<Presence>,
    profile: BudgetProfile,
    depth: usize,
    state: &mut BuildState,
) -> TocNode {
    if values.is_empty() {
        return TocNode {
            name: name.to_string(),
            path: path.to_string(),
            kind: NodeKind::Null,
            presence,
            observed_items: None,
            shape_count: None,
            children: Vec::new(),
        };
    }

    if depth >= profile.max_depth {
        state.truncate(format!(
            "Depth limit {} reached at {path}.",
            profile.max_depth
        ));
        return TocNode {
            name: name.to_string(),
            path: path.to_string(),
            kind: combined_kind(values),
            presence,
            observed_items: None,
            shape_count: None,
            children: Vec::new(),
        };
    }

    match combined_kind(values) {
        NodeKind::Object => {
            summarize_object_values(name, path, values, presence, profile, depth, state)
        }
        NodeKind::Array => {
            summarize_array_values(name, path, values, presence, profile, depth, state)
        }
        kind => TocNode {
            name: name.to_string(),
            path: path.to_string(),
            kind,
            presence,
            observed_items: None,
            shape_count: None,
            children: Vec::new(),
        },
    }
}

fn summarize_object_values(
    name: &str,
    path: &str,
    values: &[&Value],
    presence: Option<Presence>,
    profile: BudgetProfile,
    depth: usize,
    state: &mut BuildState,
) -> TocNode {
    let objects = values
        .iter()
        .filter_map(|value| value.as_object())
        .collect::<Vec<_>>();
    let mut field_values: BTreeMap<String, Vec<&Value>> = BTreeMap::new();

    for object in &objects {
        for (key, value) in object.iter() {
            field_values.entry(key.clone()).or_default().push(value);
        }
    }

    let total_fields = field_values.len();
    if total_fields > profile.max_children {
        state.truncate(format!(
            "Child limit {} reached at {path}; omitted {} field(s).",
            profile.max_children,
            total_fields - profile.max_children
        ));
    }

    let children = field_values
        .into_iter()
        .take(profile.max_children)
        .map(|(key, child_values)| {
            let child_presence = if objects.len() > 1 {
                Some(Presence {
                    observed: child_values.len(),
                    total: objects.len(),
                })
            } else {
                None
            };
            let child_path = append_path_key(path, &key);
            summarize_values(
                &key,
                &child_path,
                &child_values,
                child_presence,
                profile,
                depth + 1,
                state,
            )
        })
        .collect::<Vec<_>>();

    TocNode {
        name: name.to_string(),
        path: path.to_string(),
        kind: NodeKind::Object,
        presence,
        observed_items: None,
        shape_count: None,
        children,
    }
}

fn summarize_array_values(
    name: &str,
    path: &str,
    values: &[&Value],
    presence: Option<Presence>,
    profile: BudgetProfile,
    depth: usize,
    state: &mut BuildState,
) -> TocNode {
    let arrays = values
        .iter()
        .filter_map(|value| value.as_array())
        .collect::<Vec<_>>();
    let total_items = arrays.iter().map(|array| array.len()).sum::<usize>();
    let mut sampled_items = Vec::new();

    for array in &arrays {
        for item in array.iter() {
            if sampled_items.len() >= profile.max_array_items {
                break;
            }
            sampled_items.push(item);
        }
        if sampled_items.len() >= profile.max_array_items {
            break;
        }
    }

    if total_items > sampled_items.len() {
        state.truncate(format!(
            "Array item limit {} reached at {path}; observed {}/{} item(s).",
            profile.max_array_items,
            sampled_items.len(),
            total_items
        ));
    }

    let shape_count = if sampled_items.is_empty() {
        None
    } else {
        Some(shape_count(&sampled_items, profile.max_signature_depth))
    };
    let child_path = format!("{path}[]");
    let children = if sampled_items.is_empty() {
        Vec::new()
    } else {
        vec![summarize_values(
            "[]",
            &child_path,
            &sampled_items,
            None,
            profile,
            depth + 1,
            state,
        )]
    };

    TocNode {
        name: name.to_string(),
        path: path.to_string(),
        kind: NodeKind::Array,
        presence,
        observed_items: Some(sampled_items.len()),
        shape_count,
        children,
    }
}

fn combined_kind(values: &[&Value]) -> NodeKind {
    let kinds = values
        .iter()
        .map(|value| NodeKind::from_value(value))
        .collect::<BTreeSet<_>>();
    if kinds.len() == 1 {
        *kinds.iter().next().expect("one kind exists")
    } else {
        NodeKind::Mixed
    }
}

fn append_path_key(path: &str, key: &str) -> String {
    if is_simple_key(key) {
        format!("{path}.{key}")
    } else {
        let escaped = key.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{path}[\"{escaped}\"]")
    }
}

fn is_simple_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn shape_count(values: &[&Value], max_depth: usize) -> usize {
    values
        .iter()
        .map(|value| structural_signature(value, max_depth).join("\n"))
        .collect::<BTreeSet<_>>()
        .len()
}

fn exact_shape_count(records: &[JsonlRecord], profile: BudgetProfile) -> usize {
    records
        .iter()
        .map(|record| structural_signature(&record.value, profile.max_signature_depth).join("\n"))
        .collect::<BTreeSet<_>>()
        .len()
}

fn structural_signature(value: &Value, max_depth: usize) -> Vec<String> {
    let mut signature = Vec::new();
    collect_signature(value, "$", 0, max_depth, &mut signature);
    signature.sort();
    signature
}

fn collect_signature(
    value: &Value,
    path: &str,
    depth: usize,
    max_depth: usize,
    signature: &mut Vec<String>,
) {
    let kind = NodeKind::from_value(value);
    signature.push(format!("{path}:{}", kind.compact_name()));
    if depth >= max_depth {
        return;
    }

    match value {
        Value::Object(object) => {
            for (key, child) in object {
                collect_signature(
                    child,
                    &append_path_key(path, key),
                    depth + 1,
                    max_depth,
                    signature,
                );
            }
        }
        Value::Array(array) => {
            for child in array.iter().take(3) {
                collect_signature(child, &format!("{path}[]"), depth + 1, max_depth, signature);
            }
        }
        _ => {}
    }
}

fn group_jsonl_records(
    records: &[JsonlRecord],
    profile: BudgetProfile,
    state: &mut BuildState,
) -> Vec<RecordGroup> {
    let mut exact_groups: BTreeMap<String, ShapeGroup> = BTreeMap::new();
    for (record_index, record) in records.iter().enumerate() {
        let signature = structural_signature(&record.value, profile.max_signature_depth);
        let key = signature.join("\n");
        exact_groups
            .entry(key)
            .and_modify(|group| {
                group.records.push(record_index);
                group.first_line = group.first_line.min(record.line);
            })
            .or_insert_with(|| ShapeGroup {
                records: vec![record_index],
                first_line: record.line,
                shape: vec![shape_summary(&record.value)],
            });
    }

    let mut groups = exact_groups.into_values().collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right
            .records
            .len()
            .cmp(&left.records.len())
            .then_with(|| left.first_line.cmp(&right.first_line))
    });

    let overflow = if groups.len() > profile.max_groups {
        Some(groups.split_off(profile.max_groups))
    } else {
        None
    };

    let mut record_groups = groups
        .into_iter()
        .enumerate()
        .map(|(index, group)| RecordGroup {
            label: discriminator_label(records, &group)
                .unwrap_or_else(|| format!("shape#{}", index + 1)),
            rows: group.records.len(),
            first_line: group.first_line,
            shape: group.shape,
        })
        .collect::<Vec<_>>();

    if let Some(overflow_groups) = overflow {
        let rows = overflow_groups
            .iter()
            .map(|group| group.records.len())
            .sum::<usize>();
        let first_line = overflow_groups
            .iter()
            .map(|group| group.first_line)
            .min()
            .unwrap_or(0);
        state.truncate(format!(
            "Record group limit {} reached; {} minor group(s) collapsed into other.",
            profile.max_groups,
            overflow_groups.len()
        ));
        record_groups.push(RecordGroup {
            label: "other".to_string(),
            rows,
            first_line,
            shape: vec![format!("minor_shapes≈{}", overflow_groups.len())],
        });
    }

    record_groups
}

fn discriminator_label(records: &[JsonlRecord], group: &ShapeGroup) -> Option<String> {
    const CANDIDATES: &[&str] = &[
        "type",
        "kind",
        "event_type",
        "event",
        "action",
        "op",
        "role",
        "level",
        "category",
    ];

    for candidate in CANDIDATES {
        let mut values = BTreeSet::new();
        let mut all_present = true;
        for record_index in &group.records {
            let record = &records[*record_index];
            let Some(object) = record.value.as_object() else {
                all_present = false;
                break;
            };
            let Some(value) = object.get(*candidate) else {
                all_present = false;
                break;
            };
            let Some(label_value) = scalar_label_value(value) else {
                all_present = false;
                break;
            };
            values.insert(label_value);
        }

        if all_present && values.len() == 1 {
            let value = values.into_iter().next().expect("one discriminator value");
            return Some(format!("{candidate}={value}"));
        }
    }

    None
}

fn scalar_label_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(truncate_label(text)),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn truncate_label(text: &str) -> String {
    const MAX_CHARS: usize = 24;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join("_");
    if normalized.chars().count() <= MAX_CHARS {
        normalized
    } else {
        let mut truncated = normalized.chars().take(MAX_CHARS).collect::<String>();
        truncated.push('…');
        truncated
    }
}

fn shape_summary(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let keys = object.keys().cloned().collect::<Vec<_>>().join(",");
            format!("object{{{keys}}}")
        }
        Value::Array(_) => "array".to_string(),
        _ => NodeKind::from_value(value).compact_name().to_string(),
    }
}

fn count_nodes(node: &TocNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

fn default_notes() -> Vec<String> {
    vec![
        "Output is based on bounded structural scanning.".to_string(),
        "`?` means not present in all observed samples.".to_string(),
        "Array indexes are collapsed into [].".to_string(),
    ]
}

fn suggested_reads_for_json(path: &Path, root: &TocNode, format: DataFormat) -> Vec<String> {
    if format == DataFormat::Yaml {
        return Vec::new();
    }

    let Some(array_path) = find_first_array_path(root) else {
        return vec![format!("jq '.' {}", display_path(path))];
    };
    let jq_path = array_path.trim_start_matches('$');
    vec![format!("jq '{jq_path}[0:5]' {}", display_path(path))]
}

fn find_first_array_path(node: &TocNode) -> Option<String> {
    if node.kind == NodeKind::Array && node.path != "$" {
        return Some(node.path.clone());
    }
    node.children.iter().find_map(find_first_array_path)
}

fn print_compact(data: &DataTocData, warnings: &[String], budget: Budget) {
    let mut header = format!(
        "format={} mode={} complete={} budget={}",
        data.format,
        data.mode.compact_name(),
        data.complete,
        budget
    );
    if let Some(parsed_as) = data.parsed_as {
        header.push_str(&format!(" parsed_as={parsed_as}"));
    }
    if let Some(sampled_lines) = data.summary.sampled_lines {
        header.push_str(&format!(" sampled_lines={sampled_lines}"));
    }

    println!("# data-toc {}", data.path);
    println!("{header}");
    println!();

    if data.format == DataFormat::Jsonl {
        println!(
            "$ array<record> virtual=jsonl groups≈{}",
            data.record_groups.len()
        );
        for child in renderable_children(&data.root) {
            render_node(child, "", true);
        }
    } else {
        println!("{}", compact_node_label(&data.root));
        render_children(&data.root, "");
    }

    if !data.record_groups.is_empty() {
        println!();
        println!("Record groups:");
        for group in &data.record_groups {
            println!(
                "- {} rows={} first_line={}",
                group.label, group.rows, group.first_line
            );
            println!("  shape: {}", group.shape.join(", "));
        }
    }

    if !data.notes.is_empty() || !warnings.is_empty() {
        println!();
        println!("Notes:");
        for note in &data.notes {
            println!("- {note}");
        }
        for warning in warnings {
            println!("- {warning}");
        }
    }

    if !data.suggested_reads.is_empty() {
        println!();
        println!("Suggested reads:");
        for read in &data.suggested_reads {
            println!("- {read}");
        }
    }
}

fn render_children(node: &TocNode, prefix: &str) {
    let children = renderable_children(node);
    let child_count = children.len();
    for (index, child) in children.into_iter().enumerate() {
        render_node(child, prefix, index + 1 == child_count);
    }
}

fn render_node(node: &TocNode, prefix: &str, is_last: bool) {
    let connector = if is_last { "└─" } else { "├─" };
    println!("{prefix}{connector} {}", compact_node_label(node));
    let next_prefix = if is_last {
        format!("{prefix}   ")
    } else {
        format!("{prefix}│  ")
    };
    render_children(node, &next_prefix);
}

fn renderable_children(node: &TocNode) -> Vec<&TocNode> {
    if node.kind == NodeKind::Array
        && node.children.len() == 1
        && node.children[0].children.is_empty()
        && node.children[0].kind.is_scalar()
    {
        Vec::new()
    } else {
        node.children.iter().collect()
    }
}

fn compact_node_label(node: &TocNode) -> String {
    let mut label = format!("{} {}", node.name, compact_kind_label(node));
    if let Some(presence) = &node.presence {
        if presence.total > 1 {
            if presence.observed == presence.total {
                label.push_str(&format!(" {}/{}", presence.observed, presence.total));
            } else {
                label.push_str(&format!("? {}/{}", presence.observed, presence.total));
            }
        }
    }
    if let Some(observed_items) = node.observed_items {
        label.push_str(&format!(" observed_items≈{observed_items}"));
    }
    if let Some(shape_count) = node.shape_count {
        if shape_count > 1 {
            label.push_str(&format!(" shape≈{shape_count}"));
        }
    }
    label
}

fn compact_kind_label(node: &TocNode) -> String {
    if node.kind == NodeKind::Array {
        if let Some(child) = node.children.first() {
            return format!("array<{}>", child.kind.compact_name());
        }
    }
    node.kind.compact_name().to_string()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn budget_name(profile: BudgetProfile) -> &'static str {
    match profile.max_json_bytes {
        bytes if bytes == Budget::Small.profile().max_json_bytes => "small",
        bytes if bytes == Budget::Normal.profile().max_json_bytes => "normal",
        _ => "large",
    }
}
