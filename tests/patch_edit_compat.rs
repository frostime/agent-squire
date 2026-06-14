use std::fs;

use agent_squire::builtins::patch_edit::{
    PatchApplyOptions, apply_patches, apply_patches_with_options,
};
use tempfile::tempdir;

fn smart() -> PatchApplyOptions {
    PatchApplyOptions {
        dry_run: false,
        smart_indent: true,
    }
}

#[test]
fn applies_exact_search_patch() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello\nworld\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "hello\n",
        "=======\n",
        "hi\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert_eq!(results[0].status, "applied");
    assert_eq!(results[0].match_mode.as_deref(), Some("exact"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "hi\nworld\n"
    );
}

#[test]
fn dry_run_does_not_write() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "hello\n",
        "=======\n",
        "hi\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), true);
    assert!(results[0].success);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "hello\n"
    );
}

#[test]
fn detects_already_applied() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hi\nworld\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "hello\n",
        "=======\n",
        "hi\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(results[0].status, "already_applied");
    assert_eq!(results[0].match_line, Some(1));
}

#[test]
fn loose_match_ignores_trailing_spaces() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha   \n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "alpha\n",
        "=======\n",
        "beta\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(results[0].match_mode.as_deref(), Some("loose"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "beta\n"
    );
}

#[test]
fn line_range_limits_scope() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "x\nsame\nx\nsame\n").unwrap();

    let patch = concat!(
        "# a.txt:L4-L4\n",
        "<<<<<<< SEARCH\n",
        "same\n",
        "=======\n",
        "other\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(results[0].match_line, Some(4));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "x\nsame\nx\nother\n"
    );
}

#[test]
fn ambiguous_search_fails() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "same\nx\nsame\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "same\n",
        "=======\n",
        "other\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_ambiguous");
    assert_eq!(results[0].related_lines.as_ref().unwrap(), &vec![1, 3]);
}

#[test]
fn create_new_file_and_detect_existing_identical() {
    let dir = tempdir().unwrap();

    let patch = concat!(
        "# new.txt\n",
        "<<<<<<< CREATE\n",
        "=======\n",
        "hello\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(
        fs::read_to_string(dir.path().join("new.txt")).unwrap(),
        "hello\n"
    );

    let again = apply_patches(patch, dir.path(), false);
    assert!(again[0].success);
    assert_eq!(again[0].status, "already_applied");
}

#[test]
fn overwrite_no_change() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< OVERWRITE\n",
        "=======\n",
        "hello\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(results[0].status, "no_change_patch");
}

#[test]
fn same_file_batch_uses_original_content_then_splices_backwards() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "one\n",
        "=======\n",
        "ONE\n",
        ">>>>>>> REPLACE\n\n",
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "two\n",
        "=======\n",
        "TWO\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.success));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "ONE\nTWO\nthree\n"
    );
}

#[test]
fn same_file_overlap_conflict_rejects_all_overlapping_matches() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\nb\nc\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "a\n",
        "b\n",
        "=======\n",
        "A\n",
        "B\n",
        ">>>>>>> REPLACE\n\n",
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "b\n",
        "c\n",
        "=======\n",
        "B\n",
        "C\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| !r.success));
    assert!(results.iter().all(|r| r.status == "overlap_conflict"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "a\nb\nc\n"
    );
}

#[test]
fn smart_indent_reports_unique_candidate_without_flag() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.txt"),
        "    fn foo() {\n        content\n    }\n",
    )
    .unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "fn foo() {\n",
        "    content\n",
        "}\n",
        "=======\n",
        "fn bar() {\n",
        "    new_content\n",
        "}\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches(patch, dir.path(), false);
    assert!(!results[0].success);
    assert_eq!(results[0].status, "indent_mismatch");
    assert_eq!(results[0].indent_from.as_deref(), Some(""));
    assert_eq!(results[0].indent_to.as_deref(), Some("    "));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "    fn foo() {\n        content\n    }\n"
    );
}

