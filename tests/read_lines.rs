use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn compact_output_reads_inclusive_line_range_with_one_based_numbers() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "alpha\nbeta\ngamma\ndelta\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["lines", "sample.txt", "-s", "1-3"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "sample.txt | utf-8 lf | 4 lines | 1-based",
        ))
        .stdout(predicate::str::contains("@@ 1-3 requested=1-3"))
        .stdout(predicate::str::contains("  1 | alpha"))
        .stdout(predicate::str::contains("  2 | beta"))
        .stdout(predicate::str::contains("  3 | gamma"));
}

#[test]
fn context_slice_reads_neighboring_lines_when_available() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "1\n2\n3\n4\n5\n6\n7\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["read-lines", "sample.txt", "--slice", "5~2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@@ 3-7 requested=5~2"))
        .stdout(predicate::str::contains("  3 | 3"))
        .stdout(predicate::str::contains("  7 | 7"));
}

#[test]
fn start_and_end_resolve_to_file_boundaries() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "first\nsecond\nthird").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["lines", "sample.txt", "-s", "start-2", "-s", "end"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("@@ 1-2 requested=start-2"));
    assert!(stdout.contains("  1 | first"));
    assert!(stdout.contains("  2 | second"));
    assert!(stdout.contains("@@ 3-3 requested=end"));
    assert!(stdout.contains("  3 | third"));
}

#[test]
fn multiple_slices_preserve_request_order_and_duplicates() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["lines", "sample.txt", "-s", "3", "-s", "1-2", "-s", "3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    let first = stdout.find("@@ 3-3 requested=3").unwrap();
    let second = stdout.find("@@ 1-2 requested=1-2").unwrap();
    let duplicate = stdout.rfind("@@ 3-3 requested=3").unwrap();
    assert!(first < second);
    assert!(second < duplicate);
}

#[test]
fn fully_out_of_bounds_slice_falls_back_to_last_line_with_warning() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["lines", "sample.txt", "-s", "100-120"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: slice 100-120 clipped to 3-3",
        ))
        .stdout(predicate::str::contains("@@ 3-3 requested=100-120"))
        .stdout(predicate::str::contains("  3 | c"));
}

#[test]
fn invalid_slice_fails_without_output_body() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["lines", "sample.txt", "-s", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid slice: 0"))
        .stdout(predicate::str::is_empty());
}

#[test]
fn json_output_uses_standard_envelope() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "lines", "sample.txt", "-s", "2-3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "read-lines");
    assert_eq!(json["data"]["file"]["path"], "sample.txt");
    assert_eq!(json["data"]["file"]["encoding"], "utf-8");
    assert_eq!(json["data"]["file"]["newline"], "lf");
    assert_eq!(json["data"]["file"]["line_count"], 4);
    assert_eq!(json["data"]["slices"][0]["request"], "2-3");
    assert_eq!(json["data"]["slices"][0]["start_line"], 2);
    assert_eq!(json["data"]["slices"][0]["end_line"], 3);
    assert_eq!(json["data"]["slices"][0]["content"], "b\nc");
    assert_eq!(json["warnings"].as_array().unwrap().len(), 0);
}
