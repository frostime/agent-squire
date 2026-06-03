use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn json_output_extracts_reference_occurrences_and_resolves_files() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::create_dir_all(dir.path().join("assets")).unwrap();
    fs::write(dir.path().join("docs/intro.md"), "# Intro\n").unwrap();
    fs::write(dir.path().join("Design.md"), "# Design\n").unwrap();
    fs::write(dir.path().join("assets/logo.png"), "png").unwrap();
    fs::write(
        dir.path().join("README.md"),
        "[intro](docs/intro.md#install)\n\
         ![logo](./assets/logo.png)\n\
         [[Design]]\n\
         [web](https://example.com)\n\
         <siyuan://blocks/20260531010806-35bkoxa>\n\
         ((20260531010806-35bkoxa '2026-05-31'))\n",
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-links", "README.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let links = json["data"]["files"][0]["links"].as_array().unwrap();

    assert_eq!(json["command"], "md-links");
    assert_eq!(json["data"]["total_links"], 6);
    assert_eq!(json["data"]["total_file_links"], 3);
    assert_eq!(json["data"]["total_existing_file_links"], 3);
    assert_eq!(links[0]["kind"], "markdown");
    assert_eq!(links[0]["raw"], "docs/intro.md#install");
    assert_eq!(links[0]["target_type"], "file");
    assert_eq!(links[0]["resolved"], "docs/intro.md");
    assert_eq!(links[0]["exists"], true);
    assert_eq!(links[2]["kind"], "wiki");
    assert_eq!(links[2]["resolved"], "Design.md");
    assert_eq!(links[3]["target_type"], "url");
    assert_eq!(
        links[4]["resolved"],
        "siyuan://blocks/20260531010806-35bkoxa"
    );
    assert_eq!(links[5]["target_type"], "siyuan_block");
    assert_eq!(links[5]["resolved"], "20260531010806-35bkoxa");
}

#[test]
fn directory_and_glob_sources_discover_markdown_files() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("docs/nested")).unwrap();
    fs::write(dir.path().join("docs/a.md"), "[x](missing-a.md)\n").unwrap();
    fs::write(dir.path().join("docs/nested/b.md"), "[x](missing-b.md)\n").unwrap();
    fs::write(dir.path().join("docs/skip.txt"), "[x](skip.md)\n").unwrap();

    let directory_output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-links", "docs"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let directory_json: Value = serde_json::from_slice(&directory_output).unwrap();

    assert_eq!(directory_json["data"]["count"], 2);
    assert_eq!(directory_json["data"]["total_links"], 2);

    let glob = dir
        .path()
        .join("docs/*.md")
        .to_string_lossy()
        .replace('\\', "/");
    let glob_output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-links", &glob])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let glob_json: Value = serde_json::from_slice(&glob_output).unwrap();

    assert_eq!(glob_json["data"]["count"], 1);
    assert_eq!(glob_json["data"]["files"][0]["path"], "docs/a.md");
}

#[test]
fn workspace_paths_are_supported_across_markdown_wiki_and_code_refs() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("notes/sub")).unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "").unwrap();
    fs::write(dir.path().join("src/main.rs"), "").unwrap();
    fs::write(dir.path().join("src/wiki.md"), "").unwrap();
    fs::write(
        dir.path().join("notes/sub/page.md"),
        "[lib](/src/lib.rs)\n[[src/wiki]]\n`src/main.rs`\n`plain`\n",
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path().join("notes/sub"))
        .args([
            "--print",
            "json",
            "md-links",
            "page.md",
            "--workspace",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let links = json["data"]["files"][0]["links"].as_array().unwrap();

    assert_eq!(links.len(), 3);
    assert_eq!(links[0]["resolved"], "src/lib.rs");
    assert_eq!(links[0]["exists"], true);
    assert_eq!(links[1]["resolved"], "src/wiki.md");
    assert_eq!(links[2]["kind"], "code_span");
    assert_eq!(links[2]["resolved"], "src/main.rs");
}

#[test]
fn source_relative_paths_and_backslashes_are_normalized() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("docs/assets")).unwrap();
    fs::write(dir.path().join("docs/assets/logo.png"), "").unwrap();
    fs::write(
        dir.path().join("docs/page.md"),
        "![logo](.\\assets\\logo.png)\n",
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-links", "docs/page.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let link = &json["data"]["files"][0]["links"][0];

    assert_eq!(link["resolved"], "docs/assets/logo.png");
    assert_eq!(link["exists"], true);
}

#[test]
fn fenced_code_links_are_ignored_and_missing_files_are_reported() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("README.md"),
        "```\n[skip](missing.md)\n```\n[missing](missing.md)\n",
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "md-links", "README.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let links = json["data"]["files"][0]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["raw"], "missing.md");
    assert_eq!(links[0]["exists"], false);
}

#[test]
fn compact_output_groups_dense_agent_records_by_file() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/intro.md"), "# Intro\n").unwrap();
    fs::write(
        dir.path().join("README.md"),
        "[intro](docs/intro.md#install)\n[missing](missing.md)\n",
    )
    .unwrap();

    Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["md-links", "README.md"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# files=1 links=2 file_links=2 existing_file_links=1 missing_file_links=1",
        ))
        .stdout(predicate::str::contains(
            "@ README.md links=2 file_links=2 missing_file_links=1",
        ))
        .stdout(predicate::str::contains(
            "L1|ok|markdown|file|\"docs/intro.md#install\"=>\"docs/intro.md\"",
        ))
        .stdout(predicate::str::contains(
            "L2|missing|markdown|file|\"missing.md\"",
        ));
}
