use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Markdown,
    Image,
    Wiki,
    CodeSpan,
    Angle,
    SiyuanBlock,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    Url,
    File,
    SiyuanBlock,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct RawLink {
    pub line_num: usize,
    pub kind: LinkKind,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MdLink {
    pub line_num: usize,
    pub kind: LinkKind,
    pub raw: String,
    pub target_type: TargetType,
    pub resolved: Option<String>,
    pub exists: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MdLinksFile {
    pub path: String,
    pub links: Vec<MdLink>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MdLinksData {
    pub files: Vec<MdLinksFile>,
    pub count: usize,
    pub total_links: usize,
    pub total_file_links: usize,
    pub total_existing_file_links: usize,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub display_path: String,
}
