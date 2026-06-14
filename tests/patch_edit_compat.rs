use std::fs;

use agent_squire::builtins::patch_edit::apply_patches;
use tempfile::tempdir;

#[test]
fn applies_exact_search_patch() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello\nworld\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
hello
=======
hi
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
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

    let patch = r#"# a.txt
<<<<<<< SEARCH
hello
=======
hi
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), true, false);
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

    let patch = r#"# a.txt
<<<<<<< SEARCH
hello
=======
hi
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(results[0].success);
    assert_eq!(results[0].status, "already_applied");
    assert_eq!(results[0].match_line, Some(1));
}

#[test]
fn loose_match_ignores_trailing_spaces() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha   \n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
alpha
=======
beta
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
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

    let patch = r#"# a.txt:L4-L4
<<<<<<< SEARCH
same
=======
other
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
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

    let patch = r#"# a.txt
<<<<<<< SEARCH
same
=======
other
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_ambiguous");
    assert_eq!(results[0].related_lines.as_ref().unwrap(), &vec![1, 3]);
}

#[test]
fn create_new_file_and_detect_existing_identical() {
    let dir = tempdir().unwrap();

    let patch = r#"# new.txt
<<<<<<< CREATE
=======
hello
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(results[0].success);
    assert_eq!(
        fs::read_to_string(dir.path().join("new.txt")).unwrap(),
        "hello\n"
    );

    let again = apply_patches(patch, dir.path(), false, false);
    assert!(again[0].success);
    assert_eq!(again[0].status, "already_applied");
}

#[test]
fn overwrite_no_change() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< OVERWRITE
=======
hello
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(results[0].success);
    assert_eq!(results[0].status, "no_change_patch");
}

#[test]
fn same_file_batch_uses_original_content_then_splices_backwards() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
one
=======
ONE
>>>>>>> REPLACE

# a.txt
<<<<<<< SEARCH
two
=======
TWO
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
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

    let patch = r#"# a.txt
<<<<<<< SEARCH
a
b
=======
A
B
>>>>>>> REPLACE

# a.txt
<<<<<<< SEARCH
b
c
=======
B
C
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| !r.success));
    assert!(results.iter().all(|r| r.status == "overlap_conflict"));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "a\nb\nc\n"
    );
}

// --- Smart indent tests ---

#[test]
fn indent_mismatch_without_flag() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "    fn foo() {\n        content\n    }\n").unwrap();

    // Search block missing the 4-space indent
    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {
    content
}
=======
fn bar() {
    new_content
}
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(!results[0].success);
    assert_eq!(results[0].status, "indent_mismatch");
    assert_eq!(results[0].indent_delta.as_deref(), Some("    "));
    // File unchanged
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "    fn foo() {\n        content\n    }\n"
    );
}

#[test]
fn smart_indent_applies_with_adjustment() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "    fn foo() {\n        content\n    }\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {
    content
}
=======
fn bar() {
    new_content
}
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, true);
    assert!(results[0].success);
    assert_eq!(results[0].status, "applied");
    assert_eq!(results[0].match_mode.as_deref(), Some("indent_shift"));
    assert_eq!(results[0].match_line, Some(1));
    assert_eq!(results[0].indent_delta.as_deref(), Some("    "));
    // Replace lines get the 4-space indent prepended
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "    fn bar() {\n        new_content\n    }\n"
    );
}

#[test]
fn smart_indent_empty_lines_preserved() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "    fn foo() {\n\n    }\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {

}
=======
fn bar() {

}
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, true);
    assert!(results[0].success);
    assert_eq!(results[0].match_mode.as_deref(), Some("indent_shift"));
    // Empty lines in search should match empty lines in file,
    // and empty lines in replace should NOT get indent added
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "    fn bar() {\n\n    }\n"
    );
}

#[test]
fn smart_indent_tab_prefix() {
    let dir = tempdir().unwrap();
    // All lines have a single tab prefix relative to search content
    fs::write(dir.path().join("a.txt"), "\tfn foo() {\n\tbar\n\t}\n").unwrap();

    // Search without any leading tabs — indent_shift should detect tab delta
    let patch = "# a.txt\n<<<<<<< SEARCH\nfn foo() {\nbar\n}\n=======\nfn baz() {\nqux\n}\n>>>>>>> REPLACE\n";

    let results = apply_patches(patch, dir.path(), false, true);
    assert!(results[0].success, "expected success, got: {:?}", results[0].error);
    assert_eq!(results[0].match_mode.as_deref(), Some("indent_shift"));
    // Tab prefix applied to replace lines
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "\tfn baz() {\n\tqux\n\t}\n"
    );
}

#[test]
fn search_indent_ambiguous() {
    let dir = tempdir().unwrap();
    // File has two identical blocks at same indent level
    fs::write(dir.path().join("a.txt"), "    fn foo() {\n    }\n    fn foo() {\n    }\n").unwrap();

    // Search missing indent: "fn foo() {\n}" matches both blocks with "    " delta
    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {
}
=======
bar
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_indent_ambiguous");
}

#[test]
fn smart_indent_no_delta_needed() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "fn foo() {\n    content\n}\n").unwrap();

    // Search already has correct indentation
    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {
    content
}
=======
fn bar() {
    new_content
}
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, true);
    // Should use exact match, not indent_shift
    assert!(results[0].success);
    assert_eq!(results[0].match_mode.as_deref(), Some("exact"));
    assert_eq!(results[0].indent_delta, None);
}

#[test]
fn smart_indent_mixed_whitespace_no_common() {
    let dir = tempdir().unwrap();
    // File has tab+space mixed indent across lines that can't have a consistent delta
    fs::write(dir.path().join("a.txt"), "\tfn foo() {\n        bar\n\t}\n").unwrap();

    // Search without any indent — the file has tab on line 1 but spaces on line 2
    // After adding tab delta: \tfn foo() matches \tfn foo() but \tbar doesn't match \t\tbar (need 2 tabs)
    // After adding space delta: spaces+fn doesn't match \tfn (tab vs space)
    // So no consistent delta exists
    let patch = "# a.txt\n<<<<<<< SEARCH\nfn foo() {\nbar\n}\n=======\nbaz\n>>>>>>> REPLACE\n";

    let results = apply_patches(patch, dir.path(), false, true);
    // No consistent delta can make all lines match
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_not_found");
}

#[test]
fn search_indent_ambiguous_with_flag() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "    fn foo() {\n    }\n    fn foo() {\n    }\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {
}
=======
bar
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, false);
    assert!(!results[0].success);
    assert_eq!(results[0].status, "search_indent_ambiguous");
}

#[test]
fn smart_indent_preserves_existing_match_priority() {
    let dir = tempdir().unwrap();
    // When exact match exists, should not use indent_shift
    fs::write(dir.path().join("a.txt"), "fn foo() {\n    x\n}\n    fn foo() {\n        y\n    }\n").unwrap();

    let patch = r#"# a.txt
<<<<<<< SEARCH
fn foo() {
    x
}
=======
fn bar() {
    z
}
>>>>>>> REPLACE
"#;

    let results = apply_patches(patch, dir.path(), false, true);
    assert!(results[0].success);
    // Should use exact, not indent_shift
    assert_eq!(results[0].match_mode.as_deref(), Some("exact"));
}