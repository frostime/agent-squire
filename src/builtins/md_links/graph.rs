use std::path::Path;

use super::model::{LinkKind, SourceFile, TargetType};

#[derive(Debug, Clone)]
pub(crate) struct LinkEdge {
    pub source: String,
    pub line_num: usize,
    pub kind: LinkKind,
    pub raw: String,
    pub target_type: TargetType,
    pub resolved: Option<String>,
}

pub(crate) fn analyze_edges(
    source: &SourceFile,
    workspace: &Path,
) -> Result<Vec<LinkEdge>, String> {
    let content = std::fs::read_to_string(&source.path).map_err(|err| err.to_string())?;

    Ok(super::parse::parse_links(&content)
        .into_iter()
        .filter_map(|raw| super::resolve::resolve_link(raw, &source.path, workspace))
        .map(|link| LinkEdge {
            source: source.display_path.clone(),
            line_num: link.line_num,
            kind: link.kind,
            raw: link.raw,
            target_type: link.target_type,
            resolved: link.resolved,
        })
        .collect())
}
