use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::model::{
    ComposeError, ComposeResult, FailureCase, RenderOptions, ShellMode, SourceSpec,
};
use super::text::{decode_text, limit_bytes};

#[derive(Debug, Clone)]
pub enum ResolvedSource {
    Text(String),
    Exec { stdout: String, stderr: String },
}

#[derive(Default)]
pub struct SourceCache {
    stdin: Option<String>,
}

pub fn resolve_source(
    source: &SourceSpec,
    options: &RenderOptions,
    cache: &mut SourceCache,
    timeout_override: Option<u64>,
) -> ComposeResult<ResolvedSource> {
    match source {
        SourceSpec::Stdin => resolve_stdin(cache),
        SourceSpec::File(path) => resolve_file(path, options),
        SourceSpec::Env(name) => resolve_env(name),
        SourceSpec::Exec(command) => resolve_exec(command, options, timeout_override),
    }
}

// stdin is a process-wide stream, so repeated `${{stdin}}` references share one
// cached read. Interactive stdin fails fast to avoid hanging agent runs.
fn resolve_stdin(cache: &mut SourceCache) -> ComposeResult<ResolvedSource> {
    if let Some(text) = &cache.stdin {
        return Ok(ResolvedSource::Text(text.clone()));
    }

    let mut stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(ComposeError::new(
            "stdin_unavailable",
            Some(FailureCase::Error),
            "stdin: is unavailable because stdin is an interactive terminal",
        ));
    }

    let mut bytes = Vec::new();
    stdin.read_to_end(&mut bytes).map_err(|err| {
        ComposeError::new(
            "stdin_read_failed",
            Some(FailureCase::Error),
            format!("Failed to read stdin: {err}"),
        )
    })?;
    let text = decode_text(&bytes, "stdin")?;
    cache.stdin = Some(text.clone());
    Ok(ResolvedSource::Text(text))
}

fn resolve_file(path: &str, options: &RenderOptions) -> ComposeResult<ResolvedSource> {
    let path_buf = PathBuf::from(path);
    let resolved = if path_buf.is_absolute() {
        path_buf
    } else {
        options.cwd.join(path_buf)
    };

    let bytes = std::fs::read(&resolved).map_err(|err| {
        let case = if err.kind() == std::io::ErrorKind::NotFound {
            FailureCase::NotFound
        } else {
            FailureCase::Error
        };
        ComposeError::new(
            if case == FailureCase::NotFound {
                "source_404"
            } else {
                "file_read_failed"
            },
            Some(case),
            format!("Failed to read file {}: {err}", resolved.display()),
        )
    })?;
    let text = decode_text(&bytes, &resolved.display().to_string())?;
    let (text, _) = limit_bytes(&text, options.max_file_bytes, options.fail_on_truncated)?;
    Ok(ResolvedSource::Text(text))
}

fn resolve_env(name: &str) -> ComposeResult<ResolvedSource> {
    std::env::var(name).map(ResolvedSource::Text).map_err(|_| {
        ComposeError::new(
            "source_404",
            Some(FailureCase::NotFound),
            format!("Environment variable not found: {name}"),
        )
    })
}

// Exec is the main trust boundary: it requires --allow-exec, runs through the
// selected shell, captures both streams, and enforces timeout by polling/kill.
fn resolve_exec(
    command_text: &str,
    options: &RenderOptions,
    timeout_override: Option<u64>,
) -> ComposeResult<ResolvedSource> {
    if !options.allow_exec {
        return Err(ComposeError::new(
            "exec_disabled",
            Some(FailureCase::Error),
            "exec: is disabled; pass --allow-exec to enable command execution",
        ));
    }

    let timeout = Duration::from_secs(timeout_override.unwrap_or(options.timeout_seconds));
    let mut command = shell_command(options.shell, command_text);
    command.current_dir(&options.cwd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|err| {
        ComposeError::new(
            "exec_spawn_failed",
            Some(FailureCase::Error),
            format!("Failed to run command: {err}"),
        )
    })?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ComposeError::new(
                        "exec_timeout",
                        Some(FailureCase::Timeout),
                        format!("Command timed out after {} seconds", timeout.as_secs()),
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => {
                return Err(ComposeError::new(
                    "exec_wait_failed",
                    Some(FailureCase::Error),
                    format!("Failed to wait for command: {err}"),
                ));
            }
        }
    }

    let output = child.wait_with_output().map_err(|err| {
        ComposeError::new(
            "exec_output_failed",
            Some(FailureCase::Error),
            format!("Failed to capture command output: {err}"),
        )
    })?;

    if !output.status.success() {
        return Err(ComposeError::new(
            "exec_nonzero",
            Some(FailureCase::Error),
            format!("Command exited with status {}", output.status),
        ));
    }

    let stdout = decode_text(&output.stdout, "exec stdout")?;
    let stderr = decode_text(&output.stderr, "exec stderr")?;
    let (stdout, _) = limit_bytes(
        &stdout,
        options.max_command_bytes,
        options.fail_on_truncated,
    )?;
    let (stderr, _) = limit_bytes(
        &stderr,
        options.max_command_bytes,
        options.fail_on_truncated,
    )?;
    Ok(ResolvedSource::Exec { stdout, stderr })
}

fn shell_command(shell: ShellMode, command_text: &str) -> Command {
    match shell {
        ShellMode::Auto => auto_shell_command(command_text),
        ShellMode::Sh => shell_with_arg("sh", "-c", command_text),
        ShellMode::Bash => shell_with_arg("bash", "-c", command_text),
        ShellMode::Pwsh => powershell_command("pwsh", command_text),
        ShellMode::Powershell => powershell_command("powershell.exe", command_text),
        ShellMode::Cmd => shell_with_arg("cmd.exe", "/C", command_text),
    }
}

#[cfg(windows)]
fn auto_shell_command(command_text: &str) -> Command {
    if command_available("pwsh") {
        return powershell_command("pwsh", command_text);
    }
    if command_available("powershell.exe") {
        return powershell_command("powershell.exe", command_text);
    }
    shell_with_arg("cmd.exe", "/C", command_text)
}

#[cfg(not(windows))]
fn auto_shell_command(command_text: &str) -> Command {
    shell_with_arg("sh", "-c", command_text)
}

#[cfg(windows)]
fn command_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn shell_with_arg(program: &str, flag: &str, command_text: &str) -> Command {
    let mut command = Command::new(program);
    command.arg(flag).arg(command_text);
    command
}

fn powershell_command(program: &str, command_text: &str) -> Command {
    let mut command = Command::new(program);
    command
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(command_text);
    command
}
