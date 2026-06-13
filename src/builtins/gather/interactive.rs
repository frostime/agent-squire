use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::model::{Prefix, Source};
use super::parser::{command_body_prefix, parse_source, selector_prefix};
use super::sources::{fzf_dirs, fzf_files};

pub fn read_sources(cwd: &std::path::Path) -> Result<Vec<Source>> {
    let mut sources = Vec::new();
    let stdin = io::stdin();

    loop {
        print!("gather> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }

        if let Some(prefix) = selector_prefix(line) {
            match select_with_fzf(prefix, cwd) {
                Ok(selected) => {
                    for source in selected {
                        println!("  ✓ Added: {}", describe_source(&source));
                        sources.push(source);
                    }
                }
                Err(err) => eprintln!("error: {err}"),
            }
            continue;
        }

        let source = if command_body_prefix(line) {
            print!("cmd body> ");
            io::stdout().flush()?;
            let mut body = String::new();
            if stdin.read_line(&mut body)? == 0 {
                break;
            }
            parse_source(&format!("cmd:{}", body.trim_end_matches(['\r', '\n'])), cwd)
        } else {
            parse_source(line, cwd)
        };

        match source {
            Ok(source) => {
                println!("  ✓ Added: {}", describe_source(&source));
                sources.push(source);
            }
            Err(err) => eprintln!("error: {err}"),
        }
    }

    Ok(sources)
}

pub fn select_with_fzf(prefix: Prefix, cwd: &std::path::Path) -> Result<Vec<Source>> {
    let choices = match prefix {
        Prefix::File | Prefix::Glob => fzf_files(cwd)?,
        Prefix::Dir | Prefix::Tree => fzf_dirs(cwd)?,
        Prefix::Cmd => bail!("cmd: does not use fzf selection"),
    };
    let selected = run_fzf(&choices).context(
        "fzf is required for interactive selection. Enter explicit prefix:path sources instead.",
    )?;

    Ok(match prefix {
        Prefix::File => selected
            .into_iter()
            .map(|path| Source::File { path, range: None })
            .collect(),
        Prefix::Dir => selected
            .into_iter()
            .map(|path| Source::Dir { path })
            .collect(),
        Prefix::Tree => selected
            .into_iter()
            .map(|path| Source::Tree { path })
            .collect(),
        Prefix::Glob => vec![Source::SelectedGlob {
            label: "fzf selection".into(),
            files: selected,
        }],
        Prefix::Cmd => unreachable!(),
    })
}

#[cfg(test)]
pub fn sources_from_fzf_lines(prefix: Prefix, lines: &str) -> Vec<Source> {
    let selected: Vec<PathBuf> = lines
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();
    match prefix {
        Prefix::File => selected
            .into_iter()
            .map(|path| Source::File { path, range: None })
            .collect(),
        Prefix::Dir => selected
            .into_iter()
            .map(|path| Source::Dir { path })
            .collect(),
        Prefix::Tree => selected
            .into_iter()
            .map(|path| Source::Tree { path })
            .collect(),
        Prefix::Glob => vec![Source::SelectedGlob {
            label: "fzf selection".into(),
            files: selected,
        }],
        Prefix::Cmd => Vec::new(),
    }
}

fn run_fzf(choices: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut child = Command::new("fzf")
        .arg("--multi")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().expect("stdin configured");
        for choice in choices {
            writeln!(stdin, "{}", choice.display())?;
        }
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8(output.stdout).context("fzf output was not UTF-8")?;
    Ok(text.lines().map(PathBuf::from).collect())
}

fn describe_source(source: &Source) -> String {
    match source {
        Source::File { path, range } => match range {
            Some(range) => format!("file:{}:{}-{}", path.display(), range.start, range.end),
            None => format!("file:{}", path.display()),
        },
        Source::Dir { path } => format!("dir:{}", path.display()),
        Source::Tree { path } => format!("tree:{}", path.display()),
        Source::Glob { pattern } => format!("glob:{pattern}"),
        Source::SelectedGlob { label, files } => format!("glob:{label} ({} files)", files.len()),
        Source::Command { command } => format!("cmd:{command}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fzf_lines_become_file_sources() {
        let sources = sources_from_fzf_lines(Prefix::File, "a.rs\nb.rs\n");
        assert_eq!(sources.len(), 2);
        assert!(matches!(&sources[0], Source::File { path, .. } if path == &PathBuf::from("a.rs")));
    }

    #[test]
    fn fzf_lines_become_grouped_glob_selection() {
        let sources = sources_from_fzf_lines(Prefix::Glob, "a.rs\nb.rs\n");
        assert!(matches!(&sources[0], Source::SelectedGlob { files, .. } if files.len() == 2));
    }
}
