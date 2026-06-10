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
fn compose_cli_prompt_prints_agent_guide() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["compose", "--prompt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Squire compose template guide"))
        .stdout(predicate::str::contains("--allow-exec"));
}

#[test]
fn compose_cli_default_writes_temp_file_and_reports_path() {
    let dir = tempdir_with_file("name.txt", "Agent\n");
    let assert = Command::cargo_bin("squire")
        .unwrap()
        .args(["compose", "--template", "Hello ${{file: name.txt |> trim}}"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::starts_with("output: "));

    let path = output_path(&assert.get_output().stdout);
    assert_eq!(fs::read_to_string(path).unwrap(), "Hello Agent");
}

#[test]
fn compose_cli_stdout_writes_rendered_body_only() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("name.txt"), "Agent\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "compose",
            "--template",
            "Hello ${{file: name.txt |> trim}}",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout("Hello Agent");
}

#[test]
fn compose_cli_json_status_does_not_embed_rendered_body() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("name.txt"), "Secret Body\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "--print",
            "json",
            "compose",
            "--template",
            "${{file: name.txt |> trim}}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("Secret Body"));
    let json: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["ok"], true);
    let path = json["data"]["output"]["path"].as_str().unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "Secret Body");
}

#[test]
fn compose_cli_check_catches_compile_conflicts_without_execute() {
    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{exec: definitely-not-a-real-command |> stdout |> stderr}}",
            "--check",
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("conflicting_stream_selectors"));
}

#[test]
fn compose_cli_check_and_list_sources_do_not_execute() {
    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{exec: definitely-not-a-real-command}}",
            "--check",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok: template valid"));

    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{exec: definitely-not-a-real-command}}",
            "--list-sources",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "1  exec: definitely-not-a-real-command",
        ));
}

#[test]
fn compose_cli_stdin_source_reads_piped_input_once() {
    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{stdin |> trim}} / ${{stdin |> oneline}}",
            "--stdout",
        ])
        .write_stdin("hello\nworld\n")
        .assert()
        .success()
        .stdout("hello\nworld / hello world");
}

#[test]
fn compose_cli_env_file_encoding_and_fallback_work() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("gbk.txt"), [0xc4, 0xe3, 0xba, 0xc3]).unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .env("COMPOSE_TEST_ENV", "ok")
        .args([
            "compose",
            "--template",
            "${{env: COMPOSE_TEST_ENV}} ${{file: gbk.txt}} ${{file: missing.txt |> on-404: \"fallback\"}}",
            "--stdout",
        ])
        .assert()
        .success()
        .stdout("ok 你好 fallback");
}

#[test]
fn compose_cli_exec_runs_when_allowed_and_errors_can_fallback() {
    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{exec: rustc --version |> head: 1}}",
            "--stdout",
            "--allow-exec",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("rustc"));

    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{exec: exit 7 |> on-error: \"failed\"}}",
            "--stdout",
            "--allow-exec",
        ])
        .assert()
        .success()
        .stdout("failed");
}

#[test]
fn compose_cli_exec_requires_allow_exec() {
    Command::cargo_bin("squire")
        .unwrap()
        .args([
            "compose",
            "--template",
            "${{exec: rustc --version}}",
            "--stdout",
        ])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("exec: is disabled"));
}

#[test]
fn compose_cli_output_requires_overwrite_for_existing_file() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("out.md");
    fs::write(&output, "old").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "compose",
            "--template",
            "new",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("output file exists"));

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "compose",
            "--template",
            "new",
            "--output",
            output.to_str().unwrap(),
            "--overwrite",
        ])
        .assert()
        .success();
    assert_eq!(fs::read_to_string(output).unwrap(), "new");
}

#[test]
fn compose_cli_exec_spills_large_output_to_artifact() {
    let output = Command::cargo_bin("squire")
        .unwrap()
        .args([
            "--print",
            "json",
            "compose",
            "--template",
            "${{exec: rustc --version |> head: 1}}",
            "--allow-exec",
            "--max-command-bytes",
            "4",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["schemaVersion"], 1);
    assert_eq!(json["data"]["truncated"], true);
    let rendered_path = json["data"]["output"]["path"].as_str().unwrap();
    let rendered = fs::read_to_string(rendered_path).unwrap();
    assert!(rendered.contains("rust"));
    assert!(rendered.contains("stdout truncated after 4 bytes"));

    let artifact = &json["data"]["artifacts"][0];
    assert_eq!(artifact["kind"], "spill");
    assert_eq!(artifact["sourceIndex"], 1);
    assert_eq!(artifact["stream"], "stdout");
    assert_eq!(artifact["complete"], true);
    let spill_path = artifact["path"].as_str().unwrap();
    assert!(fs::read_to_string(spill_path).unwrap().contains("rustc"));
}

#[test]
fn compose_cli_fail_on_truncated_preserves_spill_artifact_in_error() {
    let output = Command::cargo_bin("squire")
        .unwrap()
        .args([
            "--print",
            "json",
            "compose",
            "--template",
            "${{exec: rustc --version}}",
            "--allow-exec",
            "--max-command-bytes",
            "4",
            "--fail-on-truncated",
        ])
        .assert()
        .code(9)
        .get_output()
        .stderr
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "limit_exceeded");
    let spill_path = json["error"]["artifacts"][0]["path"].as_str().unwrap();
    assert!(fs::read_to_string(spill_path).unwrap().contains("rustc"));
}

#[test]
fn compose_cli_template_load_errors_use_json_envelope() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing.tpl");

    let output = Command::cargo_bin("squire")
        .unwrap()
        .args([
            "--print",
            "json",
            "compose",
            "--template-file",
            missing.to_str().unwrap(),
        ])
        .assert()
        .code(4)
        .get_output()
        .stderr
        .clone();

    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "compose");
    assert_eq!(json["error"]["code"], "template_read_failed");
    assert_eq!(json["error"]["case"], "404");
    assert_eq!(json["meta"]["schemaVersion"], 1);
}

fn tempdir_with_file(name: &str, content: &str) -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(name), content).unwrap();
    dir
}
