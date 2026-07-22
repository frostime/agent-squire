use anyhow::Result;

use crate::runtime::output::{self, Envelope, PrintMode};

use super::model::{MdLinksData, TargetType};

pub fn print(data: MdLinksData, warnings: Vec<String>, cwd: String, mode: PrintMode) -> Result<()> {
    match mode {
        PrintMode::Json => {
            let payload = Envelope::new("md-links", data)
                .with_warnings(warnings)
                .with_meta(serde_json::json!({ "cwd": cwd }));
            output::print_json(&payload)?;
        }
        _ => print_compact(&data, &warnings),
    }
    Ok(())
}

fn print_compact(data: &MdLinksData, warnings: &[String]) {
    println!(
        "# files={} links={} file_links={} existing_file_links={} missing_file_links={}",
        data.count,
        data.total_links,
        data.total_file_links,
        data.total_existing_file_links,
        data.total_file_links - data.total_existing_file_links
    );

    for warning in warnings {
        println!("! {warning}");
    }

    for file in &data.files {
        let file_links = file
            .links
            .iter()
            .filter(|link| link.target_type == TargetType::File)
            .count();
        let existing_file_links = file
            .links
            .iter()
            .filter(|link| link.target_type == TargetType::File && link.exists == Some(true))
            .count();
        println!(
            "@ {} links={} file_links={} missing_file_links={}",
            file.path,
            file.links.len(),
            file_links,
            file_links - existing_file_links
        );

        if let Some(error) = &file.error {
            println!("! file_error={}", json_string(error));
            continue;
        }

        for link in &file.links {
            println!(
                "L{}|{}|{}|{}|{}",
                link.line_num,
                status_name(&link.target_type, link.exists),
                kind_name(&link.kind),
                target_type_name(&link.target_type),
                target_display(&link.raw, link.resolved.as_deref())
            );
        }
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

fn status_name(target_type: &TargetType, exists: Option<bool>) -> &'static str {
    match target_type {
        TargetType::File => {
            if exists == Some(true) {
                "ok"
            } else {
                "missing"
            }
        }
        TargetType::Url => "url",
        TargetType::SiyuanBlock => "siyuan_block",
        TargetType::Unknown => "unknown",
    }
}

fn target_display(raw: &str, resolved: Option<&str>) -> String {
    match resolved {
        Some(resolved) if resolved != raw => {
            format!("{}=>{}", json_string(raw), json_string(resolved))
        }
        _ => json_string(raw),
    }
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
