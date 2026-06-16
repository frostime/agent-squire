use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

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
