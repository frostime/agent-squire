use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn json_finds_backlinks_by_resolved_target() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("README.md"), "[foo](notes/foo.md#intro)\n").unwrap();
    fs::write(dir.path().join("docs/index.md"), "[foo](../notes/foo.md)\n").unwrap();
    fs::write(dir.path().join("notes/bar.md"), "[[notes/foo|Foo]]\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-backlinks", "notes/foo.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let page = &json["data"]["pages"][0];
    let backlinks = page["backlinks"].as_array().unwrap();

    assert_eq!(json["command"], "md-backlinks");
    assert_eq!(json["data"]["focus_count"], 1);
    assert_eq!(json["data"]["total_backlinks"], 3);
    assert_eq!(page["path"], "notes/foo.md");
    assert_eq!(page["exists"], true);
    assert_eq!(backlinks[0]["source"], "README.md");
    assert_eq!(backlinks[0]["line_num"], 1);
    assert_eq!(backlinks[0]["kind"], "markdown");
    assert_eq!(backlinks[0]["raw"], "notes/foo.md#intro");
    assert_eq!(backlinks[1]["source"], "docs/index.md");
    assert_eq!(backlinks[1]["raw"], "../notes/foo.md");
    assert_eq!(backlinks[2]["source"], "notes/bar.md");
    assert_eq!(backlinks[2]["kind"], "wiki");
    assert_eq!(backlinks[2]["raw"], "notes/foo|Foo");
    assert_eq!(json["meta"]["from"][0], ".");
    assert!(
        json["meta"]["cwd"]
            .as_str()
            .unwrap()
            .ends_with(dir.path().file_name().unwrap().to_str().unwrap())
    );
    assert_eq!(json["meta"]["respect_gitignore"], true);
}

#[test]
fn global_cwd_is_the_single_root_for_focus_and_from_paths() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("outside")).unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("docs/index.md"), "[foo](../notes/foo.md)\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path().join("outside"))
        .args([
            "--cwd",
            dir.path().to_str().unwrap(),
            "--print",
            "json",
            "md-backlinks",
            "notes/foo.md",
            "--from",
            "docs",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["data"]["pages"][0]["path"], "notes/foo.md");
    assert_eq!(json["data"]["corpus_files"], 1);
    assert_eq!(json["data"]["total_backlinks"], 1);
    assert_eq!(
        json["data"]["pages"][0]["backlinks"][0]["source"],
        "docs/index.md"
    );
}

#[test]
fn md_backlinks_rejects_command_local_workspace() {
    Command::cargo_bin("squire")
        .unwrap()
        .args(["md-backlinks", "notes/foo.md", "--workspace", "."])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--workspace"));
}

#[test]
fn duplicate_focus_pages_are_deduplicated() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("README.md"), "[foo](notes/foo.md)\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "--print",
            "json",
            "md-backlinks",
            "notes/foo.md",
            "./notes/foo.md",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["data"]["focus_count"], 1);
    assert_eq!(json["data"]["pages"].as_array().unwrap().len(), 1);
    assert_eq!(json["data"]["total_backlinks"], 1);
}

#[test]
fn plain_text_filename_is_not_a_backlink() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("notes/other.md"), "# Other\n").unwrap();
    fs::write(
        dir.path().join("README.md"),
        "Plain text mentions notes/foo.md.\n[other](notes/other.md)\n",
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-backlinks", "notes/foo.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["data"]["total_backlinks"], 0);
    assert_eq!(
        json["data"]["pages"][0]["backlinks"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn default_corpus_respects_gitignore_and_no_gitignore_overrides_it() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("ignored.md"), "[foo](notes/foo.md)\n").unwrap();
    fs::write(dir.path().join("visible.md"), "# Visible\n").unwrap();

    let default_output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-backlinks", "notes/foo.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let default_json: Value = serde_json::from_slice(&default_output).unwrap();
    assert_eq!(default_json["data"]["total_backlinks"], 0);
    assert_eq!(default_json["meta"]["respect_gitignore"], true);

    let all_output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "--print",
            "json",
            "md-backlinks",
            "notes/foo.md",
            "--no-gitignore",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let all_json: Value = serde_json::from_slice(&all_output).unwrap();
    assert_eq!(all_json["data"]["total_backlinks"], 1);
    assert_eq!(
        all_json["data"]["pages"][0]["backlinks"][0]["source"],
        "ignored.md"
    );
    assert_eq!(all_json["meta"]["respect_gitignore"], false);
}

#[test]
fn explicit_ignored_file_in_from_is_included() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("ignored.md"), "[foo](notes/foo.md)\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "--print",
            "json",
            "md-backlinks",
            "notes/foo.md",
            "--from",
            "ignored.md",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["data"]["total_backlinks"], 1);
    assert_eq!(
        json["data"]["pages"][0]["backlinks"][0]["source"],
        "ignored.md"
    );
}

#[test]
fn compact_output_is_dense_and_grouped_by_page() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/foo.md"), "# Foo\n").unwrap();
    fs::write(dir.path().join("README.md"), "[foo](notes/foo.md)\n").unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["md-backlinks", "notes/foo.md"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# focus=1 corpus_files=2 backlinks=1 gitignore=true builtin_skip=true",
        ))
        .stdout(predicate::str::contains(
            "@ notes/foo.md exists=true backlinks=1",
        ))
        .stdout(predicate::str::contains(
            "README.md:L1|markdown|\"notes/foo.md\"",
        ));
}

#[test]
fn missing_focus_page_can_have_backlinks() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("README.md"), "[ghost](missing.md)\n").unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-backlinks", "missing.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json["data"]["pages"][0]["path"], "missing.md");
    assert_eq!(json["data"]["pages"][0]["exists"], false);
    assert_eq!(json["data"]["total_backlinks"], 1);
    assert_eq!(
        json["data"]["pages"][0]["backlinks"][0]["source"],
        "README.md"
    );
}
