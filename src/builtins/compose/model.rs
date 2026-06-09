use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Template {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub enum Segment {
    Literal(String),
    Interpolation(Interpolation),
}

#[derive(Debug, Clone)]
pub struct Interpolation {
    pub raw: String,
    pub location: Location,
    pub commands: Vec<CommandNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct CommandNode {
    pub name: String,
    pub body: Option<CommandBody>,
}

#[derive(Debug, Clone)]
pub struct CommandBody {
    pub value: String,
    pub quoted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureCase {
    #[serde(rename = "404")]
    NotFound,
    Error,
    Timeout,
    Range,
    Encoding,
    Binary,
    Limit,
    Modifier,
    Parse,
}

impl FailureCase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "404",
            Self::Error => "error",
            Self::Timeout => "timeout",
            Self::Range => "range",
            Self::Encoding => "encoding",
            Self::Binary => "binary",
            Self::Limit => "limit",
            Self::Modifier => "modifier",
            Self::Parse => "parse",
        }
    }

    pub fn from_policy_name(name: &str) -> Option<Self> {
        match name {
            "on-404" => Some(Self::NotFound),
            "on-error" => Some(Self::Error),
            "on-timeout" => Some(Self::Timeout),
            "on-range" => Some(Self::Range),
            "on-binary" => Some(Self::Binary),
            "on-encoding" => Some(Self::Encoding),
            "on-limit" => Some(Self::Limit),
            "on-modifier" => Some(Self::Modifier),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ComposeError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<FailureCase>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

impl ComposeError {
    pub fn new(
        code: impl Into<String>,
        case: Option<FailureCase>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            case,
            message: message.into(),
            raw: None,
            location: None,
        }
    }

    pub fn parse(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, Some(FailureCase::Parse), message)
    }

    pub fn with_interpolation(mut self, interpolation: &Interpolation) -> Self {
        if self.raw.is_none() {
            self.raw = Some(interpolation.raw.clone());
        }
        if self.location.is_none() {
            self.location = Some(interpolation.location.clone());
        }
        self
    }
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ComposeError {}

pub type ComposeResult<T> = Result<T, ComposeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShellMode {
    Auto,
    Sh,
    Bash,
    Pwsh,
    Powershell,
    Cmd,
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub cwd: PathBuf,
    pub allow_exec: bool,
    pub shell: ShellMode,
    pub timeout_seconds: u64,
    pub total_timeout_seconds: Option<u64>,
    pub max_lines: Option<usize>,
    pub max_bytes: Option<usize>,
    pub max_file_bytes: usize,
    pub max_command_bytes: usize,
    pub fail_on_truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub index: usize,
    pub kind: String,
    pub argument: String,
    pub location: Location,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComposeStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputInfo>,
    pub bytes: usize,
    pub sources: usize,
    pub truncated: bool,
}
