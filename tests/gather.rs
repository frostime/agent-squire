use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn output_path(stdout: &[u8]) -> String {
    let text = String::from_utf8(stdout.to_vec()).unwrap();
    text.strip_prefix("output: ").unwrap().trim().to_string()
}

fn extract_zip(zip_path: &Path) -> tempfile::TempDir {
    let out = tempdir().unwrap();
    #[cfg(windows)]
    let status = StdCommand::new("powershell")
        .args(["-NoProfile", "-Command", "Expand-Archive"])
        .arg("-Path")
        .arg(zip_path)
        .arg("-DestinationPath")
        .arg(out.path())
        .arg("-Force")
        .status()
        .unwrap();
    #[cfg(not(windows))]
    let status = StdCommand::new("unzip")
        .arg("-q")
        .arg(zip_path)
        .arg("-d")
        .arg(out.path())
        .status()
        .unwrap();
    assert!(status.success(), "failed to extract {}", zip_path.display());
    out
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

// ── /zip integration tests ──

#[test]
fn gather_zip_creates_structured_archive() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:a.txt\n/zip asq-test-gather-zip-1.zip\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Should mention the zip file was written
    assert!(
        stdout.contains("asq-test-gather-zip-1.zip"),
        "stdout: {stdout}"
    );

    let zip_path = dir.path().join("asq-test-gather-zip-1.zip");
    assert!(
        zip_path.exists(),
        "zip should exist at {}",
        zip_path.display()
    );

    let extracted = extract_zip(&zip_path);
    assert_eq!(
        fs::read_to_string(extracted.path().join("files/a.txt")).unwrap(),
        "alpha\n"
    );
    let manifest = fs::read_to_string(extracted.path().join("manifest.json")).unwrap();
    assert!(manifest.contains(r#""inZip": "files/a.txt""#));
}

#[test]
fn gather_zip_empty_sources_errors() {
    let dir = tempdir().unwrap();

    let assert = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("/zip\n/exit\n")
        .assert()
        .success();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("No sources to package"), "stderr: {stderr}");
}

#[test]
fn gather_zip_done_exits_after_packaging() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:a.txt\n/zip /done\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Zip written"))
        .stdout(predicate::str::contains("alpha"));
}

#[test]
fn gather_zip_includes_ranged_file_artifact() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("main.rs"),
        "line1\nline2\nline3\nline4\nline5\n",
    )
    .unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:main.rs:2-4\n/zip ranged.zip /done\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Zip written"));

    let extracted = extract_zip(&dir.path().join("ranged.zip"));
    let artifact = extracted.path().join("artifacts/file-0-main.rs-L2-4.txt");
    assert_eq!(fs::read_to_string(artifact).unwrap(), "line2\nline3\nline4");
}

#[test]
fn gather_zip_existing_output_errors_without_overwrite() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    fs::write(dir.path().join("existing.zip"), "keep me").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:a.txt\n/zip existing.zip\n/exit\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("output file exists"));

    assert_eq!(
        fs::read_to_string(dir.path().join("existing.zip")).unwrap(),
        "keep me"
    );
}

#[test]
fn gather_zip_no_file_sources_does_not_execute_commands() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("cmd:echo side > marker\n/zip\n/exit\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("No file sources to package"));

    assert!(!dir.path().join("marker").exists());
}

#[test]
fn gather_zip_warning_cancel_does_not_execute_commands() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("bin.dat"), [0, 1, 2, 3]).unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["gather", "--stdout", "-i"])
        .write_stdin("file:bin.dat\ncmd:echo side > marker\n/zip\nn\n/exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("binary file(s) detected"));

    assert!(!dir.path().join("marker").exists());
}
