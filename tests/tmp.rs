use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn tmp_creates_markdown_file_with_timestamp_prefix_by_default() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["tmp", "--root", root.to_str().unwrap(), "note"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(
        name.contains("_note.md"),
        "expected timestamp-prefixed note.md, got {name}"
    );
    assert!(path.is_file());
}

#[test]
fn tmp_no_time_prefix_creates_plain_markdown_file() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "note",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "note.md");
    assert!(path.is_file());
}

#[test]
fn tmp_dir_flag_creates_directory() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--dir",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "logs",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "logs");
    assert!(path.is_dir());
}

#[test]
fn tmp_infers_directory_from_trailing_slash() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "logs/",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "logs");
    assert!(path.is_dir());
}

#[test]
fn tmp_file_flag_overrides_trailing_slash() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--file",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "logs/",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "logs");
    assert!(path.is_file());
}

#[test]
fn tmp_honors_explicit_extension() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "scratch.py",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "scratch.py");
    assert!(path.is_file());
}

#[test]
fn tmp_does_not_overwrite_existing_file() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let existing = root.join("note.md");
    fs::write(&existing, "existing content").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "note",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&existing).unwrap();
    assert_eq!(content, "existing content");
}

#[test]
fn tmp_creates_custom_root_if_missing() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("nested").join("root");
    assert!(!root.exists());

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "x",
        ])
        .assert()
        .success();

    assert!(root.exists());
    assert!(root.join("x.md").exists());
}

#[test]
fn tmp_generates_random_name_when_name_is_omitted() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["tmp", "--no-time-prefix", "--root", root.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert!(path.is_file());
    let name = path.file_name().unwrap().to_str().unwrap();
    assert_eq!(name.len(), 8, "expected 5 random chars + .md, got {name}");
    assert!(name.ends_with(".md"));
}

#[test]
fn tmp_generates_random_directory_when_name_is_omitted_with_dir_flag() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "tmp",
            "--dir",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let path = PathBuf::from(stdout.trim());

    assert!(path.is_dir());
    let name = path.file_name().unwrap().to_str().unwrap();
    assert_eq!(name.len(), 5, "expected 5 random chars, got {name}");
}

#[test]
fn tmp_json_output_includes_path_kind_and_root() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "--print",
            "json",
            "tmp",
            "--no-time-prefix",
            "--root",
            root.to_str().unwrap(),
            "note",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert!(json["ok"].as_bool().unwrap());
    assert_eq!(json["command"], "tmp");

    let data = &json["data"];
    let path = data["path"].as_str().unwrap();
    assert!(path.ends_with("note.md"), "unexpected path {path}");
    assert_eq!(data["kind"], "file");
    assert!(data["root"].as_str().unwrap().contains("root"));
}
