use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use uuid::Uuid;

use super::model::{
    ComposeArtifact, ComposeError, ComposeResult, FailureCase, RenderOptions, ShellMode, SourceSpec,
};
use super::render::RenderDeadline;
use super::text::{decode_text, limit_bytes};

#[derive(Debug, Clone)]
pub struct SourceText {
    pub text: String,
    pub truncated: bool,
    pub artifact: Option<ComposeArtifact>,
}

#[derive(Debug, Clone)]
pub enum ResolvedSource {
    Text(String),
    Exec {
        stdout: Box<SourceText>,
        stderr: Box<SourceText>,
        artifacts: Vec<ComposeArtifact>,
    },
}

pub struct SourceCache {
    stdin: Option<String>,
    spill_budget: Arc<Mutex<SpillBudget>>,
}

impl SourceCache {
    pub fn new(max_spill_bytes: usize) -> Self {
        Self {
            stdin: None,
            spill_budget: Arc::new(Mutex::new(SpillBudget {
                remaining: max_spill_bytes,
                max: max_spill_bytes,
            })),
        }
    }
}

struct SpillBudget {
    remaining: usize,
    max: usize,
}

pub fn resolve_source(
    source: &SourceSpec,
    options: &RenderOptions,
    cache: &mut SourceCache,
    timeout_override: Option<u64>,
    deadline: Option<RenderDeadline>,
    source_index: usize,
) -> ComposeResult<ResolvedSource> {
    match source {
        SourceSpec::Stdin => resolve_stdin(cache),
        SourceSpec::File(path) => resolve_file(path, options),
        SourceSpec::Env(name) => resolve_env(name),
        SourceSpec::Exec(command) => resolve_exec(
            command,
            options,
            cache,
            timeout_override,
            deadline,
            source_index,
        ),
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
// selected shell, drains both streams while the child runs, and enforces timeout.
fn resolve_exec(
    command_text: &str,
    options: &RenderOptions,
    cache: &mut SourceCache,
    timeout_override: Option<u64>,
    deadline: Option<RenderDeadline>,
    source_index: usize,
) -> ComposeResult<ResolvedSource> {
    if !options.allow_exec {
        return Err(ComposeError::new(
            "exec_disabled",
            Some(FailureCase::Error),
            "exec: is disabled; pass --allow-exec to enable command execution",
        ));
    }

    let timeout = effective_timeout(
        timeout_override.unwrap_or(options.timeout_seconds),
        deadline,
    )?;
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

    let stdout = child.stdout.take().expect("stdout configured as piped");
    let stderr = child.stderr.take().expect("stderr configured as piped");
    let stdout_handle = spawn_stream_drain(
        stdout,
        "stdout",
        source_index,
        options.max_command_bytes,
        Arc::clone(&cache.spill_budget),
    );
    let stderr_handle = spawn_stream_drain(
        stderr,
        "stderr",
        source_index,
        options.max_command_bytes,
        Arc::clone(&cache.spill_budget),
    );

    let timed_out = wait_for_child(&mut child, timeout)?;
    let stdout_capture = join_drain(stdout_handle, "stdout")?;
    let stderr_capture = join_drain(stderr_handle, "stderr")?;
    let artifacts = [
        stdout_capture.artifact.clone(),
        stderr_capture.artifact.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if timed_out {
        return Err(ComposeError::new(
            "exec_timeout",
            Some(FailureCase::Timeout),
            format!("Command timed out after {} seconds", timeout.as_secs()),
        )
        .with_artifacts(artifacts));
    }

    let status = child.wait().map_err(|err| {
        ComposeError::new(
            "exec_wait_failed",
            Some(FailureCase::Error),
            format!("Failed to wait for command: {err}"),
        )
    })?;
    if !status.success() {
        return Err(ComposeError::new(
            "exec_nonzero",
            Some(FailureCase::Error),
            format!("Command exited with status {status}"),
        )
        .with_artifacts(artifacts));
    }

    Ok(ResolvedSource::Exec {
        stdout: Box::new(stdout_capture.into_source_text("exec stdout")?),
        stderr: Box::new(stderr_capture.into_source_text("exec stderr")?),
        artifacts,
    })
}

fn effective_timeout(
    local_seconds: u64,
    deadline: Option<RenderDeadline>,
) -> ComposeResult<Duration> {
    let local = Duration::from_secs(local_seconds);
    let Some(deadline) = deadline else {
        return Ok(local);
    };
    let Some(remaining) = deadline.remaining() else {
        return Err(ComposeError::new(
            "total_timeout",
            Some(FailureCase::Timeout),
            "Render total timeout expired",
        ));
    };
    Ok(local.min(remaining))
}

fn wait_for_child(child: &mut Child, timeout: Duration) -> ComposeResult<bool> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(false),
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(true);
                }
                thread::sleep(Duration::from_millis(20));
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
}

trait ExecStream: Read + Send + 'static {}
impl ExecStream for ChildStdout {}
impl ExecStream for ChildStderr {}

fn spawn_stream_drain<R: ExecStream>(
    reader: R,
    stream: &'static str,
    source_index: usize,
    render_limit: usize,
    spill_budget: Arc<Mutex<SpillBudget>>,
) -> thread::JoinHandle<ComposeResult<CapturedStream>> {
    thread::spawn(move || drain_stream(reader, stream, source_index, render_limit, spill_budget))
}

#[derive(Debug)]
struct CapturedStream {
    prefix: Vec<u8>,
    truncated: bool,
    artifact: Option<ComposeArtifact>,
}

impl CapturedStream {
    fn into_source_text(self, label: &str) -> ComposeResult<SourceText> {
        Ok(SourceText {
            text: decode_text(&self.prefix, label)?,
            truncated: self.truncated,
            artifact: self.artifact,
        })
    }
}

fn drain_stream<R: Read>(
    mut reader: R,
    stream: &'static str,
    source_index: usize,
    render_limit: usize,
    spill_budget: Arc<Mutex<SpillBudget>>,
) -> ComposeResult<CapturedStream> {
    let mut prefix = Vec::new();
    let mut spill: Option<SpillState> = None;
    let mut buf = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buf).map_err(|err| {
            ComposeError::new(
                "exec_output_failed",
                Some(FailureCase::Error),
                format!("Failed to read exec {stream}: {err}"),
            )
        })?;
        if read == 0 {
            break;
        }
        let chunk = &buf[..read];
        if prefix.len() < render_limit {
            let keep = (render_limit - prefix.len()).min(chunk.len());
            prefix.extend_from_slice(&chunk[..keep]);
            if keep < chunk.len() {
                ensure_spill(
                    &mut spill,
                    stream,
                    source_index,
                    render_limit,
                    &prefix,
                    &spill_budget,
                )?;
                write_spill(
                    spill.as_mut().expect("spill initialized"),
                    &chunk[keep..],
                    &spill_budget,
                )?;
            }
        } else {
            ensure_spill(
                &mut spill,
                stream,
                source_index,
                render_limit,
                &prefix,
                &spill_budget,
            )?;
            write_spill(
                spill.as_mut().expect("spill initialized"),
                chunk,
                &spill_budget,
            )?;
        }
    }

    let artifact = spill.map(|spill| spill.into_artifact(&spill_budget));
    Ok(CapturedStream {
        prefix,
        truncated: artifact.is_some(),
        artifact,
    })
}

