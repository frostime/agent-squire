use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn squire() -> Command {
    Command::cargo_bin("squire").unwrap()
}

#[test]
fn dry_run_does_not_write_then_yes_applies_state_transition() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nB\nC\n").unwrap();
    let spec = "arrange a.md\n  before A = 1-1, B = 2-2, C = 3-end\n  after C, A, B\nend arrange";

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin(spec)
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"))
        .stdout(predicate::str::contains("before: A=1-1, B=2-2, C=3-end"))
        .stdout(predicate::str::contains("after : C, A, B"))
        .stdout(predicate::str::contains("No file written"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "A\nB\nC\n");

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(spec)
        .assert()
        .success()
        .stdout(predicate::str::contains("written"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "C\nA\nB\n");
}

#[test]
fn explicit_gap_can_be_preserved_and_moved() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nhidden\nB\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "arrange a.md\n  before A = 1-1, <gap:hidden>, B = 3-end\n  after B, <gap:hidden>, A\nend arrange",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("gap hidden = 2-2"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "B\nhidden\nA\n");
}

#[test]
fn hidden_gap_fails_without_writing() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nhidden\nB\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("arrange a.md\n  before A = 1-1, B = 3-end\n  after B, A\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("UNDECLARED_GAP"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "A\nhidden\nB\n");
}

#[test]
fn cross_file_extract_creates_parent_directory() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("foo.rs");
    fs::write(&file, "api\nparser\nrest\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "arrange main = foo.rs\n  before api = 1-1, parser = 2-2, rest = 3-end\n  after api, rest\nend arrange\n\narrange src/parser.rs\n  before <missing>\n  after main::parser\nend arrange",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("target main = foo.rs"))
        .stdout(predicate::str::contains("target src/parser.rs"));

    assert_eq!(fs::read_to_string(&file).unwrap(), "api\nrest\n");
    assert_eq!(
        fs::read_to_string(dir.path().join("src/parser.rs")).unwrap(),
        "parser\n"
    );
}

#[test]
fn share_material_can_be_inserted() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("snippets")).unwrap();
    fs::write(dir.path().join("snippets/header.rs"), "// header\n").unwrap();
    fs::write(dir.path().join("foo.rs"), "body\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "share tpl = snippets/header.rs\n  header = 1-end\nend share\n\narrange foo.rs\n  before body = 1-end\n  after tpl::header, body\nend arrange",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("share tpl = snippets/header.rs"));

    assert_eq!(
        fs::read_to_string(dir.path().join("foo.rs")).unwrap(),
        "// header\nbody\n"
    );
}

#[test]
fn empty_and_missing_states_create_and_clear_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("full.md"), "content\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "arrange empty/new.txt\n  before <missing>\n  after <empty>\nend arrange\n\narrange full.md\n  before all = 1-end\n  after <empty>\nend arrange",
        )
        .assert()
        .success();

    assert_eq!(
        fs::metadata(dir.path().join("empty/new.txt"))
            .unwrap()
            .len(),
        0
    );
    assert_eq!(fs::read_to_string(dir.path().join("full.md")).unwrap(), "");
}

#[test]
fn duplicate_normalized_arrange_path_fails() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin(
            "arrange a.md\n  before A = 1-end\n  after A\nend arrange\n\narrange ./a.md\n  before A = 1-end\n  after A\nend arrange",
        )
        .assert()
        .failure()
        .stderr(predicate::str::contains("DUPLICATE_PATH"));
}

#[test]
fn named_before_range_cannot_be_referenced_as_bare_after_range() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin("arrange a.md\n  before A = 1-end\n  after 1-end\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("UNKNOWN_REFERENCE"));
}

#[test]
fn crlf_newline_preserved_for_existing_target() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\r\nB\r\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin("arrange a.md\n  before A = 1-1, B = 2-end\n  after B, A\nend arrange")
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&file).unwrap(), "B\r\nA\r\n");
}

#[test]
fn dry_run_overrides_yes() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "A\nB\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--dry-run", "--yes"])
        .write_stdin("arrange a.md\n  before A = 1-1, B = 2-end\n  after B, A\nend arrange")
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "A\nB\n");
}

