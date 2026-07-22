use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn utf32_bom_bytes(text: &str, little_endian: bool) -> Vec<u8> {
    let mut bytes = if little_endian {
        vec![0xFF, 0xFE, 0x00, 0x00]
    } else {
        vec![0x00, 0x00, 0xFE, 0xFF]
    };
    for ch in text.chars() {
        let pair = if little_endian {
            (ch as u32).to_le_bytes()
        } else {
            (ch as u32).to_be_bytes()
        };
        bytes.extend_from_slice(&pair);
    }
    bytes
}

fn utf16_bom_bytes(text: &str, little_endian: bool) -> Vec<u8> {
    let mut bytes = if little_endian {
        vec![0xFF, 0xFE]
    } else {
        vec![0xFE, 0xFF]
    };
    for unit in text.encode_utf16() {
        let pair = if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        bytes.extend_from_slice(&pair);
    }
    bytes
}

#[test]
fn file_info_reports_utf32_bom_without_misclassifying_as_utf16() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("le.txt"), utf32_bom_bytes("A\nB\n", true)).unwrap();
    fs::write(dir.path().join("be.txt"), utf32_bom_bytes("A\nB\n", false)).unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "file-info", "le.txt", "be.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let files = json["data"]["files"].as_array().unwrap();

    assert_eq!(files[0]["encoding"], "utf-32-be");
    assert_eq!(files[0]["bom"], "utf-32-be");
    assert_eq!(files[0]["newline"], "unknown");
    assert!(files[0]["line_count"].is_null());
    assert_eq!(files[1]["encoding"], "utf-32-le");
    assert_eq!(files[1]["bom"], "utf-32-le");
    assert_eq!(files[1]["newline"], "unknown");
    assert!(files[1]["line_count"].is_null());
}

#[test]
fn file_info_glob_recurses_into_matched_directories() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("packages/alpha")).unwrap();
    fs::create_dir_all(dir.path().join("packages/beta/src")).unwrap();
    fs::write(dir.path().join("packages/alpha/README.md"), "# Alpha\n").unwrap();
    fs::write(
        dir.path().join("packages/beta/src/lib.rs"),
        "fn beta() {}\n",
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "file-info", "packages/*"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let paths = json["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(paths.len(), 2);
    assert!(paths[0].ends_with("/packages/alpha/README.md"));
    assert!(paths[1].ends_with("/packages/beta/src/lib.rs"));
    assert!(
        json["data"]["missing_sources"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn file_info_reports_utf16_bom_newline_and_line_count() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("le.txt"),
        utf16_bom_bytes("第一行\r\n第二行\r\n第三行\r\n", true),
    )
    .unwrap();
    fs::write(
        dir.path().join("be.txt"),
        utf16_bom_bytes("first\r\nsecond\r\nthird\r\n", false),
    )
    .unwrap();

    let output = Command::cargo_bin("squire")
        .unwrap()
        .current_dir(dir.path())
        .args(["--print", "json", "file-info", "le.txt", "be.txt"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    let files = json["data"]["files"].as_array().unwrap();

    assert_eq!(files[0]["encoding"], "utf-16-be");
    assert_eq!(files[0]["newline"], "crlf");
    assert_eq!(files[0]["line_count"], 3);
    assert_eq!(files[1]["encoding"], "utf-16-le");
    assert_eq!(files[1]["newline"], "crlf");
    assert_eq!(files[1]["line_count"], 3);
}