fn ensure_spill(
    spill: &mut Option<SpillState>,
    stream: &'static str,
    source_index: usize,
    render_limit: usize,
    prefix: &[u8],
    budget: &Arc<Mutex<SpillBudget>>,
) -> ComposeResult<()> {
    if spill.is_some() {
        return Ok(());
    }
    let mut state = SpillState::new(stream, source_index, render_limit)?;
    write_spill(&mut state, prefix, budget)?;
    *spill = Some(state);
    Ok(())
}

fn write_spill(
    spill: &mut SpillState,
    chunk: &[u8],
    budget: &Arc<Mutex<SpillBudget>>,
) -> ComposeResult<()> {
    let allowed = {
        let mut budget = budget.lock().expect("spill budget mutex poisoned");
        let allowed = chunk.len().min(budget.remaining);
        budget.remaining -= allowed;
        allowed
    };
    if allowed > 0 {
        spill.write_all(&chunk[..allowed])?;
    }
    if allowed < chunk.len() {
        spill.complete = false;
    }
    Ok(())
}

struct SpillState {
    file: File,
    path: PathBuf,
    stream: &'static str,
    source_index: usize,
    rendered_bytes: usize,
    saved_bytes: usize,
    complete: bool,
}

impl SpillState {
    fn new(
        stream: &'static str,
        source_index: usize,
        rendered_bytes: usize,
    ) -> ComposeResult<Self> {
        let dir = std::env::temp_dir().join("agent-temp");
        fs::create_dir_all(&dir).map_err(|err| {
            ComposeError::new(
                "spill_create_failed",
                Some(FailureCase::Error),
                format!("Failed to create spill directory {}: {err}", dir.display()),
            )
        })?;
        let timestamp = Local::now().format("%Y%m%dT%H%M%S");
        let path = dir.join(format!(
            "asq-compose-spill-{timestamp}-{}.{stream}.txt",
            Uuid::new_v4()
        ));
        let file = File::create(&path).map_err(|err| {
            ComposeError::new(
                "spill_create_failed",
                Some(FailureCase::Error),
                format!("Failed to create spill file {}: {err}", path.display()),
            )
        })?;
        Ok(Self {
            file,
            path,
            stream,
            source_index,
            rendered_bytes,
            saved_bytes: 0,
            complete: true,
        })
    }

    fn write_all(&mut self, bytes: &[u8]) -> ComposeResult<()> {
        self.file.write_all(bytes).map_err(|err| {
            ComposeError::new(
                "spill_write_failed",
                Some(FailureCase::Error),
                format!("Failed to write spill file {}: {err}", self.path.display()),
            )
        })?;
        self.saved_bytes += bytes.len();
        Ok(())
    }

    fn into_artifact(self, budget: &Arc<Mutex<SpillBudget>>) -> ComposeArtifact {
        let max_saved_bytes = budget.lock().expect("spill budget mutex poisoned").max;
        let message = if self.complete {
            format!(
                "{} truncated after {} bytes; full stream saved to {}",
                self.stream,
                self.rendered_bytes,
                self.path.display()
            )
        } else {
            format!(
                "{} truncated after {} bytes; spill capped at {} bytes and saved to {}",
                self.stream,
                self.rendered_bytes,
                max_saved_bytes,
                self.path.display()
            )
        };
        ComposeArtifact {
            kind: "spill".into(),
            path: self.path.display().to_string(),
            source_index: self.source_index,
            source_kind: "exec".into(),
            stream: self.stream.into(),
            rendered_bytes: self.rendered_bytes,
            saved_bytes: self.saved_bytes,
            max_saved_bytes,
            complete: self.complete,
            message,
        }
    }
}

fn join_drain(
    handle: thread::JoinHandle<ComposeResult<CapturedStream>>,
    stream: &str,
) -> ComposeResult<CapturedStream> {
    handle.join().map_err(|_| {
        ComposeError::new(
            "exec_output_failed",
            Some(FailureCase::Error),
            format!("Failed to join exec {stream} reader"),
        )
    })?
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