#[test]
fn json_output_envelope_contains_targets() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\nB\n").unwrap();

    let out = squire()
        .current_dir(dir.path())
        .args(["--json", "rearrange", "--stdin"])
        .write_stdin("arrange a.md\n  before A = 1-1, B = 2-end\n  after B, A\nend arrange")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "rearrange");
    assert_eq!(json["data"]["written"], false);
    assert_eq!(json["data"]["targets"][0]["path"], "a.md");
    assert_eq!(json["data"]["targets"][0]["after"], "B, A");
}

#[test]
fn invalid_utf8_bom_file_fails_without_lossy_rewrite() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.txt");
    let original = vec![0xEF, 0xBB, 0xBF, 0xFF, b'\n', b'A', b'\n'];
    fs::write(&file, &original).unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "arrange a.txt\n  before bad = 1-1, good = 2-end\n  after good, bad\nend arrange",
        )
        .assert()
        .failure()
        .stderr(predicate::str::contains("ENCODING_ERROR"));
    assert_eq!(fs::read(&file).unwrap(), original);
}

#[test]
fn unspaced_slugged_arrange_does_not_become_path() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("header.rs"), "h\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "share tpl=header.rs\n  header=1-end\nend share\n\narrange main=src/foo.rs\n  before <missing>\n  after tpl::header\nend arrange",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("target main = src/foo.rs"));

    assert!(dir.path().join("src/foo.rs").is_file());
    assert!(!dir.path().join("main=src/foo.rs").exists());
}

#[test]
fn empty_sequence_items_are_rejected() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\nB\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin("arrange a.md\n  before A = 1-1, , B = 2-end\n  after B, A,\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("INVALID_SPEC"));
}

#[test]
fn self_external_reference_is_rejected() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin("arrange main = a.md\n  before A = 1-end\n  after main::A\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("UNKNOWN_REFERENCE"));
}

#[test]
fn gap_name_collisions_and_empty_gaps_are_rejected() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\nh\nB\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin("arrange a.md\n  before A = 1-1, <gap:A>, B = 3-end\n  after A, <gap:A>, B\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("DUPLICATE_NAME"));

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin("arrange a.md\n  before A = 1-1, <gap:g>, B = 2-end\n  after A, <gap:g>, B\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("EMPTY_GAP"));
}

#[test]
fn share_and_arrange_same_path_is_rejected() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin(
            "share src = ./a.md\n  A = 1-end\nend share\n\narrange a.md\n  before A = 1-end\n  after A\nend arrange",
        )
        .assert()
        .failure()
        .stderr(predicate::str::contains("DUPLICATE_PATH"));
}

#[test]
fn whitespace_only_file_is_not_empty_state() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin"])
        .write_stdin("arrange a.md\n  before <empty>\n  after <missing>\nend arrange")
        .assert()
        .failure()
        .stderr(predicate::str::contains("INVALID_STATE"));
}

#[test]
fn validation_failure_prevents_all_writes() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.md");
    let b = dir.path().join("b.md");
    fs::write(&a, "A\nB\n").unwrap();
    fs::write(&b, "X\nhidden\nY\n").unwrap();

    squire()
        .current_dir(dir.path())
        .args(["rearrange", "--stdin", "--yes"])
        .write_stdin(
            "arrange a.md\n  before A = 1-1, B = 2-end\n  after B, A\nend arrange\n\narrange b.md\n  before X = 1-1, Y = 3-end\n  after Y, X\nend arrange",
        )
        .assert()
        .failure()
        .stderr(predicate::str::contains("UNDECLARED_GAP"));

    assert_eq!(fs::read_to_string(a).unwrap(), "A\nB\n");
    assert_eq!(fs::read_to_string(b).unwrap(), "X\nhidden\nY\n");
}

#[test]
fn rearrange_help_does_not_describe_old_action_dsl() {
    squire()
        .args(["rearrange", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("state-transition"))
        .stdout(predicate::str::contains("Move, copy, delete").not())
        .stdout(predicate::str::contains("in one file").not());
}

#[test]
fn prompt_prints_dst_guide_without_old_actions() {
    squire()
        .args(["rearrange", "--prompt"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Arrange state-transition DSL"))
        .stdout(predicate::str::contains("share <slug> = <file>"))
        .stdout(predicate::str::contains("move/copy/delete").not());
}
