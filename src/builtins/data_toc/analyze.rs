use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::builtins::data_toc::types::*;
use crate::builtins::data_toc::util::*;

// ---------------------------------------------------------------------------
// Format-specific analyzers
// ---------------------------------------------------------------------------

pub(crate) fn analyze_json(
    path: &Path,
    profile: BudgetProfile,
    include_examples: bool,
) -> Result<(DataTocData, Vec<String>)> {
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
        include_examples,
        Vec::new(),
    ))
}

pub(crate) fn analyze_yaml(
    path: &Path,
    profile: BudgetProfile,
    include_examples: bool,
) -> Result<(DataTocData, Vec<String>)> {
    let value = yaml_to_json(path)?;
    Ok(analyze_json_value(
        path,
        DataFormat::Yaml,
        Some(DataFormat::Json),
        value,
        profile,
        include_examples,
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
    include_examples: bool,
    extra_notes: Vec<String>,
) -> (DataTocData, Vec<String>) {
    let mut state = BuildState::new();
    let root = summarize_values(
        "$",
        "$",
        &[&value],
        None,
        profile,
        0,
        include_examples,
        &mut state,
    );
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

pub(crate) fn analyze_jsonl(
    path: &Path,
    profile: BudgetProfile,
    include_examples: bool,
) -> Result<(DataTocData, Vec<String>)> {
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

    // Collect references for the virtual array summarizer.
    // `sampled_lines` counts all lines read (including empty ones that were
    // skipped above); `records.len()` counts only successfully parsed records.
    let values = records
        .iter()
        .map(|record| &record.value)
        .collect::<Vec<_>>();
    let element = summarize_values(
        "[]",
        "$[]",
        &values,
        None,
        profile,
        0,
        include_examples,
        &mut state,
    );
    let root = TocNode {
        name: "$".to_string(),
        path: "$".to_string(),
        kind: NodeKind::Array,
        presence: None,
        observed_items: Some(records.len()),
        shape_count: Some(exact_shape_count(&records, profile)),
        children: vec![element],
        examples: Vec::new(),
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

// ---------------------------------------------------------------------------
// Recursive value summarizers
// ---------------------------------------------------------------------------

/// Recursively walk a JSON value tree, building a bounded [`TocNode`] tree.
///
/// Contract:
/// - `depth >= profile.max_depth` → truncate (not error), mark `state.truncated`.
/// - Empty `values` → return a `Null` node (caller should guard before calling).
/// - `include_examples` propagates unchanged through recursion.
/// - `presence` is `Some` only inside sampled arrays where not all parents have
///   this field.
///
/// Called from `analyze_json_value` (single root value) and `analyze_jsonl`
/// (all sampled records as virtual array elements).
#[allow(clippy::too_many_arguments)]
fn summarize_values(
    name: &str,
    path: &str,
    values: &[&Value],
    presence: Option<Presence>,
    profile: BudgetProfile,
    depth: usize,
    include_examples: bool,
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
            examples: Vec::new(),
        };
    }

    let examples = example_values(values, include_examples, profile);

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
            examples,
        };
    }

    match combined_kind(values) {
        NodeKind::Object => summarize_object_values(
            name,
            path,
            values,
            presence,
            profile,
            depth,
            include_examples,
            examples,
            state,
        ),
        NodeKind::Array => summarize_array_values(
            name,
            path,
            values,
            presence,
            profile,
            depth,
            include_examples,
            examples,
            state,
        ),
        kind => TocNode {
            name: name.to_string(),
            path: path.to_string(),
            kind,
            presence,
            observed_items: None,
            shape_count: None,
            children: Vec::new(),
            examples,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn summarize_object_values(
    name: &str,
    path: &str,
    values: &[&Value],
    presence: Option<Presence>,
    profile: BudgetProfile,
    depth: usize,
    include_examples: bool,
    examples: Vec<String>,
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

    let mut fields = field_values
        .into_iter()
        .map(|(key, values)| ObjectFieldValues {
            key,
            values,
            total: objects.len(),
        })
        .collect::<Vec<_>>();

    if let Some(dynamic_field) = compress_dynamic_fields(&fields, path, state) {
        fields = vec![dynamic_field];
    }

    let total_fields = fields.len();
    if total_fields > profile.max_children {
        state.truncate(format!(
            "Child limit {} reached at {path}; omitted {} field(s).",
            profile.max_children,
            total_fields - profile.max_children
        ));
    }

    let children = fields
        .into_iter()
        .take(profile.max_children)
        .map(|field| {
            let child_presence = if field.total > 1 {
                Some(Presence {
                    observed: field.values.len(),
                    total: field.total,
                })
            } else {
                None
            };
            let child_path = if field.key == "{dynamic_key}" {
                format!("{path}.{{dynamic_key}}")
            } else {
                append_path_key(path, &field.key)
            };
            summarize_values(
                &field.key,
                &child_path,
                &field.values,
                child_presence,
                profile,
                depth + 1,
                include_examples,
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
        examples,
    }
}

#[allow(clippy::too_many_arguments)]
fn summarize_array_values(
    name: &str,
    path: &str,
    values: &[&Value],
    presence: Option<Presence>,
    profile: BudgetProfile,
    depth: usize,
    include_examples: bool,
    examples: Vec<String>,
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
            include_examples,
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
        examples,
    }
}

// ---------------------------------------------------------------------------
// Dynamic key compression and example extraction
// ---------------------------------------------------------------------------

/// Heuristic: collapse sibling object keys that look like dynamic map keys
/// (e.g. `user_001`, `user_002`, …) into a single `{dynamic_key}` placeholder.
///
/// Triggers only when:
/// 1. ≥ 4 sibling fields (small maps are unlikely to be dynamic).
/// 2. ≤ 2 distinct structural signatures across all sibling values
///    (dynamic keys usually map to the same shape).
/// 3. Key names look dynamic: at least half contain digits.
///
/// Ref: PRD §8.5 Dynamic Key Compression.
fn compress_dynamic_fields<'a>(
    fields: &[ObjectFieldValues<'a>],
    path: &str,
    state: &mut BuildState,
) -> Option<ObjectFieldValues<'a>> {
    if fields.len() < 4 {
        return None;
    }

    let mut signatures = BTreeSet::new();
    let mut values = Vec::new();
    for field in fields {
        for value in &field.values {
            signatures.insert(structural_signature(value, 3).join("\n"));
            values.push(*value);
        }
    }

    if signatures.len() <= 2 && dynamic_key_names(fields) {
        state.warn(format!(
            "Some sibling keys at {path} were compressed as {{dynamic_key}}."
        ));
        return Some(ObjectFieldValues {
            key: "{dynamic_key}".to_string(),
            values,
            total: fields.len(),
        });
    }

    None
}

/// Return `true` if sibling keys exhibit dynamic naming patterns.
fn dynamic_key_names(fields: &[ObjectFieldValues<'_>]) -> bool {
    let numericish = fields
        .iter()
        .filter(|field| field.key.chars().any(|ch| ch.is_ascii_digit()))
        .count();
    numericish * 2 >= fields.len()
}

/// Extract up to `profile.max_examples` scalar example values.
/// Returns empty when `include_examples` is false.
/// Composite types (objects, arrays) are skipped — only scalars produce examples.
/// Ref: design principle "structure first, values hidden by default".
fn example_values(
    values: &[&Value],
    include_examples: bool,
    profile: BudgetProfile,
) -> Vec<String> {
    if !include_examples {
        return Vec::new();
    }

    values
        .iter()
        .filter_map(|value| example_value(value))
        .take(profile.max_examples)
        .collect()
}

/// Format a single scalar as a displayable example string.
/// Returns `None` for objects/arrays (not useful as inline examples).
fn example_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("null".to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(redact_and_truncate(text)),
        Value::Array(_) | Value::Object(_) => None,
    }
}

/// Redact sensitive-looking strings, then truncate long ones.
///
/// Redaction patterns (case-insensitive substring match):
/// `token`, `secret`, `password`, `apikey`, `api_key`, `@` (email-like).
/// Strategy: over-redact rather than under-redact — false positives are
/// acceptable because examples are optional and agents can read originals.
///
/// Truncation: strings > 32 chars are cut with a `…` suffix.
fn redact_and_truncate(text: &str) -> String {
    const MAX_CHARS: usize = 32;
    let lower = text.to_ascii_lowercase();
    let sensitive = lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("apikey")
        || lower.contains("api_key")
        || text.contains('@');
    if sensitive {
        return "<redacted>".to_string();
    }

    let mut normalized = text.replace('\n', " ");
    if normalized.chars().count() > MAX_CHARS {
        normalized = normalized.chars().take(MAX_CHARS).collect::<String>();
        normalized.push('…');
    }
    format!("\"{normalized}\"")
}

// ---------------------------------------------------------------------------
// JSONL record grouping and discriminator labels
// ---------------------------------------------------------------------------

/// Two-stage JSONL grouping:
/// 1. **Exact shape grouping** — records with identical structural signatures
///    (depth-bounded `$.field:type` feature sets) go into the same group.
/// 2. **Discriminator split** — within each shape group, check if a candidate
///    field (`type`, `kind`, `event_type`, …) cleanly separates sub-groups.
///    If so, split and label as `type=error` etc.
///
/// Overflow groups beyond `profile.max_groups` collapse into a single `other`
/// bucket with a `minor_shapes≈N` shape note.
///
/// Ref: PRD §8.4 JSONL Structural Clustering.
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

    let mut groups = exact_groups
        .into_values()
        .flat_map(|group| split_by_discriminator(records, group))
        .collect::<Vec<_>>();
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
            label: group
                .label
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

fn split_by_discriminator(records: &[JsonlRecord], group: ShapeGroup) -> Vec<LabeledShapeGroup> {
    if let Some((field, values)) = discriminator_values(records, &group) {
        if values.len() > 1 {
            return values
                .into_iter()
                .map(|(value, record_indexes)| LabeledShapeGroup {
                    label: Some(format!("{field}={value}")),
                    first_line: record_indexes
                        .iter()
                        .map(|record_index| records[*record_index].line)
                        .min()
                        .unwrap_or(group.first_line),
                    records: record_indexes,
                    shape: group.shape.clone(),
                })
                .collect();
        }
        if let Some((value, _)) = values.into_iter().next() {
            return vec![LabeledShapeGroup {
                label: Some(format!("{field}={value}")),
                records: group.records,
                first_line: group.first_line,
                shape: group.shape,
            }];
        }
    }

    vec![LabeledShapeGroup {
        label: None,
        records: group.records,
        first_line: group.first_line,
        shape: group.shape,
    }]
}

/// Find a field that explains the structural grouping.
///
/// Logic (from PRD §8.4.4): "先看结构是否形成组；再看某个字段是否能够解释这些组。"
/// We iterate CANDIDATES in priority order. For each, check if ALL records in
/// the shape group have that field with a scalar value. If yes, return the
/// field name and the map of value→record-indexes. The caller uses this to
/// either split a multi-value group or label a single-value group.
///
/// CANDIDATES order = priority: `type` is checked before `event_type`, etc.
fn discriminator_values(
    records: &[JsonlRecord],
    group: &ShapeGroup,
) -> Option<(&'static str, BTreeMap<String, Vec<usize>>)> {
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
        let mut values: BTreeMap<String, Vec<usize>> = BTreeMap::new();
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
            values.entry(label_value).or_default().push(*record_index);
        }

        if all_present && !values.is_empty() {
            return Some((candidate, values));
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
