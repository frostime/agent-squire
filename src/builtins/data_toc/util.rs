//! Small, stateless helpers used by `data-toc` analysis and rendering.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::builtins::data_toc::types::*;

// ---------------------------------------------------------------------------
// Path and key helpers
// ---------------------------------------------------------------------------

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn budget_name(profile: BudgetProfile) -> &'static str {
    match profile.max_json_bytes {
        bytes if bytes == Budget::Small.profile().max_json_bytes => "small",
        bytes if bytes == Budget::Normal.profile().max_json_bytes => "normal",
        _ => "large",
    }
}

pub(crate) fn combined_kind(values: &[&Value]) -> NodeKind {
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

// ---------------------------------------------------------------------------
// Structural signatures and shape counting
// ---------------------------------------------------------------------------

pub(crate) fn append_path_key(path: &str, key: &str) -> String {
    if is_simple_key(key) {
        format!("{path}.{key}")
    } else {
        let escaped = key.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{path}[\"{escaped}\"]")
    }
}

pub(crate) fn is_simple_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

pub(crate) fn shape_count(values: &[&Value], max_depth: usize) -> usize {
    values
        .iter()
        .map(|value| structural_signature(value, max_depth).join("\n"))
        .collect::<BTreeSet<_>>()
        .len()
}

pub(crate) fn exact_shape_count(records: &[JsonlRecord], profile: BudgetProfile) -> usize {
    records
        .iter()
        .map(|record| structural_signature(&record.value, profile.max_signature_depth).join("\n"))
        .collect::<BTreeSet<_>>()
        .len()
}

/// Build a depth-bounded structural feature set for a JSON value.
///
/// Each feature is `$.path:type` (e.g. `$.error.code:number`).
/// The signature is sorted, making it order-independent for comparison.
/// Array items are sampled up to 3 elements to avoid deep arrays dominating
/// the signature.
///
/// Used for:
/// - JSONL exact shape grouping (`group_jsonl_records`)
/// - Shape count estimation (`shape_count`, `exact_shape_count`)
/// - Dynamic key compression heuristic (`compress_dynamic_fields`)
///
/// Ref: PRD §8.4.1 Per-row structural features.
pub(crate) fn structural_signature(value: &Value, max_depth: usize) -> Vec<String> {
    let mut signature = Vec::new();
    collect_signature(value, "$", 0, max_depth, &mut signature);
    signature.sort();
    signature
}

pub(crate) fn collect_signature(
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

pub(crate) fn shape_summary(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let keys = object.keys().cloned().collect::<Vec<_>>().join(",");
            format!("object{{{keys}}}")
        }
        Value::Array(_) => "array".to_string(),
        _ => NodeKind::from_value(value).compact_name().to_string(),
    }
}

pub(crate) fn count_nodes(node: &TocNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

pub(crate) fn default_notes() -> Vec<String> {
    vec![
        "Output is based on bounded structural scanning.".to_string(),
        "`?` means not present in all observed samples.".to_string(),
        "Array indexes are collapsed into [].".to_string(),
    ]
}

pub(crate) fn suggested_reads_for_json(
    path: &Path,
    root: &TocNode,
    format: DataFormat,
) -> Vec<String> {
    if format == DataFormat::Yaml {
        return Vec::new();
    }

    let Some(array_node) = find_best_array_node(root) else {
        return vec![format!("jq '.' {}", display_path(path))];
    };
    let jq_path = array_node.path.trim_start_matches('$');
    let mut reads = vec![format!("jq '{jq_path}[0:5]' {}", display_path(path))];

    if let Some(projection) = suggested_projection(array_node) {
        reads.push(format!(
            "jq '{jq_path}[0:20] | map({projection})' {}",
            display_path(path)
        ));
    }

    reads
}

pub(crate) fn find_best_array_node(node: &TocNode) -> Option<&TocNode> {
    let mut best = if node.kind == NodeKind::Array && node.path != "$" {
        Some(node)
    } else {
        None
    };
    for child in &node.children {
        if let Some(candidate) = find_best_array_node(child) {
            if best.and_then(|node| node.observed_items).unwrap_or(0)
                < candidate.observed_items.unwrap_or(0)
            {
                best = Some(candidate);
            }
        }
    }
    best
}

pub(crate) fn suggested_projection(array_node: &TocNode) -> Option<String> {
    let element = array_node.children.first()?;
    if element.kind != NodeKind::Object {
        return None;
    }
    let fields = element
        .children
        .iter()
        .filter(|child| child.name != "{dynamic_key}")
        .filter(|child| {
            child
                .presence
                .as_ref()
                .is_none_or(|presence| presence.observed > 0)
        })
        .take(4)
        .map(|child| child.name.as_str())
        .collect::<Vec<_>>();
    if fields.is_empty() {
        None
    } else {
        Some(format!("{{{}}}", fields.join(", ")))
    }
}
