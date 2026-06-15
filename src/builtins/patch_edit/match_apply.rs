use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::io::{TextEncoding, atomic_write_text, read_target_text_with_encoding};
use super::model::{PatchApplyOptions, PatchApplyResult, PatchBlock, PatchMatch, PatchOperation};
use super::parse::parse_patches;
use super::text::{
    common_base_indent, convert_newlines, detect_newline_style, is_blank_line, migrate_base_indent,
    norm_line_exact, norm_line_loose, split_lines_keepends, strip_base_indent,
};

#[derive(Debug, Clone)]
struct SmartIndentCandidate {
    start: usize,
    indent_from: String,
    indent_to: String,
}

pub fn apply_patches(
    patch_text: &str,
    project_root: &Path,
    dry_run: bool,
) -> Vec<PatchApplyResult> {
    apply_patches_with_options(
        patch_text,
        project_root,
        PatchApplyOptions {
            dry_run,
            smart_indent: false,
        },
    )
}

pub fn apply_patches_with_options(
    patch_text: &str,
    project_root: &Path,
    options: PatchApplyOptions,
) -> Vec<PatchApplyResult> {
    match parse_patches(patch_text, project_root) {
        Ok(patches) => apply_parsed_patches_with_options(&patches, options),
        Err(errors) => errors
            .into_iter()
            .map(|error| PatchApplyResult {
                patch: None,
                success: false,
                status: "parse_error".into(),
                error: Some(error),
                match_mode: None,
                match_line: None,
                related_lines: None,
                source_line_start: None,
                search_line_count: 0,
                replace_line_count: 0,
                indent_from: None,
                indent_to: None,
            })
            .collect(),
    }
}

pub fn apply_parsed_patches(patches: &[PatchBlock], dry_run: bool) -> Vec<PatchApplyResult> {
    apply_parsed_patches_with_options(
        patches,
        PatchApplyOptions {
            dry_run,
            smart_indent: false,
        },
    )
}

pub fn apply_parsed_patches_with_options(
    patches: &[PatchBlock],
    options: PatchApplyOptions,
) -> Vec<PatchApplyResult> {
    let mut indexed_results: Vec<(usize, PatchApplyResult)> = Vec::new();
    let mut file_search_patches: BTreeMap<std::path::PathBuf, Vec<(usize, PatchBlock)>> =
        BTreeMap::new();

    for (idx, patch) in patches.iter().cloned().enumerate() {
        if patch.operation == PatchOperation::Search {
            file_search_patches
                .entry(patch.file_path.clone())
                .or_default()
                .push((idx, patch));
        } else {
            indexed_results.push((idx, apply_patch(&patch, options)));
        }
    }

    for (_file_path, indexed_searches) in file_search_patches {
        if indexed_searches.len() == 1 {
            let (idx, patch) = &indexed_searches[0];
            indexed_results.push((*idx, apply_patch(patch, options)));
        } else {
            let patches = indexed_searches
                .iter()
                .map(|(_, p)| p.clone())
                .collect::<Vec<_>>();
            let results = apply_search_patches_batch(&patches, options);
            for ((idx, _), result) in indexed_searches.into_iter().zip(results) {
                indexed_results.push((idx, result));
            }
        }
    }

    indexed_results.sort_by_key(|(idx, _)| *idx);
    indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect()
}

fn apply_patch(patch: &PatchBlock, options: PatchApplyOptions) -> PatchApplyResult {
    match apply_patch_inner(patch, options) {
        Ok(result) => result,
        Err(error) => base_result(
            patch,
            false,
            "write_error",
            Some(format!("Failed to apply patch: {error}")),
            0,
            0,
            None,
            None,
        ),
    }
}

