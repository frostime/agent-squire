//! Core vocabulary for `rearrange`: the parsed spec and the planner's output.
//!
//! All line numbers exposed here are 1-based and inclusive, matching the DSL
//! the user writes. The planner converts to 0-based indices internally.

use std::path::PathBuf;

/// A fully parsed spec: one target file, optional named chunks, one action.
///
/// SPEC: v1 permits exactly one action per invocation and one target file.
#[derive(Debug)]
pub struct Spec {
    pub file: PathBuf,
    pub chunks: Vec<ChunkDef>,
    pub action: Action,
}

/// A named line range declared via `chunk <name> = <range>`.
#[derive(Debug)]
pub struct ChunkDef {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub enum Action {
    Move {
        src: Region,
        to: Anchor,
    },
    Copy {
        src: Region,
        to: Anchor,
    },
    Delete {
        src: Region,
    },
    Rearrange {
        from: Vec<String>,
        to: Vec<String>,
        gap: Gap,
    },
}

/// The source of a move/copy/delete: either an inline range or a chunk name.
#[derive(Debug)]
pub enum Region {
    Inline { start: usize, end: usize },
    Named(String),
}

/// An insertion point, expressed against the original file (1-based).
#[derive(Debug)]
pub enum Anchor {
    Start,
    End,
    Before(usize),
    After(usize),
}

/// How `rearrange` treats lines that sit between declared chunk slots.
#[derive(Debug)]
pub enum Gap {
    /// Keep gaps pinned in their original inter-slot positions (default).
    Slot,
    /// Discard all inter-slot gaps.
    Drop,
    /// Fail if any non-empty gap exists between slots.
    Error,
}

/// Structured failure codes surfaced to agents via JSON `meta.error_code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidSpec,
    MultipleActions,
    UnknownChunk,
    InvalidRange,
    RangeOutOfBounds,
    OverlappingChunks,
    AnchorOutOfBounds,
    AnchorInsideMovedChunk,
    RearrangeSetMismatch,
    NonEmptyGap,
    FileNotFound,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSpec => "INVALID_SPEC",
            Self::MultipleActions => "MULTIPLE_ACTIONS",
            Self::UnknownChunk => "UNKNOWN_CHUNK",
            Self::InvalidRange => "INVALID_RANGE",
            Self::RangeOutOfBounds => "RANGE_OUT_OF_BOUNDS",
            Self::OverlappingChunks => "OVERLAPPING_CHUNKS",
            Self::AnchorOutOfBounds => "ANCHOR_OUT_OF_BOUNDS",
            Self::AnchorInsideMovedChunk => "ANCHOR_INSIDE_MOVED_CHUNK",
            Self::RearrangeSetMismatch => "REARRANGE_SET_MISMATCH",
            Self::NonEmptyGap => "NON_EMPTY_GAP",
            Self::FileNotFound => "FILE_NOT_FOUND",
        }
    }
}

/// A coded failure. `message` is human-facing; `code` drives JSON output.
#[derive(Debug)]
pub struct RearrangeError {
    pub code: ErrorCode,
    pub message: String,
}

impl RearrangeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RearrangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for RearrangeError {}

pub type Result<T> = std::result::Result<T, RearrangeError>;
