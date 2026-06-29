//! Planner for the `rearrange` state-transition DSL.
//!
//! The planner owns all semantic invariants: path identity, pre-state snapshot,
//! file-state validation, material provenance, and final target bytes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::builtins::rearrange::ast::{
    AfterItem, ArrangeAst, BeforeItem, FileState, RangeExpr, ShareAst, SpecAst,
};
use crate::builtins::rearrange::error::{ErrorCode, RearrangeError, Result};
use crate::builtins::rearrange::parser;
use crate::builtins::rearrange::path::{PathResolver, ResolvedPath, reject_prefix_conflicts};
use crate::builtins::rearrange::textio::{self, TextFile};

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub changed: bool,
    pub shares: Vec<SharePreview>,
    pub targets: Vec<TargetPreview>,
    #[serde(skip)]
    edits: Vec<TargetEdit>,
}

#[derive(Debug, Serialize)]
pub struct SharePreview {
    pub slug: String,
    pub path: String,
    pub items: Vec<ItemPreview>,
}

#[derive(Debug, Serialize)]
pub struct ItemPreview {
    pub name: String,
    pub range: String,
    pub lines: usize,
}

#[derive(Debug, Serialize)]
pub struct TargetPreview {
    pub path: String,
    pub slug: Option<String>,
    pub before: String,
    pub after: String,
    pub exports: Vec<String>,
    pub gaps: Vec<GapPreview>,
    pub effects: Vec<String>,
    pub changed: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct GapPreview {
    pub name: String,
    pub range: String,
    pub lines: usize,
}

#[derive(Debug)]
struct TargetEdit {
    path: PathBuf,
    action: EditAction,
    changed: bool,
}

#[derive(Debug)]
enum EditAction {
    Delete,
    Write(Vec<u8>),
}

#[derive(Debug)]
struct ResolvedSpec {
    shares: Vec<ResolvedShare>,
    arranges: Vec<ResolvedArrange>,
}

#[derive(Debug)]
struct ResolvedShare {
    ast: ShareAst,
    path: ResolvedPath,
}

#[derive(Debug)]
struct ResolvedArrange {
    ast: ArrangeAst,
    path: ResolvedPath,
}

#[derive(Debug, Clone)]
struct Material {
    lines: Vec<String>,
}

#[derive(Debug)]
struct LocalMaterials {
    names: HashMap<String, Material>,
    anonymous: HashMap<String, Material>,
    gaps: HashMap<String, Material>,
    exports: Vec<String>,
    gap_previews: Vec<GapPreview>,
}

#[derive(Debug)]
struct Snapshot {
    files: HashMap<String, Option<TextFile>>,
}

type ExportMap = HashMap<(String, String), Material>;

pub fn execute(spec_text: &str, cwd: &Path, write: bool) -> Result<Outcome> {
    let ast = parser::parse(spec_text)?;
    let resolved = resolve_spec(ast, cwd)?;
    let snapshot = read_snapshot(&resolved)?;
    let mut outcome = build_outcome(&resolved, &snapshot)?;

    if write {
        apply(&outcome)?;
        for target in &mut outcome.targets {
            if target.changed {
                target.effects.push("written".into());
            }
        }
    }

    Ok(outcome)
}

fn resolve_spec(ast: SpecAst, cwd: &Path) -> Result<ResolvedSpec> {
    let resolver = PathResolver::new(cwd)?;
    reject_duplicate_slugs(&ast)?;

    let mut share_paths = HashMap::new();
    let mut arrange_paths = HashMap::new();
    let mut shares = Vec::new();
    let mut arranges = Vec::new();

    for share in ast.shares {
        let path = resolver.resolve(&share.path)?;
        if share_paths
            .insert(path.key.clone(), path.display.clone())
            .is_some()
        {
            return Err(err_at(
                ErrorCode::DuplicatePath,
                share.line,
                format!("duplicate share path: {}", share.path),
            ));
        }
        shares.push(ResolvedShare { ast: share, path });
    }

    for arrange in ast.arranges {
        let path = resolver.resolve(&arrange.path)?;
        if arrange_paths
            .insert(path.key.clone(), path.display.clone())
            .is_some()
        {
            return Err(err_at(
                ErrorCode::DuplicatePath,
                arrange.line,
                format!("duplicate arrange path: {}", arrange.path),
            ));
        }
        if share_paths.contains_key(&path.key) {
            return Err(err_at(
                ErrorCode::DuplicatePath,
                arrange.line,
                format!("path cannot be both share and arrange: {}", arrange.path),
            ));
        }
        arranges.push(ResolvedArrange { ast: arrange, path });
    }

    let arrange_paths = arranges
        .iter()
        .map(|arrange| arrange.path.clone())
        .collect::<Vec<_>>();
    reject_prefix_conflicts(&arrange_paths)?;

    Ok(ResolvedSpec { shares, arranges })
}

fn reject_duplicate_slugs(ast: &SpecAst) -> Result<()> {
    let mut slugs = HashSet::new();
    for share in &ast.shares {
        if !slugs.insert(share.slug.clone()) {
            return Err(err_at(
                ErrorCode::DuplicateSlug,
                share.line,
                format!("duplicate slug: {}", share.slug),
            ));
        }
    }
    for arrange in &ast.arranges {
        if let Some(slug) = &arrange.slug
            && !slugs.insert(slug.clone())
        {
            return Err(err_at(
                ErrorCode::DuplicateSlug,
                arrange.line,
                format!("duplicate slug: {slug}"),
            ));
        }
    }
    Ok(())
}

fn read_snapshot(spec: &ResolvedSpec) -> Result<Snapshot> {
    let mut files = HashMap::new();

    for share in &spec.shares {
        if !share.path.abs.is_file() {
            return Err(err_at(
                ErrorCode::FileNotFound,
                share.ast.line,
                format!("share file not found: {}", share.path.display),
            ));
        }
        let file = textio::read_file(&share.path.abs)
            .map_err(|e| err(ErrorCode::IoError, e.to_string()))?;
        files.insert(share.path.key.clone(), Some(file));
    }

    for arrange in &spec.arranges {
        let file = if arrange.path.abs.is_file() {
            Some(
                textio::read_file(&arrange.path.abs)
                    .map_err(|e| err(ErrorCode::IoError, e.to_string()))?,
            )
        } else {
            None
        };
        files.insert(arrange.path.key.clone(), file);
    }

    Ok(Snapshot { files })
}

fn build_outcome(spec: &ResolvedSpec, snapshot: &Snapshot) -> Result<Outcome> {
    let (share_previews, mut exports) = validate_shares(spec, snapshot)?;
    let mut locals = Vec::new();

    for (idx, arrange) in spec.arranges.iter().enumerate() {
        let local = validate_before(arrange, snapshot)?;
        if let Some(slug) = &arrange.ast.slug {
            for (name, material) in &local.names {
                exports.insert((slug.clone(), name.clone()), material.clone());
            }
        }
        locals.push((idx, local));
    }

    let local_by_idx = locals.into_iter().collect::<HashMap<_, _>>();
    let mut targets = Vec::new();
    let mut edits = Vec::new();

    for (idx, arrange) in spec.arranges.iter().enumerate() {
        let local = local_by_idx
            .get(&idx)
            .expect("local materials built for each arrange");
        let (target, edit) = materialize_target(arrange, snapshot, local, &exports)?;
        targets.push(target);
        edits.push(edit);
    }

    let changed = edits.iter().any(|edit| edit.changed);
    Ok(Outcome {
        changed,
        shares: share_previews,
        targets,
        edits,
    })
}

fn validate_shares(
    spec: &ResolvedSpec,
    snapshot: &Snapshot,
) -> Result<(Vec<SharePreview>, ExportMap)> {
    let mut previews = Vec::new();
    let mut exports = HashMap::new();

    for share in &spec.shares {
        let file = snapshot
            .files
            .get(&share.path.key)
            .and_then(Option::as_ref)
            .expect("share file checked in snapshot");
        let mut names = HashSet::new();
        let mut ranges = Vec::new();
        let mut items = Vec::new();

        for item in &share.ast.items {
            if !names.insert(item.name.clone()) {
                return Err(err_at(
                    ErrorCode::DuplicateName,
                    item.line,
                    format!("duplicate share item: {}", item.name),
                ));
            }
            let (start, end) = resolve_range(&item.range, file.lines.len(), item.line)?;
            ranges.push((start, end, item.line));
            let lines = file.lines[start - 1..end].to_vec();
            exports.insert(
                (share.ast.slug.clone(), item.name.clone()),
                Material {
                    lines: lines.clone(),
                },
            );
            items.push(ItemPreview {
                name: item.name.clone(),
                range: item.range.raw.clone(),
                lines: lines.len(),
            });
        }

        ranges.sort_by_key(|(start, _, _)| *start);
        for pair in ranges.windows(2) {
            if pair[1].0 <= pair[0].1 {
                return Err(err_at(
                    ErrorCode::InvalidRange,
                    pair[1].2,
                    "share ranges must not overlap",
                ));
            }
        }

        previews.push(SharePreview {
            slug: share.ast.slug.clone(),
            path: share.path.display.clone(),
            items,
        });
    }

    Ok((previews, exports))
}

fn validate_before(arrange: &ResolvedArrange, snapshot: &Snapshot) -> Result<LocalMaterials> {
    let file = snapshot
        .files
        .get(&arrange.path.key)
        .and_then(Option::as_ref);
    match &arrange.ast.before {
        FileState::Missing => {
            if file.is_some() {
                return Err(err_at(
                    ErrorCode::InvalidState,
                    arrange.ast.line,
                    format!("before <missing> but file exists: {}", arrange.path.display),
                ));
            }
            Ok(LocalMaterials::empty())
        }
        FileState::Empty => {
            let Some(file) = file else {
                return Err(err_at(
                    ErrorCode::InvalidState,
                    arrange.ast.line,
                    format!(
                        "before <empty> but file is missing: {}",
                        arrange.path.display
                    ),
                ));
            };
            if !file.is_empty_file() {
                return Err(err_at(
                    ErrorCode::InvalidState,
                    arrange.ast.line,
                    "before <empty> but file is not 0 bytes",
                ));
            }
            Ok(LocalMaterials::empty())
        }
        FileState::Sequence(items) => {
            let Some(file) = file else {
                return Err(err_at(
                    ErrorCode::InvalidState,
                    arrange.ast.line,
                    format!(
                        "before sequence but file is missing: {}",
                        arrange.path.display
                    ),
                ));
            };
            if file.is_empty_file() {
                return Err(err_at(
                    ErrorCode::InvalidState,
                    arrange.ast.line,
                    "before sequence but file is empty",
                ));
            }
            validate_before_sequence(items, &file.lines)
        }
    }
}

impl LocalMaterials {
    fn empty() -> Self {
        Self {
            names: HashMap::new(),
            anonymous: HashMap::new(),
            gaps: HashMap::new(),
            exports: Vec::new(),
            gap_previews: Vec::new(),
        }
    }
}

fn validate_before_sequence(items: &[BeforeItem], lines: &[String]) -> Result<LocalMaterials> {
    let mut local = LocalMaterials::empty();
    let mut namespace = HashSet::new();
    let mut prev_end: Option<usize> = None;
    let mut pending_gap: Option<(String, usize)> = None;

    for item in items {
        match item {
            BeforeItem::Gap { name, line } => {
                if prev_end.is_none() || pending_gap.is_some() {
                    return Err(err_at(
                        ErrorCode::InvalidState,
                        *line,
                        "gap must appear between two ranges",
                    ));
                }
                if !namespace.insert(name.clone()) {
                    return Err(err_at(
                        ErrorCode::DuplicateName,
                        *line,
                        format!("duplicate before item: {name}"),
                    ));
                }
                pending_gap = Some((name.clone(), *line));
            }
            BeforeItem::Anonymous { range, line } => {
                let (start, end) = resolve_range(range, lines.len(), *line)?;
                bind_range_boundary(
                    start,
                    end,
                    *line,
                    lines,
                    &mut prev_end,
                    &mut pending_gap,
                    &mut local,
                )?;
                local.anonymous.insert(
                    range.raw.clone(),
                    Material {
                        lines: lines[start - 1..end].to_vec(),
                    },
                );
            }
            BeforeItem::Named { name, range, line } => {
                if !namespace.insert(name.clone()) {
                    return Err(err_at(
                        ErrorCode::DuplicateName,
                        *line,
                        format!("duplicate before item: {name}"),
                    ));
                }
                let (start, end) = resolve_range(range, lines.len(), *line)?;
                bind_range_boundary(
                    start,
                    end,
                    *line,
                    lines,
                    &mut prev_end,
                    &mut pending_gap,
                    &mut local,
                )?;
                local.names.insert(
                    name.clone(),
                    Material {
                        lines: lines[start - 1..end].to_vec(),
                    },
                );
                local.exports.push(name.clone());
            }
        }
    }

    if let Some((_, line)) = pending_gap {
        return Err(err_at(
            ErrorCode::InvalidState,
            line,
            "gap must appear between two ranges",
        ));
    }
    if prev_end != Some(lines.len()) {
        return Err(err(
            ErrorCode::IncompleteCoverage,
            "before sequence must cover through file end",
        ));
    }
    Ok(local)
}

fn bind_range_boundary(
    start: usize,
    end: usize,
    line: usize,
    lines: &[String],
    prev_end: &mut Option<usize>,
    pending_gap: &mut Option<(String, usize)>,
    local: &mut LocalMaterials,
) -> Result<()> {
    if let Some(prev) = *prev_end {
        if start <= prev {
            return Err(err_at(
                ErrorCode::InvalidRange,
                line,
                "before ranges must be strictly ascending and non-overlapping",
            ));
        }
        if let Some((gap_name, gap_line)) = pending_gap.take() {
            if start == prev + 1 {
                return Err(err_at(ErrorCode::EmptyGap, gap_line, "gap is empty"));
            }
            let gap_start = prev + 1;
            let gap_end = start - 1;
            let gap_lines = lines[gap_start - 1..gap_end].to_vec();
            local.gaps.insert(
                gap_name.clone(),
                Material {
                    lines: gap_lines.clone(),
                },
            );
            local.gap_previews.push(GapPreview {
                name: gap_name,
                range: format!("{gap_start}-{gap_end}"),
                lines: gap_lines.len(),
            });
        } else if start > prev + 1 {
            return Err(err_at(
                ErrorCode::UndeclaredGap,
                line,
                format!("hidden gap {}-{} must be declared", prev + 1, start - 1),
            ));
        }
    } else if start != 1 {
        return Err(err_at(
            ErrorCode::IncompleteCoverage,
            line,
            "first before range must start at line 1",
        ));
    }
    *prev_end = Some(end);
    Ok(())
}

fn materialize_target(
    arrange: &ResolvedArrange,
    snapshot: &Snapshot,
    local: &LocalMaterials,
    exports: &ExportMap,
) -> Result<(TargetPreview, TargetEdit)> {
    let original = snapshot
        .files
        .get(&arrange.path.key)
        .and_then(Option::as_ref);
    let before = state_summary_before(&arrange.ast.before);
    let after = state_summary_after(&arrange.ast.after);
    let desired = resolve_after_state(&arrange.ast, local, exports)?;
    let changed = desired_changed(&desired, original);
    let bytes = desired_bytes(&desired, original)?;
    let effects = target_effects(&desired, original, changed);

    let edit = TargetEdit {
        path: arrange.path.abs.clone(),
        action: match bytes {
            Some(bytes) => EditAction::Write(bytes),
            None => EditAction::Delete,
        },
        changed,
    };
    let target = TargetPreview {
        path: arrange.path.display.clone(),
        slug: arrange.ast.slug.clone(),
        before,
        after,
        exports: arrange
            .ast
            .slug
            .as_ref()
            .map(|slug| {
                local
                    .exports
                    .iter()
                    .map(|name| format!("{slug}::{name}"))
                    .collect()
            })
            .unwrap_or_default(),
        gaps: local.gap_previews.clone(),
        effects,
        changed,
    };
    Ok((target, edit))
}

#[derive(Debug)]
enum DesiredState {
    Missing,
    Empty,
    Lines(Vec<String>),
}

fn resolve_after_state(
    arrange: &ArrangeAst,
    local: &LocalMaterials,
    exports: &ExportMap,
) -> Result<DesiredState> {
    match &arrange.after {
        FileState::Missing => {
            if matches!(&arrange.before, FileState::Missing) {
                return Err(err_at(
                    ErrorCode::InvalidState,
                    arrange.line,
                    "before <missing> -> after <missing> is invalid",
                ));
            }
            Ok(DesiredState::Missing)
        }
        FileState::Empty => Ok(DesiredState::Empty),
        FileState::Sequence(items) => {
            let mut lines = Vec::new();
            for item in items {
                let material = resolve_after_item(item, arrange.slug.as_deref(), local, exports)?;
                lines.extend(material.lines.clone());
            }
            Ok(DesiredState::Lines(lines))
        }
    }
}

fn resolve_after_item(
    item: &AfterItem,
    current_slug: Option<&str>,
    local: &LocalMaterials,
    exports: &ExportMap,
) -> Result<Material> {
    match item {
        AfterItem::Anonymous { range, line } => {
            local.anonymous.get(&range.raw).cloned().ok_or_else(|| {
                err_at(
                    ErrorCode::UnknownReference,
                    *line,
                    format!("unknown anonymous range: {}", range.raw),
                )
            })
        }
        AfterItem::Local { name, line } => local.names.get(name).cloned().ok_or_else(|| {
            err_at(
                ErrorCode::UnknownReference,
                *line,
                format!("unknown local material: {name}"),
            )
        }),
        AfterItem::Gap { name, line } => local.gaps.get(name).cloned().ok_or_else(|| {
            err_at(
                ErrorCode::UnknownReference,
                *line,
                format!("unknown gap: {name}"),
            )
        }),
        AfterItem::External { slug, name, line } => {
            if current_slug == Some(slug.as_str()) {
                return Err(err_at(
                    ErrorCode::UnknownReference,
                    *line,
                    "current arrange materials must be referenced locally, not through its slug",
                ));
            }
            exports
                .get(&(slug.clone(), name.clone()))
                .cloned()
                .ok_or_else(|| {
                    err_at(
                        ErrorCode::UnknownReference,
                        *line,
                        format!("unknown external material: {slug}::{name}"),
                    )
                })
        }
    }
}

fn desired_changed(desired: &DesiredState, original: Option<&TextFile>) -> bool {
    match desired {
        DesiredState::Missing => original.is_some(),
        DesiredState::Empty => !original.is_some_and(TextFile::is_empty_file),
        DesiredState::Lines(lines) => original.map(|file| &file.lines != lines).unwrap_or(true),
    }
}

fn desired_bytes(desired: &DesiredState, original: Option<&TextFile>) -> Result<Option<Vec<u8>>> {
    match desired {
        DesiredState::Missing => Ok(None),
        DesiredState::Empty => Ok(Some(Vec::new())),
        DesiredState::Lines(lines) => {
            if let Some(file) = original {
                file.render_existing(lines)
                    .map(Some)
                    .map_err(|e| err(ErrorCode::EncodingError, e))
            } else {
                Ok(Some(textio::render_created(lines)))
            }
        }
    }
}

fn target_effects(
    desired: &DesiredState,
    original: Option<&TextFile>,
    changed: bool,
) -> Vec<String> {
    let effect = match (original.is_some(), desired, changed) {
        (false, DesiredState::Lines(lines), _) => format!("create file ({} lines)", lines.len()),
        (false, DesiredState::Empty, _) => "create empty file".into(),
        (true, DesiredState::Missing, _) => "delete file".into(),
        (true, DesiredState::Empty, true) => "clear file".into(),
        (true, DesiredState::Lines(lines), true) => format!("rewrite file ({} lines)", lines.len()),
        (_, _, false) => "no-op".into(),
        (false, DesiredState::Missing, _) => "invalid".into(),
    };
    vec![effect]
}

fn apply(outcome: &Outcome) -> Result<()> {
    for edit in &outcome.edits {
        if !edit.changed {
            continue;
        }
        match &edit.action {
            EditAction::Delete => textio::delete_file(&edit.path)
                .map_err(|e| err(ErrorCode::IoError, e.to_string()))?,
            EditAction::Write(bytes) => textio::write_file(&edit.path, bytes)
                .map_err(|e| err(ErrorCode::IoError, e.to_string()))?,
        }
    }
    Ok(())
}

fn resolve_range(range: &RangeExpr, line_count: usize, line: usize) -> Result<(usize, usize)> {
    let end = range.resolved_end(line_count);
    if line_count == 0 || range.start > line_count || end > line_count || range.start > end {
        return Err(err_at(
            ErrorCode::RangeOutOfBounds,
            line,
            format!(
                "range {} is outside file bounds (1-{line_count})",
                range.raw
            ),
        ));
    }
    Ok((range.start, end))
}

fn state_summary_before(state: &FileState<BeforeItem>) -> String {
    match state {
        FileState::Missing => "<missing>".into(),
        FileState::Empty => "<empty>".into(),
        FileState::Sequence(items) => items
            .iter()
            .map(describe_before_item)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn state_summary_after(state: &FileState<AfterItem>) -> String {
    match state {
        FileState::Missing => "<missing>".into(),
        FileState::Empty => "<empty>".into(),
        FileState::Sequence(items) => items
            .iter()
            .map(describe_after_item)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn describe_before_item(item: &BeforeItem) -> String {
    match item {
        BeforeItem::Anonymous { range, .. } => range.raw.clone(),
        BeforeItem::Named { name, range, .. } => format!("{name}={}", range.raw),
        BeforeItem::Gap { name, .. } => format!("<gap:{name}>"),
    }
}

fn describe_after_item(item: &AfterItem) -> String {
    match item {
        AfterItem::Anonymous { range, .. } => range.raw.clone(),
        AfterItem::Local { name, .. } => name.clone(),
        AfterItem::Gap { name, .. } => format!("<gap:{name}>"),
        AfterItem::External { slug, name, .. } => format!("{slug}::{name}"),
    }
}

fn err(code: ErrorCode, message: impl Into<String>) -> RearrangeError {
    RearrangeError::new(code, message)
}

fn err_at(code: ErrorCode, line: usize, message: impl Into<String>) -> RearrangeError {
    RearrangeError::at_line(code, line, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn hidden_gap_is_rejected() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "A\nh\nB\n").unwrap();
        let spec = "arrange a.md\n  before A = 1-1, B = 3-end\n  after B, A\nend arrange";
        let err = execute(spec, dir.path(), false).unwrap_err();
        assert_eq!(err.code, ErrorCode::UndeclaredGap);
    }

    #[test]
    fn explicit_gap_can_move() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "A\nh\nB\n").unwrap();
        let spec = "arrange a.md\n  before A = 1-1, <gap:hidden>, B = 3-end\n  after B, <gap:hidden>, A\nend arrange";
        let out = execute(spec, dir.path(), true).unwrap();
        assert!(out.changed);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.md")).unwrap(),
            "B\nh\nA\n"
        );
    }

    #[test]
    fn named_range_cannot_be_referenced_as_bare_range() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "A\n").unwrap();
        let spec = "arrange a.md\n  before A = 1-end\n  after 1-end\nend arrange";
        let err = execute(spec, dir.path(), false).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnknownReference);
    }
}
