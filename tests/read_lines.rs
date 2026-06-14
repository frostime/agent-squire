use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

const V: &str = "\u{2502}"; // │

#[test]
fn compact_output_reads_inclusive_line_range_with_one_based_numbers() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "alpha\nbeta\ngamma\ndelta\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "-r", "1-3"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "sample.txt {V} utf-8 lf {V} 4 lines {V} 1-based"
        )))
        .stdout(predicate::str::contains("@@ 1-3 requested=1-3"))
        .stdout(predicate::str::contains(format!("  1 {V} alpha")))
        .stdout(predicate::str::contains(format!("  2 {V} beta")))
        .stdout(predicate::str::contains(format!("  3 {V} gamma")));
}

#[test]
fn context_slice_reads_neighboring_lines_when_available() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "1\n2\n3\n4\n5\n6\n7\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["read-range", "sample.txt", "--range", "5~2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@@ 3-7 requested=5~2"))
        .stdout(predicate::str::contains(format!("  3 {V} 3")))
        .stdout(predicate::str::contains(format!("  7 {V} 7")));
}

#[test]
fn internal_slice_aliases_accept_line_prefix_and_colon_range() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "-r", "L2-L3", "-r", "2:3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("@@ 2-3 requested=L2-L3"));
    assert!(stdout.contains("@@ 2-3 requested=2:3"));
    let sep = format!("  2 {V} b");
    assert_eq!(stdout.matches(&sep).count(), 2);
    let sep = format!("  3 {V} c");
    assert_eq!(stdout.matches(&sep).count(), 2);
}

#[test]
fn internal_slice_aliases_are_hidden_from_help() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["range", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("L10-L50").not())
        .stdout(predicate::str::contains("10:50").not());
}

#[test]
fn start_and_end_resolve_to_file_boundaries() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "first\nsecond\nthird").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "-r", "start-2", "-r", "end"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("@@ 1-2 requested=start-2"));
    assert!(stdout.contains(format!("  1 {V} first").as_str()));
    assert!(stdout.contains(format!("  2 {V} second").as_str()));
    assert!(stdout.contains("@@ 3-3 requested=end"));
    assert!(stdout.contains(format!("  3 {V} third").as_str()));
}

#[test]
fn multiple_slices_preserve_request_order_and_duplicates() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "-r", "3", "-r", "1-2", "-r", "3"])
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
        .args(["range", "sample.txt", "-r", "100-120"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: range 100-120 clipped to 3-3",
        ))
        .stdout(predicate::str::contains("@@ 3-3 requested=100-120"))
        .stdout(predicate::str::contains(format!("  3 {V} c")));
}

#[test]
fn invalid_slice_fails_without_output_body() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "-r", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid range: 0"))
        .stdout(predicate::str::is_empty());
}

#[test]
fn json_output_uses_standard_envelope() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "range", "sample.txt", "-r", "2-3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "read-range");
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

#[test]
fn head_reads_first_n_lines() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\ne\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "--head", "3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("@@ 1-3 requested=head:3"));
    assert!(stdout.contains(format!("  1 {V} a").as_str()));
    assert!(stdout.contains(format!("  3 {V} c").as_str()));
    assert!(!stdout.contains(format!("  4 {V} d").as_str()));
}

#[test]
fn tail_reads_last_n_lines() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\nc\nd\ne\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "--tail", "2"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("@@ 4-5 requested=tail:2"));
    assert!(stdout.contains(format!("  4 {V} d").as_str()));
    assert!(stdout.contains(format!("  5 {V} e").as_str()));
    assert!(!stdout.contains(format!("  1 {V} a").as_str()));
}

#[test]
fn no_args_prints_entire_file() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "x\ny\nz\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(stdout.contains("@@ 1-3 requested=all"));
    assert!(stdout.contains(format!("  1 {V} x").as_str()));
    assert!(stdout.contains(format!("  3 {V} z").as_str()));
}

#[test]
fn head_tail_range_mutually_exclusive() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "--head", "1", "--tail", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn head_zero_fails() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "--head", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least 1"));
}

#[test]
fn tail_zero_fails() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.txt"), "a\nb\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["range", "sample.txt", "--tail", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least 1"));
}
