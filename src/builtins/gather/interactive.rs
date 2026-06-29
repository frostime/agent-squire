use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use super::model::{Prefix, Source};
use super::parser::{command_body_prefix, parse_source, selector_prefix};
use super::sources::{fzf_dirs, fzf_files};

pub struct InteractiveResult {
    pub sources: Vec<Source>,
    pub render: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveCommand {
    Help,
    List,
    Done,
    Exit,
    ToggleAll,
    Zip {
        path: Option<String>,
        and_done: bool,
    },
}

pub fn read_sources(cwd: &Path, respect_gitignore: bool) -> Result<InteractiveResult> {
    let mut sources = Vec::new();
    let mut respect_gitignore = respect_gitignore;
    let stdin = io::stdin();
    println!("Interactive gather. Type /help for commands, /done to render, /exit to quit.");

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

        if line.trim() == "^D" {
            break;
        }

        if let Some(command) = parse_interactive_command(line) {
            match command {
                InteractiveCommand::Help => print_interactive_help(),
                InteractiveCommand::List => print_sources(&sources),
                InteractiveCommand::Done => break,
                InteractiveCommand::Exit => {
                    return Ok(InteractiveResult {
                        sources,
                        render: false,
                    });
                }
                InteractiveCommand::ToggleAll => {
                    respect_gitignore = !respect_gitignore;
                    let mode = if respect_gitignore {
                        "respecting .gitignore"
                    } else {
                        "including gitignored files"
                    };
                    println!("  fzf selection now {mode}");
                }
                InteractiveCommand::Zip { path, and_done } => {
                    let output_path = path.map(PathBuf::from);
                    match super::zip::assemble_zip(
                        &sources,
                        cwd,
                        respect_gitignore,
                        output_path,
                    ) {
                        Ok(Some(_)) => {
                            if and_done {
                                break;
                            }
                        }
                        Ok(None) => {
                            // User cancelled at warning prompt
                        }
                        Err(err) => {
                            eprintln!("error: {err}");
                        }
                    }
                }
            }
            continue;
        }

        if let Some(prefix) = selector_prefix(line) {
            match select_with_fzf(prefix, cwd, respect_gitignore) {
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
            Err(err) => eprintln!("error: {err}. Try /help"),
        }
    }

    Ok(InteractiveResult {
        sources,
        render: true,
    })
}

pub fn parse_interactive_command(line: &str) -> Option<InteractiveCommand> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();

    if let Some(rest) = lower.strip_prefix("/zip") {
        let rest = rest.trim_start();
        if rest.is_empty() {
            return Some(InteractiveCommand::Zip {
                path: None,
                and_done: false,
            });
        }
        let (path_part, and_done) = if rest == "/done" || rest == "--done" {
            ("", true)
        } else if let Some(p) = rest.strip_suffix(" /done") {
            (p.trim(), true)
        } else if let Some(p) = rest.strip_suffix(" --done") {
            (p.trim(), true)
        } else {
            (rest, false)
        };
        let path = if path_part.is_empty() {
            None
        } else {
            Some(path_part.to_string())
        };
        return Some(InteractiveCommand::Zip { path, and_done });
    }

    match lower.as_str() {
        "/help" | "help" | "?" => Some(InteractiveCommand::Help),
        "/list" | "list" => Some(InteractiveCommand::List),
        "/done" | "done" => Some(InteractiveCommand::Done),
        "/exit" | "exit" | "quit" => Some(InteractiveCommand::Exit),
        "/all" | "all" => Some(InteractiveCommand::ToggleAll),
        _ => None,
    }
}

pub fn select_with_fzf(prefix: Prefix, cwd: &Path, respect_gitignore: bool) -> Result<Vec<Source>> {
    let choices = match prefix {
        Prefix::File | Prefix::Glob => fzf_files(cwd, respect_gitignore)?,
        Prefix::Dir | Prefix::Tree => fzf_dirs(cwd, respect_gitignore)?,
        Prefix::Cmd => bail!("cmd: does not use fzf selection"),
    };
    let selected = run_fzf(&choices).context(
        "fzf is required for interactive selection. Enter explicit prefix:path sources instead.",
    )?;

    edit_selected_sources(prefix, selected, cwd)
}

fn edit_selected_sources(
    prefix: Prefix,
    selected: Vec<PathBuf>,
    cwd: &Path,
) -> Result<Vec<Source>> {
    if selected.is_empty() {
        return Ok(Vec::new());
    }

    if prefix == Prefix::Glob {
        return edit_grouped_glob_selection(selected, cwd);
    }

    let total = selected.len();
    let mut sources = Vec::new();
    for (index, path) in selected.into_iter().enumerate() {
        let initial = default_source_line(prefix, &path);
        let prompt = if total == 1 {
            "edit> ".to_string()
        } else {
            format!("edit {}/{}> ", index + 1, total)
        };
        let Some(edited) = read_prefilled_line(&prompt, &initial)? else {
            continue;
        };
        match parse_edited_source(&edited, cwd) {
            Ok(source) => sources.push(source),
            Err(err) => eprintln!("error: {err}. Skipped: {edited}"),
        }
    }
    Ok(sources)
}

