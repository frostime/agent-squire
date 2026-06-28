use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn squire() -> Command {
    Command::cargo_bin("squire").unwrap()
}

/// RFC case 1: single-file move, dry-run leaves file untouched, --yes applies.
#[test]
fn move_dry_run_does_not_write_then_yes_applies() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "1\n2\n3\n4\n5\n").unwrap();
    let spec = "file a.md\nmove 1-2 to after 4";

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin(spec)
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"))
        .stdout(predicate::str::contains("No file written"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "1\n2\n3\n4\n5\n");

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(spec)
        .assert()
        .success()
        .stdout(predicate::str::contains("modified"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "3\n4\n1\n2\n5\n");
}

/// RFC case 2: contiguous chunks reorder.
#[test]
fn rearrange_contiguous_chunks() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nB\nC\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "file a.md\nchunk A = 1-1\nchunk B = 2-2\nchunk C = 3-3\nrearrange A, B, C => C, A, B",
        )
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&file).unwrap(), "C\nA\nB\n");
}

/// RFC case 3: gap=slot keeps hidden lines pinned between slots.
#[test]
fn rearrange_gap_slot_keeps_hidden() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nh1\nB\nC\nh2\nD\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "file a.md\nchunk A = 1-1\nchunk B = 3-3\nchunk C = 4-4\nchunk D = 6-6\nrearrange A, B, C, D => B, D, C, A",
        )
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&file).unwrap(), "B\nh1\nD\nC\nh2\nA\n");
}

/// RFC case 4: gap=drop discards hidden lines and reports them.
#[test]
fn rearrange_gap_drop_discards_hidden() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nh1\nh2\nB\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("file a.md\nchunk A = 1-1\nchunk B = 4-4\nrearrange A, B => B, A gap=drop")
        .assert()
        .success()
        .stdout(predicate::str::contains("dropped 2-3"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "B\nA\n");
}

/// RFC case 5: overlapping chunks fail with a structured code, no write.
#[test]
fn overlapping_chunks_fail_without_writing() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "1\n2\n3\n4\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("file a.md\nchunk A = 1-2\nchunk B = 2-3\nrearrange A, B => B, A")
        .assert()
        .failure()
        .stderr(predicate::str::contains("OVERLAPPING_CHUNKS"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "1\n2\n3\n4\n");
}

/// BC-3: CRLF newline style is preserved on write.
#[test]
fn crlf_newline_preserved() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "1\r\n2\r\n3\r\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("file a.md\nmove 1-1 to after 3")
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&file).unwrap(), "2\r\n3\r\n1\r\n");
}

/// BC-1/BC-4: more than one action is rejected.
#[test]
fn multiple_actions_rejected() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "1\n2\n3\n4\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("file a.md\ndelete 1-1\ndelete 2-2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("MULTIPLE_ACTIONS"));
}

/// BC-4: missing target file reports FILE_NOT_FOUND.
#[test]
fn missing_file_reports_not_found() {
    let dir = tempdir().unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("file nope.md\ndelete 1-1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("FILE_NOT_FOUND"));
}

/// BC-5: JSON output uses the standard envelope with a diff field.
#[test]
fn json_output_envelope() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "1\n2\n3\n").unwrap();

    let out = squire()
        .current_dir(dir.path())
        .args(["--json", "rearrange", "--stdin"])
        .write_stdin("file a.md\nmove 1-1 to end")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "rearrange");
    assert_eq!(json["data"]["written"], false);
    assert!(
        json["data"]["diff"]
            .as_str()
            .unwrap()
            .contains("--- a/a.md")
    );
}

#[test]
fn prompt_prints_guide() {
    squire()
        .args(["rearrange", "--prompt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Squire rearrange format"));
}
