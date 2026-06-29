#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidSpec,
    InvalidName,
    DuplicateName,
    DuplicateSlug,
    DuplicatePath,
    PathEscapesCwd,
    PathConflict,
    FileNotFound,
    NotAFile,
    InvalidRange,
    RangeOutOfBounds,
    InvalidState,
    IncompleteCoverage,
    UndeclaredGap,
    EmptyGap,
    UnknownReference,
    EncodingError,
    IoError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSpec => "INVALID_SPEC",
            Self::InvalidName => "INVALID_NAME",
            Self::DuplicateName => "DUPLICATE_NAME",
            Self::DuplicateSlug => "DUPLICATE_SLUG",
            Self::DuplicatePath => "DUPLICATE_PATH",
            Self::PathEscapesCwd => "PATH_ESCAPES_CWD",
            Self::PathConflict => "PATH_CONFLICT",
            Self::FileNotFound => "FILE_NOT_FOUND",
            Self::NotAFile => "NOT_A_FILE",
            Self::InvalidRange => "INVALID_RANGE",
            Self::RangeOutOfBounds => "RANGE_OUT_OF_BOUNDS",
            Self::InvalidState => "INVALID_STATE",
            Self::IncompleteCoverage => "INCOMPLETE_COVERAGE",
            Self::UndeclaredGap => "UNDECLARED_GAP",
            Self::EmptyGap => "EMPTY_GAP",
            Self::UnknownReference => "UNKNOWN_REFERENCE",
            Self::EncodingError => "ENCODING_ERROR",
            Self::IoError => "IO_ERROR",
        }
    }
}

#[derive(Debug)]
pub struct RearrangeError {
    pub code: ErrorCode,
    pub message: String,
    pub line: Option<usize>,
}

impl RearrangeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            line: None,
        }
    }

    pub fn at_line(code: ErrorCode, line: usize, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            line: Some(line),
        }
    }
}

impl std::fmt::Display for RearrangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(line) = self.line {
            write!(f, "{} at line {line}: {}", self.code.as_str(), self.message)
        } else {
            write!(f, "{}: {}", self.code.as_str(), self.message)
        }
    }
}

impl std::error::Error for RearrangeError {}

pub type Result<T> = std::result::Result<T, RearrangeError>;