fn edit_grouped_glob_selection(selected: Vec<PathBuf>, cwd: &Path) -> Result<Vec<Source>> {
    let initial = "glob:fzf selection";
    let Some(edited) = read_prefilled_line("edit> ", initial)? else {
        return Ok(Vec::new());
    };
    if edited == initial {
        return Ok(vec![Source::SelectedGlob {
            label: "fzf selection".into(),
            files: selected,
        }]);
    }
    Ok(vec![parse_edited_source(&edited, cwd)?])
}

fn read_prefilled_line(prompt: &str, initial: &str) -> Result<Option<String>> {
    let mut editor = DefaultEditor::new().context("failed to initialize line editor")?;
    match editor.readline_with_initial(prompt, (initial, "")) {
        Ok(line) => {
            let line = line.trim().to_string();
            Ok((!line.is_empty()).then_some(line))
        }
        Err(ReadlineError::Eof | ReadlineError::Interrupted) => Ok(None),
        Err(err) => Err(err).context("failed to read edited source line"),
    }
}

fn parse_edited_source(line: &str, cwd: &Path) -> Result<Source> {
    parse_source(line, cwd)
}

fn default_source_line(prefix: Prefix, path: &Path) -> String {
    let path = path.display().to_string().replace('\\', "/");
    match prefix {
        Prefix::File => format!("file:{path}"),
        Prefix::Dir => format!("dir:{path}"),
        Prefix::Tree => format!("tree:{path}"),
        Prefix::Glob => format!("glob:{path}"),
        Prefix::Cmd => format!("cmd:{path}"),
    }
}

#[cfg(test)]
pub fn sources_from_fzf_lines(prefix: Prefix, lines: &str) -> Vec<Source> {
    let selected: Vec<PathBuf> = lines
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();
    sources_from_selected_paths_for_tests(prefix, selected)
}

#[cfg(test)]
fn sources_from_selected_paths_for_tests(prefix: Prefix, selected: Vec<PathBuf>) -> Vec<Source> {
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

fn print_interactive_help() {
    println!(
        "Commands:\n\
           /help        show this help\n\
           /list        show selected sources\n\
           /zip [path]  package sources into a zip\n\
           /done        render selected sources\n\
           /exit        quit without rendering\n\
           /all         toggle gitignored fzf candidates\n\
         Sources:\n\
           file:path\n\
           file:path:start-end\n\
           dir:path\n\
           tree:path\n\
           glob:pattern\n\
           cmd:command\n\
         Selectors:\n\
           file: dir: tree: glob: open fzf; selected paths open edit> for confirmation\n\
           In edit>, press Enter to accept, edit text to add ranges, or clear to skip"
    );
}

fn print_sources(sources: &[Source]) {
    if sources.is_empty() {
        println!("  (no sources)");
        return;
    }
    for (index, source) in sources.iter().enumerate() {
        println!("  {}. {}", index + 1, describe_source(source));
    }
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
    fn parses_internal_commands() {
        assert_eq!(
            parse_interactive_command("/help"),
            Some(InteractiveCommand::Help)
        );
        assert_eq!(
            parse_interactive_command("exit"),
            Some(InteractiveCommand::Exit)
        );
        assert_eq!(
            parse_interactive_command("done"),
            Some(InteractiveCommand::Done)
        );
        assert_eq!(
            parse_interactive_command("/all"),
            Some(InteractiveCommand::ToggleAll)
        );
        assert_eq!(parse_interactive_command("file:"), None);
    }

    #[test]
    fn parses_zip_command() {
        assert_eq!(
            parse_interactive_command("/zip"),
            Some(InteractiveCommand::Zip {
                path: None,
                and_done: false,
            })
        );
        assert_eq!(
            parse_interactive_command("/zip /done"),
            Some(InteractiveCommand::Zip {
                path: None,
                and_done: true,
            })
        );
        assert_eq!(
            parse_interactive_command("/zip --done"),
            Some(InteractiveCommand::Zip {
                path: None,
                and_done: true,
            })
        );
        assert_eq!(
            parse_interactive_command("/zip my-package.zip"),
            Some(InteractiveCommand::Zip {
                path: Some("my-package.zip".into()),
                and_done: false,
            })
        );
        assert_eq!(
            parse_interactive_command("/zip my-package.zip /done"),
            Some(InteractiveCommand::Zip {
                path: Some("my-package.zip".into()),
                and_done: true,
            })
        );
    }


    #[test]
    fn default_source_line_uses_selected_path() {
        assert_eq!(
            default_source_line(Prefix::File, Path::new("src/main.rs")),
            "file:src/main.rs"
        );
        assert_eq!(
            default_source_line(Prefix::Tree, Path::new("src")),
            "tree:src"
        );
    }

    #[test]
    fn edited_file_source_can_add_range() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();

        let source = parse_edited_source("file:src/main.rs:10-20", dir.path()).unwrap();
        assert!(
            matches!(source, Source::File { range: Some(range), .. } if range.start == 10 && range.end == 20)
        );
    }

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
