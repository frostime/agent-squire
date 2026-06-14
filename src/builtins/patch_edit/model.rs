use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperation {
    Search,
    Create,
    Overwrite,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PatchApplyOptions {
    pub dry_run: bool,
    pub smart_indent: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchBlock {
    #[serde(skip_serializing)]
    pub file_path: PathBuf,
    pub display_path: String,
    pub operation: PatchOperation,
    pub line_range: Option<(Option<usize>, Option<usize>)>,
    pub search_content: String,
    pub replace_content: String,
    pub source_line_start: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchApplyResult {
    pub patch: Option<PatchBlock>,
    pub success: bool,
    pub status: String,
    pub error: Option<String>,
    pub match_mode: Option<String>,
    pub match_line: Option<usize>,
    pub related_lines: Option<Vec<usize>>,
    pub source_line_start: Option<usize>,
    pub search_line_count: usize,
    pub replace_line_count: usize,
    pub indent_from: Option<String>,
    pub indent_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PatchMatch {
    pub patch: PatchBlock,
    pub abs_start: usize,
    pub abs_end: usize,
    pub match_mode: Option<String>,
    pub match_line: Option<usize>,
    pub status: String,
    pub error: Option<String>,
    pub related_lines: Option<Vec<usize>>,
    pub search_line_count: usize,
    pub replace_line_count: usize,
    pub indent_from: Option<String>,
    pub indent_to: Option<String>,
}