fn apply_patch_inner(
    patch: &PatchBlock,
    options: PatchApplyOptions,
) -> anyhow::Result<PatchApplyResult> {
    if patch.operation == PatchOperation::Create {
        let replace_text = convert_newlines(&patch.replace_content, "\n");
        let replace_lines = split_lines_keepends(&replace_text);

        if patch.file_path.exists() {
            if !patch.file_path.is_file() {
                return Ok(base_result(
                    patch,
                    false,
                    "not_a_file",
                    Some(format!("Not a file: {}", patch.display_path)),
                    0,
                    replace_lines.len(),
                    None,
                    None,
                ));
            }

            let (existing, _) = read_target_text_with_encoding(&patch.file_path)?;
            if convert_newlines(&existing, "\n") == replace_text {
                return Ok(base_result(
                    patch,
                    true,
                    "already_applied",
                    Some("CREATE target already exists with identical content".into()),
                    0,
                    replace_lines.len(),
                    None,
                    None,
                ));
            }

            return Ok(base_result(
                patch,
                false,
                "file_exists",
                Some("CREATE target already exists with different content".into()),
                0,
                replace_lines.len(),
                None,
                None,
            ));
        }

        if !options.dry_run {
            if let Some(parent) = patch.file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            atomic_write_text(&patch.file_path, &replace_text, TextEncoding::Utf8)?;
        }

        return Ok(base_result(
            patch,
            true,
            "applied",
            None,
            0,
            replace_lines.len(),
            None,
            None,
        ));
    }

    if !patch.file_path.exists() {
        return Ok(base_result(
            patch,
            false,
            "missing_file",
            Some(format!("File does not exist: {}", patch.display_path)),
            0,
            0,
            None,
            None,
        ));
    }

    if !patch.file_path.is_file() {
        return Ok(base_result(
            patch,
            false,
            "not_a_file",
            Some(format!("Not a file: {}", patch.display_path)),
            0,
            0,
            None,
            None,
        ));
    }

    let (content, encoding) = read_target_text_with_encoding(&patch.file_path)?;
    let newline = detect_newline_style(&content);
    let replace_text = convert_newlines(&patch.replace_content, newline);
    let replace_lines = split_lines_keepends(&replace_text);

    if patch.operation == PatchOperation::Overwrite {
        if content == replace_text {
            return Ok(base_result(
                patch,
                true,
                "no_change_patch",
                Some("OVERWRITE would not change the file".into()),
                0,
                replace_lines.len(),
                None,
                None,
            ));
        }

        if !options.dry_run {
            atomic_write_text(&patch.file_path, &replace_text, encoding)?;
        }

        return Ok(base_result(
            patch,
            true,
            "applied",
            None,
            0,
            replace_lines.len(),
            None,
            None,
        ));
    }

    let file_lines = split_lines_keepends(&content);
    let matched = match_patch(
        patch,
        &file_lines,
        newline,
        content.is_empty(),
        options.smart_indent,
    );

    if matched.status != "matched" {
        return Ok(match_to_result(&matched));
    }

    let replace_lines = adjusted_replace_lines(&matched, newline).unwrap_or_else(|| {
        split_lines_keepends(&convert_newlines(&patch.replace_content, newline))
    });
    let new_content = file_lines[..matched.abs_start]
        .iter()
        .chain(replace_lines.iter())
        .chain(file_lines[matched.abs_end..].iter())
        .cloned()
        .collect::<String>();

    if !options.dry_run {
        atomic_write_text(&patch.file_path, &new_content, encoding)?;
    }

    let mut result = base_result(
        patch,
        true,
        "applied",
        None,
        matched.search_line_count,
        matched.replace_line_count,
        matched.indent_from.clone(),
        matched.indent_to.clone(),
    );
    result.match_mode = matched.match_mode;
    result.match_line = matched.match_line;
    Ok(result)
}

