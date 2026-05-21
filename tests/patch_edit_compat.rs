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

    let results = apply_patches(patch, dir.path(), false);
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert_eq!(results[0].status, "applied");
    assert_eq!(results[0].match_mode.as_deref(), Some("exact"));
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "hi\nworld\n");
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

    let results = apply_patches(patch, dir.path(), true);
    assert!(results[0].success);
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "hello\n");
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

    let results = apply_patches(patch, dir.path(), false);
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

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(results[0].match_mode.as_deref(), Some("loose"));
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "beta\n");
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

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(results[0].match_line, Some(4));
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "x\nsame\nx\nother\n");
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

    let results = apply_patches(patch, dir.path(), false);
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

    let results = apply_patches(patch, dir.path(), false);
    assert!(results[0].success);
    assert_eq!(fs::read_to_string(dir.path().join("new.txt")).unwrap(), "hello\n");

    let again = apply_patches(patch, dir.path(), false);
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

    let results = apply_patches(patch, dir.path(), false);
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

    let results = apply_patches(patch, dir.path(), false);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.success));
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "ONE\nTWO\nthree\n");
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

    let results = apply_patches(patch, dir.path(), false);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| !r.success));
    assert!(results.iter().all(|r| r.status == "overlap_conflict"));
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "a\nb\nc\n");
}