#[test]
fn smart_indent_applies_missing_outer_indent() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.txt"),
        "    fn foo() {\n        content\n    }\n",
    )
    .unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "fn foo() {\n",
        "    content\n",
        "}\n",
        "=======\n",
        "fn bar() {\n",
        "    new_content\n",
        "}\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches_with_options(patch, dir.path(), smart());
    assert!(results[0].success);
    assert_eq!(results[0].status, "applied");
    assert_eq!(results[0].match_mode.as_deref(), Some("indent_shift"));
    assert_eq!(results[0].indent_from.as_deref(), Some(""));
    assert_eq!(results[0].indent_to.as_deref(), Some("    "));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "    fn bar() {\n        new_content\n    }\n"
    );
}

#[test]
fn smart_indent_can_reduce_base_indent() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "fn foo() {\n    content\n}\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "        fn foo() {\n",
        "            content\n",
        "        }\n",
        "=======\n",
        "        fn bar() {\n",
        "            new_content\n",
        "        }\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches_with_options(patch, dir.path(), smart());
    assert!(results[0].success, "{:?}", results[0].error);
    assert_eq!(results[0].indent_from.as_deref(), Some("        "));
    assert_eq!(results[0].indent_to.as_deref(), Some(""));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "fn bar() {\n    new_content\n}\n"
    );
}

#[test]
fn smart_indent_preserves_deep_yaml_relative_indent() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.yml"),
        "root:\n          key:\n            child: old\n",
    )
    .unwrap();

    let patch = concat!(
        "# a.yml\n",
        "<<<<<<< SEARCH\n",
        "      key:\n",
        "        child: old\n",
        "=======\n",
        "      key:\n",
        "        child: new\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches_with_options(patch, dir.path(), smart());
    assert!(results[0].success, "{:?}", results[0].error);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.yml")).unwrap(),
        "root:\n          key:\n            child: new\n"
    );
}

#[test]
fn smart_indent_blank_search_line_does_not_match_non_blank_target_line() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.txt"),
        "    fn foo() {\n    unexpected\n    }\n",
    )
    .unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "fn foo() {\n",
        "\n",
        "}\n",
        "=======\n",
        "fn bar() {\n",
        "\n",
        "}\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches_with_options(patch, dir.path(), smart());
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_not_found");
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "    fn foo() {\n    unexpected\n    }\n"
    );
}

#[test]
fn smart_indent_multiple_candidates_are_ambiguous() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.txt"),
        "    fn foo() {\n    }\n        fn foo() {\n        }\n",
    )
    .unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "fn foo() {\n",
        "}\n",
        "=======\n",
        "fn bar() {\n",
        "}\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches_with_options(patch, dir.path(), smart());
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_indent_ambiguous");
    assert_eq!(results[0].related_lines.as_ref().unwrap(), &vec![1, 3]);
}

#[test]
fn smart_indent_rejects_incompatible_replace_indent() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "fn foo() {\n    content\n}\n").unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "    fn foo() {\n",
        "        content\n",
        "    }\n",
        "=======\n",
        "fn bar() {\n",
        "    new_content\n",
        "}\n",
        ">>>>>>> REPLACE\n",
    );

    let results = apply_patches_with_options(patch, dir.path(), smart());
    assert!(!results[0].success);
    assert_eq!(results[0].status, "replace_indent_incompatible");
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "fn foo() {\n    content\n}\n"
    );
}

#[test]
fn smart_indent_is_idempotent() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("a.txt"),
        "    fn foo() {\n        content\n    }\n",
    )
    .unwrap();

    let patch = concat!(
        "# a.txt\n",
        "<<<<<<< SEARCH\n",
        "fn foo() {\n",
        "    content\n",
        "}\n",
        "=======\n",
        "fn bar() {\n",
        "    new_content\n",
        "}\n",
        ">>>>>>> REPLACE\n",
    );

    let first = apply_patches_with_options(patch, dir.path(), smart());
    assert_eq!(first[0].status, "applied");

    let second = apply_patches_with_options(patch, dir.path(), smart());
    assert!(second[0].success);
    assert_eq!(second[0].status, "already_applied");
    assert_eq!(second[0].match_mode.as_deref(), Some("indent_shift"));
}
