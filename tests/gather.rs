use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn output_path(stdout: &[u8]) -> String {
    let text = String::from_utf8(stdout.to_vec()).unwrap();
    text.strip_prefix("output: ").unwrap().trim().to_string()
}

#[test]
fn gather_prompt_prints_agent_guide() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["gather", "--prompt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Squire gather guide"))
        .stdout(predicate::str::contains("file:path:start-end"))
        .stdout(predicate::str::contains("fzf"));
}

#[test]
fn gather_stdout_renders_file_and_range() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("main.rs"), "one\ntwo\nthree\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "file:main.rs:2-3"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "====== FILE-START: main.rs:2-3 ======",
        ))
        .stdout(predicate::str::contains("two\nthree"))
        .stdout(predicate::str::contains("====== FILE-END ======"));
}

#[test]
fn gather_default_writes_asq_gather_temp_file() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let assert = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "file:a.txt"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("output: "));

    let path = output_path(&assert.get_output().stdout);
    assert!(path.contains("asq-gather-"), "{path}");
    let body = fs::read_to_string(path).unwrap();
    assert!(body.contains("alpha"));
}

#[test]
fn gather_output_file_respects_overwrite_guard() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    let out = dir.path().join("out.md");

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "file:a.txt", "--output", out.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "file:a.txt", "--output", out.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("pass --overwrite"));
}

#[test]
fn gather_cmd_enables_exec_internally() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["gather", "--stdout", "cmd:echo gather-ok"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "====== CMD-START: echo gather-ok ======",
        ))
        .stdout(predicate::str::contains("gather-ok"));
}

#[test]
fn gather_dir_expands_to_grouped_file_blocks() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.rs"), "alpha\n").unwrap();
    fs::write(dir.path().join("src/b.rs"), "beta\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "dir:src"])
        .assert()
        .success()
        .stdout(predicate::str::contains("====== DIR-START: src ======"))
        .stdout(predicate::str::contains("Matched files:"))
        .stdout(predicate::str::contains("- src/a.rs"))
        .stdout(predicate::str::contains(
            "====== DIR-FILE-START: src/a.rs ======",
        ))
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("====== DIR-END ======"));
}

#[test]
fn gather_glob_expands_to_grouped_file_blocks() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.rs"), "alpha\n").unwrap();
    fs::write(dir.path().join("src/a.txt"), "ignored\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "glob:src/*.rs"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "====== GLOB-START: src/*.rs ======",
        ))
        .stdout(predicate::str::contains("- src/a.rs"))
        .stdout(predicate::str::contains(
            "====== GLOB-FILE-START: src/a.rs ======",
        ))
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("ignored").not());
}

#[test]
fn gather_no_gitignore_includes_ignored_dir_files() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.rs"), "alpha\n").unwrap();
    fs::write(dir.path().join("src/ignored.rs"), "ignored\n").unwrap();
    fs::write(dir.path().join(".gitignore"), "src/ignored.rs\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "dir:src"])
        .assert()
        .success()
        .stdout(predicate::str::contains("src/ignored.rs").not());

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "--no-gitignore", "dir:src"])
        .assert()
        .success()
        .stdout(predicate::str::contains("src/ignored.rs"))
        .stdout(predicate::str::contains("ignored"));
}

#[test]
fn gather_interactive_done_renders_and_exit_aborts() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:a.txt\n/done\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha"));

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:a.txt\n/exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha").not());
}

#[test]
fn gather_tree_renders_structure_without_exec_requirement() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.rs"), "alpha\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "tree:src"])
        .assert()
        .success()
        .stdout(predicate::str::contains("====== TREE-START: src ======"))
        .stdout(predicate::str::contains("src/"))
        .stdout(predicate::str::contains("src/a.rs"));
}

#[test]
fn gather_json_status_uses_gather_identity() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "gather", "file:a.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    let json: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "gather");
    let path = json["data"]["output"]["path"].as_str().unwrap();
    assert!(path.contains("asq-gather-"));
    assert!(fs::read_to_string(path).unwrap().contains("alpha"));
}