fn apply_search_patches_batch(
    patches: &[PatchBlock],
    options: PatchApplyOptions,
) -> Vec<PatchApplyResult> {
    if patches.is_empty() {
        return vec![];
    }

    let file_path = &patches[0].file_path;

    if !file_path.exists() {
        return patches
            .iter()
            .map(|p| {
                base_result(
                    p,
                    false,
                    "missing_file",
                    Some(format!("File does not exist: {}", p.display_path)),
                    0,
                    0,
                    None,
                    None,
                )
            })
            .collect();
    }

    if !file_path.is_file() {
        return patches
            .iter()
            .map(|p| {
                base_result(
                    p,
                    false,
                    "not_a_file",
                    Some(format!("Not a file: {}", p.display_path)),
                    0,
                    0,
                    None,
                    None,
                )
            })
            .collect();
    }

    let (content, encoding) = match read_target_text_with_encoding(file_path) {
        Ok(v) => v,
        Err(error) => {
            return patches
                .iter()
                .map(|p| {
                    base_result(
                        p,
                        false,
                        "write_error",
                        Some(error.to_string()),
                        0,
                        0,
                        None,
                        None,
                    )
                })
                .collect();
        }
    };

    let newline = detect_newline_style(&content);
    let file_lines = split_lines_keepends(&content);
    let mut matches = patches
        .iter()
        .map(|p| {
            match_patch(
                p,
                &file_lines,
                newline,
                content.is_empty(),
                options.smart_indent,
            )
        })
        .collect::<Vec<_>>();

    if check_overlap(&matches) {
        for matched in &mut matches {
            if matched.status == "matched" {
                matched.status = "overlap_conflict".into();
                matched.error =
                    Some("Overlapping match with another patch in the same batch".into());
            }
        }
    }

    if matches.iter().any(|m| {
        !matches!(
            m.status.as_str(),
            "matched" | "already_applied" | "no_change_patch"
        )
    }) {
        return matches
            .iter()
            .map(|m| {
                if m.status == "matched" {
                    let mut result = base_result(
                        &m.patch,
                        false,
                        "write_error",
                        Some("Not applied: another patch in the same batch failed".into()),
                        m.search_line_count,
                        m.replace_line_count,
                        m.indent_from.clone(),
                        m.indent_to.clone(),
                    );
                    result.match_mode = m.match_mode.clone();
                    result.match_line = m.match_line;
                    result
                } else {
                    match_to_result(m)
                }
            })
            .collect();
    }

    let mut new_lines = file_lines.clone();
    let mut matched = matches
        .iter()
        .filter(|m| m.status == "matched")
        .cloned()
        .collect::<Vec<_>>();
    matched.sort_by_key(|m| std::cmp::Reverse(m.abs_start));

    for m in &matched {
        let replace_lines = adjusted_replace_lines(m, newline).unwrap_or_else(|| {
            split_lines_keepends(&convert_newlines(&m.patch.replace_content, newline))
        });
        new_lines.splice(m.abs_start..m.abs_end, replace_lines);
    }

    if !options.dry_run
        && let Err(error) = atomic_write_text(file_path, &new_lines.concat(), encoding)
    {
        return patches
            .iter()
            .map(|p| {
                base_result(
                    p,
                    false,
                    "write_error",
                    Some(format!("Failed to apply patch: {error}")),
                    0,
                    0,
                    None,
                    None,
                )
            })
            .collect();
    }

    matches
        .iter()
        .map(|m| {
            let status = if m.status == "matched" {
                "applied"
            } else {
                "already_applied"
            };
            let mut result = base_result(
                &m.patch,
                true,
                status,
                m.error.clone(),
                m.search_line_count,
                m.replace_line_count,
                m.indent_from.clone(),
                m.indent_to.clone(),
            );
            result.match_mode = m.match_mode.clone();
            result.match_line = m.match_line;
            result
        })
        .collect()
}

