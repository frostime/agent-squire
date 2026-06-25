use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn data_toc_prompt_prints_agent_guide() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["data-toc", "--prompt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Squire data-toc guide"))
        .stdout(predicate::str::contains("Output interpretation"))
        .stdout(predicate::str::contains("JSONL record groups"));
}

#[test]
fn data_toc_alias_matches_prompt_surface() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["datatoc", "--prompt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Squire data-toc guide"));
}

#[test]
fn data_toc_json_compact_collapses_arrays_and_reports_presence() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("result.json"),
        r#"{
  "runs": [
    {"id": "a", "metrics": {"acc": 0.9}},
    {"id": "b", "metrics": {"acc": 0.8}, "notes": "x"}
  ]
}"#,
    )
    .unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["data-toc", "result.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("format=json"))
        .stdout(predicate::str::contains("runs array<object>"))
        .stdout(predicate::str::contains("[] object"))
        .stdout(predicate::str::contains("notes string? 1/2"))
        .stdout(predicate::str::contains(
            "Array indexes are collapsed into []",
        ));
}

#[test]
fn data_toc_jsonl_compact_reports_groups_and_first_lines() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("logs.jsonl"),
        concat!(
            r#"{"type":"message","timestamp":"t1","payload":{"text":"hello"}}"#,
            "\n",
            r#"{"type":"error","timestamp":"t2","error":{"code":500}}"#,
            "\n",
            r#"{"type":"metric","timestamp":"t3","name":"latency","value":31}"#,
            "\n",
        ),
    )
    .unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["data-toc", "logs.jsonl", "--format", "jsonl"])
        .assert()
        .success()
        .stdout(predicate::str::contains("format=jsonl"))
        .stdout(predicate::str::contains("Record groups:"))
        .stdout(predicate::str::contains("type=message rows=1 first_line=1"))
        .stdout(predicate::str::contains("type=error rows=1 first_line=2"))
        .stdout(predicate::str::contains("type=metric rows=1 first_line=3"));
}

#[test]
fn data_toc_json_output_uses_envelope() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("result.json"), r#"{"runs":[{"id":"a"}]}"#).unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "data-toc", "result.json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "data-toc");
    assert_eq!(json["data"]["format"], "json");
    assert_eq!(json["data"]["mode"], "structure_toc");
    assert_eq!(json["data"]["root"]["children"][0]["path"], "$.runs");
    assert!(json["warnings"].as_array().unwrap().is_empty());
    assert_eq!(json["meta"]["budget"], "normal");
    assert_eq!(json["meta"]["schema_version"], 1);
}

#[test]
fn data_toc_invalid_jsonl_reports_line_number() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("bad.jsonl"), "{\"ok\":true}\nnot-json\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["data-toc", "bad.jsonl", "--format", "jsonl"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid JSONL at line 2"));
}

#[test]
fn data_toc_yaml_requires_yq() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("compose.yaml"),
        "services:\n  app:\n    image: demo\n",
    )
    .unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .env("PATH", "/nonexistent")
        .args(["data-toc", "compose.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("YAML support requires yq"));
}

#[test]
fn data_toc_yaml_uses_yq_when_available() {
    if std::process::Command::new("yq")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }

    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("compose.yaml"),
        "services:\n  app:\n    image: demo\n    ports:\n      - '8080:80'\n",
    )
    .unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["data-toc", "compose.yaml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("format=yaml"))
        .stdout(predicate::str::contains("parsed_as=json"))
        .stdout(predicate::str::contains("services object"))
        .stdout(predicate::str::contains(
            "YAML comments, anchors, aliases, tags, and formatting are not preserved",
        ));
}
