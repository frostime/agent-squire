use std::path::{Path, PathBuf};

use anyhow::Result;

use super::model::{LineRange, Source};
use super::sources::{expand_dir, expand_glob, render_tree};

pub fn generate_template(
    sources: &[Source],
    cwd: &Path,
    respect_gitignore: bool,
) -> Result<(String, bool)> {
    let mut out = String::new();
    let mut requires_exec = false;

    for (index, source) in sources.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n");
        }
        let (segment, exec) = generate_segment(source, cwd, respect_gitignore)?;
        requires_exec |= exec;
        out.push_str(&segment);
    }

    Ok((out, requires_exec))
}

fn generate_segment(
    source: &Source,
    cwd: &Path,
    respect_gitignore: bool,
) -> Result<(String, bool)> {
    match source {
        Source::File { path, range } => Ok((file_block(path, *range), false)),
        Source::Dir { path } => Ok((
            group_block(
                "DIR",
                &display_path(path),
                expand_dir(cwd, path, respect_gitignore)?,
            ),
            false,
        )),
        Source::Tree { path } => Ok((
            literal_block(
                "TREE",
                &display_path(path),
                &render_tree(cwd, path, respect_gitignore)?,
            ),
            false,
        )),
        Source::Glob { pattern } => Ok((
            group_block("GLOB", pattern, expand_glob(cwd, pattern)?),
            false,
        )),
        Source::SelectedGlob { label, files } => {
            Ok((group_block("GLOB", label, files.clone()), false))
        }
        Source::Command { command } => Ok((
            format!(
                "====== CMD-START: {} ======\n${{{{exec: {}}}}}\n====== CMD-END ======",
                command,
                quote_compose_body(command)
            ),
            true,
        )),
    }
}

fn group_block(kind: &str, label: &str, files: Vec<PathBuf>) -> String {
    let mut out = String::new();
    out.push_str(&format!("====== {kind}-START: {label} ======\n"));
    out.push_str("Matched files:\n");
    if files.is_empty() {
        out.push_str("(none)\n");
    } else {
        for file in &files {
            out.push_str("- ");
            out.push_str(&display_path(file));
            out.push('\n');
        }
        for file in files {
            out.push('\n');
            out.push_str(&group_file_block(kind, &file));
            out.push('\n');
        }
    }
    out.push_str(&format!("====== {kind}-END ======"));
    out
}

fn group_file_block(kind: &str, path: &Path) -> String {
    format!(
        "====== {kind}-FILE-START: {} ======\n${{{{file: {}}}}}\n====== {kind}-FILE-END ======",
        display_path(path),
        quote_compose_body(&display_path(path))
    )
}

fn file_block(path: &Path, range: Option<LineRange>) -> String {
    let label = match range {
        Some(range) => format!("{}:{}-{}", display_path(path), range.start, range.end),
        None => display_path(path),
    };
    let interpolation = match range {
        Some(range) => format!(
            "${{{{file: {} |> lines: {}-{}}}}}",
            quote_compose_body(&display_path(path)),
            range.start,
            range.end
        ),
        None => format!("${{{{file: {}}}}}", quote_compose_body(&display_path(path))),
    };
    format!("====== FILE-START: {label} ======\n{interpolation}\n====== FILE-END ======")
}

fn literal_block(kind: &str, label: &str, text: &str) -> String {
    format!("====== {kind}-START: {label} ======\n{text}====== {kind}-END ======")
}

fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

pub fn quote_compose_body(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn file_range_uses_compose_lines_transform_and_quotes_body() {
        let source = Source::File {
            path: PathBuf::from("weird}}name.rs"),
            range: Some(LineRange { start: 2, end: 4 }),
        };
        let (template, exec) = generate_template(&[source], Path::new("."), true).unwrap();
        assert!(!exec);
        assert!(template.contains(r#"${{file: "weird}}name.rs" |> lines: 2-4}}"#));
    }

    #[test]
    fn dir_group_contains_manifest_and_file_blocks() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "a").unwrap();

        let source = Source::Dir {
            path: PathBuf::from("src"),
        };
        let (template, _) = generate_template(&[source], dir.path(), true).unwrap();
        assert!(template.contains("====== DIR-START: src ======"));
        assert!(template.contains("Matched files:\n- src/a.rs"));
        assert!(template.contains("====== DIR-FILE-START: src/a.rs ======"));
        assert!(template.contains("====== DIR-FILE-END ======"));
        assert!(!template.contains("====== FILE-START: src/a.rs ======"));
    }

    #[test]
    fn command_requires_exec_and_quotes_body() {
        let source = Source::Command {
            command: "printf 'a |> b'".into(),
        };
        let (template, exec) = generate_template(&[source], Path::new("."), true).unwrap();
        assert!(exec);
        assert!(template.contains(r#"${{exec: "printf 'a |> b'"}}"#));
    }
}