fn match_patch(
    patch: &PatchBlock,
    file_lines: &[String],
    newline: &str,
    is_empty_file: bool,
    smart_indent: bool,
) -> PatchMatch {
    let search_text = convert_newlines(&patch.search_content, newline);
    let search_lines = split_lines_keepends(&search_text);
    let replace_text = convert_newlines(&patch.replace_content, newline);
    let replace_lines = split_lines_keepends(&replace_text);

    let fail = |status: &str,
                error: Option<String>,
                related_lines: Option<Vec<usize>>,
                match_mode: Option<String>,
                match_line: Option<usize>,
                indent_from: Option<String>,
                indent_to: Option<String>| PatchMatch {
        patch: patch.clone(),
        abs_start: 0,
        abs_end: 0,
        match_mode,
        match_line,
        status: status.into(),
        error,
        related_lines,
        search_line_count: search_lines.len(),
        replace_line_count: replace_lines.len(),
        indent_from,
        indent_to,
    };

    let is_empty_search = search_lines.is_empty();

    if is_empty_search && !is_empty_file {
        return fail(
            "parse_error",
            Some("SEARCH content is empty, this is not allowed when the target file is non-empty (ambiguous match)".into()),
            None,
            None,
            None,
            None,
            None,
        );
    }

    if is_empty_search && is_empty_file {
        if replace_text == content_from_file_lines(file_lines) {
            return fail(
                "no_change_patch",
                Some("SEARCH is empty and REPLACE would not change the file".into()),
                None,
                None,
                None,
                None,
                None,
            );
        }

        let mut matched = fail("matched", None, None, None, None, None, None);
        matched.abs_start = 0;
        matched.abs_end = 0;
        return matched;
    }

    if search_text == replace_text {
        return fail(
            "no_change_patch",
            Some("SEARCH and REPLACE are identical".into()),
            None,
            None,
            None,
            None,
            None,
        );
    }

    let (prefix_len, region) = if let Some(range) = patch.line_range {
        let total_lines = file_lines.len();
        let (start, end) = match normalize_line_range(range, total_lines) {
            Ok(v) => v,
            Err(error) => {
                return fail(
                    "invalid_line_range",
                    Some(error),
                    None,
                    None,
                    None,
                    None,
                    None,
                );
            }
        };

        let start_idx = start - 1;
        let end_idx_excl = end;

        if start_idx >= total_lines || end_idx_excl > total_lines {
            return fail(
                "out_of_range",
                Some(format!(
                    "Line range {} is outside file bounds (1-{total_lines})",
                    format_line_range(patch.line_range)
                )),
                None,
                None,
                None,
                None,
                None,
            );
        }

        (start_idx, file_lines[start_idx..end_idx_excl].to_vec())
    } else {
        (0usize, file_lines.to_vec())
    };

    let (search_matches, search_mode) = find_preferred_matches(&region, &search_lines);
    let (replace_matches, replace_mode) = find_preferred_matches(&region, &replace_lines);

    if search_matches.len() == 1 {
        let abs_start = prefix_len + search_matches[0];
        return PatchMatch {
            patch: patch.clone(),
            abs_start,
            abs_end: abs_start + search_lines.len(),
            match_mode: search_mode,
            match_line: Some(abs_start + 1),
            status: "matched".into(),
            error: None,
            related_lines: None,
            search_line_count: search_lines.len(),
            replace_line_count: replace_lines.len(),
            indent_from: None,
            indent_to: None,
        };
    }

    if search_matches.len() > 1 {
        let related = search_matches.iter().map(|m| prefix_len + *m + 1).collect();
        return fail(
            if replace_matches.is_empty() {
                "search_ambiguous"
            } else {
                "search_replace_coexist"
            },
            Some(if replace_matches.is_empty() {
                "SEARCH matched multiple locations; narrow the line range".into()
            } else {
                "SEARCH and REPLACE both exist in scope; narrow the line range".into()
            }),
            Some(related),
            search_mode,
            None,
            None,
            None,
        );
    }

    let smart_candidates = find_smart_indent_candidates(&region, &search_lines);
    if smart_candidates.len() > 1 {
        let related = smart_candidates
            .iter()
            .map(|candidate| prefix_len + candidate.start + 1)
            .collect();
        return fail(
            "search_indent_ambiguous",
            Some(format!(
                "SEARCH matches {} locations after indent migration; narrow the line range",
                smart_candidates.len()
            )),
            Some(related),
            None,
            None,
            None,
            None,
        );
    }

    if let Some(candidate) = smart_candidates.first() {
        let abs_start = prefix_len + candidate.start;
        if !smart_indent {
            return fail(
                "indent_mismatch",
                Some(format!(
                    "SEARCH matches after indent migration ({} -> {}). Use --smart-indent to apply",
                    format_indent(&candidate.indent_from),
                    format_indent(&candidate.indent_to)
                )),
                None,
                None,
                Some(abs_start + 1),
                Some(candidate.indent_from.clone()),
                Some(candidate.indent_to.clone()),
            );
        }

        if migrate_base_indent(&replace_lines, &candidate.indent_from, &candidate.indent_to)
            .is_none()
        {
            return fail(
                "replace_indent_incompatible",
                Some(format!(
                    "REPLACE cannot be migrated from {} to {} because a non-empty line does not start with the source indent",
                    format_indent(&candidate.indent_from),
                    format_indent(&candidate.indent_to)
                )),
                None,
                Some("indent_shift".into()),
                Some(abs_start + 1),
                Some(candidate.indent_from.clone()),
                Some(candidate.indent_to.clone()),
            );
        }

        return PatchMatch {
            patch: patch.clone(),
            abs_start,
            abs_end: abs_start + search_lines.len(),
            match_mode: Some("indent_shift".into()),
            match_line: Some(abs_start + 1),
            status: "matched".into(),
            error: None,
            related_lines: None,
            search_line_count: search_lines.len(),
            replace_line_count: replace_lines.len(),
            indent_from: Some(candidate.indent_from.clone()),
            indent_to: Some(candidate.indent_to.clone()),
        };
    }

    if smart_indent {
        let smart_replace_matches = find_smart_indent_candidates(&region, &replace_lines);
        if smart_replace_matches.len() == 1 {
            let candidate = &smart_replace_matches[0];
            return fail(
                "already_applied",
                Some("SEARCH not found, but adjusted REPLACE already exists".into()),
                None,
                Some("indent_shift".into()),
                Some(prefix_len + candidate.start + 1),
                Some(candidate.indent_from.clone()),
                Some(candidate.indent_to.clone()),
            );
        }

        if smart_replace_matches.len() > 1 {
            let related = smart_replace_matches
                .iter()
                .map(|candidate| prefix_len + candidate.start + 1)
                .collect();
            return fail(
                "replace_ambiguous",
                Some("SEARCH not found, and adjusted REPLACE matched multiple locations".into()),
                Some(related),
                Some("indent_shift".into()),
                None,
                None,
                None,
            );
        }
    }

    if replace_matches.len() == 1 {
        return fail(
            "already_applied",
            Some("SEARCH not found, but REPLACE already exists".into()),
            None,
            replace_mode,
            Some(prefix_len + replace_matches[0] + 1),
            None,
            None,
        );
    }

    if replace_matches.len() > 1 {
        let related = replace_matches
            .iter()
            .map(|m| prefix_len + *m + 1)
            .collect();
        return fail(
            "replace_ambiguous",
            Some("SEARCH not found, and REPLACE matched multiple locations".into()),
            Some(related),
            replace_mode,
            None,
            None,
            None,
        );
    }

    fail(
        "search_not_found",
        Some("SEARCH content not found in scope".into()),
        None,
        None,
        None,
        None,
        None,
    )
}

