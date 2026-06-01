use anyhow::Result;

use crate::runtime::output::{self, Envelope, PrintMode};

use super::model::{MdLinksData, TargetType};

pub fn print(
    data: MdLinksData,
    warnings: Vec<String>,
    workspace: String,
    mode: PrintMode,
) -> Result<()> {
    match mode {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "md-links",
                data,
                warnings,
                meta: serde_json::json!({ "workspace": workspace }),
            };
            output::print_json(&payload)?;
        }
        _ => print_compact(&data),
    }
    Ok(())
}

fn print_compact(data: &MdLinksData) {
    println!(
        "Statistic: files={} links={} file={} exists={}",
        data.count, data.total_links, data.total_file_links, data.total_existing_file_links
    );

    for file in &data.files {
        println!("\n=== Source File {} ===", file.path);
        if let Some(error) = &file.error {
            println!("!{}", json_string(error));
            continue;
        }
        println!("Line(1-based)|ref-kink|target-type|exist|ref-text|resolved");
        if file.links.len() == 0 {
            println!("(No references)")
        }
        for link in &file.links {
            let status = match link.target_type {
                TargetType::File => {
                    if link.exists == Some(true) {
                        "ok"
                    } else {
                        "missing"
                    }
                }
                _ => "-",
            };
            let mut line = format!(
                "L{}|{}|{}|{}|{}",
                link.line_num,
                kind_name(&link.kind),
                target_type_name(&link.target_type),
                status,
                json_string(&link.raw)
            );
            if let Some(resolved) = &link.resolved {
                line.push('|');
                line.push_str(&json_string(resolved));
            }
            println!("{line}");
        }
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

fn kind_name(kind: &super::model::LinkKind) -> &'static str {
    match kind {
        super::model::LinkKind::Markdown => "markdown",
        super::model::LinkKind::Image => "image",
        super::model::LinkKind::Wiki => "wiki",
        super::model::LinkKind::CodeSpan => "code_span",
        super::model::LinkKind::Angle => "angle",
        super::model::LinkKind::SiyuanBlock => "siyuan_block",
    }
}

fn target_type_name(target_type: &TargetType) -> &'static str {
    match target_type {
        TargetType::Url => "url",
        TargetType::File => "file",
        TargetType::SiyuanBlock => "siyuan_block",
        TargetType::Unknown => "unknown",
    }
}
