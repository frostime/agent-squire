//! Compact human-readable rendering for `data-toc` output.

use crate::builtins::data_toc::types::*;

pub(crate) fn print_compact(data: &DataTocData, warnings: &[String], budget: Budget) {
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
    if !node.examples.is_empty() {
        label.push_str(&format!(" examples=[{}]", node.examples.join(", ")));
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