fn adjusted_replace_lines(matched: &PatchMatch, newline: &str) -> Option<Vec<String>> {
    let replace_lines =
        split_lines_keepends(&convert_newlines(&matched.patch.replace_content, newline));
    if matched.match_mode.as_deref() == Some("indent_shift") {
        migrate_base_indent(
            &replace_lines,
            matched.indent_from.as_deref().unwrap_or_default(),
            matched.indent_to.as_deref().unwrap_or_default(),
        )
    } else {
        Some(replace_lines)
    }
}

fn format_indent(indent: &str) -> String {
    if indent.is_empty() {
        return "0 spaces".into();
    }

    let spaces = indent.chars().filter(|c| *c == ' ').count();
    let tabs = indent.chars().filter(|c| *c == '\t').count();
    match (spaces, tabs) {
        (0, 1) => "1 tab".into(),
        (0, n) => format!("{n} tabs"),
        (1, 0) => "1 space".into(),
        (n, 0) => format!("{n} spaces"),
        _ => format!("{:?}", indent),
    }
}

fn content_from_file_lines(file_lines: &[String]) -> String {
    file_lines.concat()
}

fn normalize_line_range(
    range: (Option<usize>, Option<usize>),
    total_lines: usize,
) -> Result<(usize, usize), String> {
    let start = range.0.unwrap_or(1);
    let end = range.1.unwrap_or(total_lines);
    if start == 0 || end == 0 || end < start {
        return Err(format!(
            "Invalid line range: {}",
            format_line_range(Some(range))
        ));
    }
    Ok((start, end))
}

fn format_line_range(range: Option<(Option<usize>, Option<usize>)>) -> String {
    match range {
        None => "Full file".into(),
        Some((Some(start), Some(end))) => format!("L{start}-L{end}"),
        Some((Some(start), None)) => format!("L{start}-"),
        Some((None, Some(end))) => format!("-L{end}"),
        Some((None, None)) => "Full file".into(),
    }
}

fn find_preferred_matches(region: &[String], needle: &[String]) -> (Vec<usize>, Option<String>) {
    let exact = find_block_matches(region, needle, false);
    if !exact.is_empty() {
        return (exact, Some("exact".into()));
    }

    let loose = find_block_matches(region, needle, true);
    if !loose.is_empty() {
        return (loose, Some("loose".into()));
    }

    (vec![], None)
}

fn find_smart_indent_candidates(region: &[String], needle: &[String]) -> Vec<SmartIndentCandidate> {
    if needle.is_empty() || needle.len() > region.len() {
        return vec![];
    }

    let indent_from = common_base_indent(needle);
    let Some(normalized_needle) = strip_base_indent(needle, &indent_from) else {
        return vec![];
    };

    let mut matches = Vec::new();
    for start in 0..=(region.len() - needle.len()) {
        let window = &region[start..start + needle.len()];
        let indent_to = common_base_indent(window);
        let Some(normalized_window) = strip_base_indent(window, &indent_to) else {
            continue;
        };

        if smart_blocks_equal(&normalized_needle, &normalized_window) {
            matches.push(SmartIndentCandidate {
                start,
                indent_from: indent_from.clone(),
                indent_to,
            });
        }
    }

    matches
}

fn smart_blocks_equal(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            if is_blank_line(a) || is_blank_line(b) {
                is_blank_line(a) && is_blank_line(b)
            } else {
                norm_line_exact(a) == norm_line_exact(b)
            }
        })
}

fn find_block_matches(region: &[String], needle: &[String], loose: bool) -> Vec<usize> {
    if needle.is_empty() || region.len() < needle.len() {
        return vec![];
    }

    let norm = |s: &String| {
        if loose {
            norm_line_loose(s)
        } else {
            norm_line_exact(s)
        }
    };
    let target = needle.iter().map(norm).collect::<Vec<_>>();
    let mut matches = Vec::new();

    for i in 0..=(region.len() - needle.len()) {
        let mut ok = true;
        for j in 0..needle.len() {
            if norm(&region[i + j]) != target[j] {
                ok = false;
                break;
            }
        }
        if ok {
            matches.push(i);
        }
    }

    matches
}

fn check_overlap(matches: &[PatchMatch]) -> bool {
    let matched = matches
        .iter()
        .filter(|m| m.status == "matched")
        .collect::<Vec<_>>();
    for i in 0..matched.len() {
        for j in (i + 1)..matched.len() {
            let a = matched[i];
            let b = matched[j];
            if a.abs_start < b.abs_end && b.abs_start < a.abs_end {
                return true;
            }
        }
    }
    false
}

fn match_to_result(m: &PatchMatch) -> PatchApplyResult {
    let success = matches!(
        m.status.as_str(),
        "matched" | "already_applied" | "no_change_patch"
    );
    let mut result = base_result(
        &m.patch,
        success,
        &m.status,
        m.error.clone(),
        m.search_line_count,
        m.replace_line_count,
        m.indent_from.clone(),
        m.indent_to.clone(),
    );
    result.match_mode = m.match_mode.clone();
    result.match_line = m.match_line;
    result.related_lines = m.related_lines.clone();
    result
}

#[allow(clippy::too_many_arguments)]
fn base_result(
    patch: &PatchBlock,
    success: bool,
    status: &str,
    error: Option<String>,
    search_line_count: usize,
    replace_line_count: usize,
    indent_from: Option<String>,
    indent_to: Option<String>,
) -> PatchApplyResult {
    PatchApplyResult {
        patch: Some(patch.clone()),
        success,
        status: status.into(),
        error,
        match_mode: None,
        match_line: None,
        related_lines: None,
        source_line_start: Some(patch.source_line_start),
        search_line_count,
        replace_line_count,
        indent_from,
        indent_to,
    }
}
