# gather-zip — Design

## Interface Contract

### `InteractiveCommand` 扩展

```rust
// interactive.rs
enum InteractiveCommand {
    Help,
    List,
    Done,
    Exit,
    ToggleAll,
    Zip { path: Option<String>, and_done: bool },  // NEW
}
```

### `parse_interactive_command()` 扩展

```rust
fn parse_interactive_command(line: &str) -> Option<InteractiveCommand> {
    let trimmed = line.trim().to_ascii_lowercase();
    // 先匹配 /zip 子命令
    if let Some(rest) = trimmed.strip_prefix("/zip") {
        let rest = rest.trim();
        if rest.is_empty() {
            return Some(InteractiveCommand::Zip { path: None, and_done: false });
        }
        if rest == "/done" || rest == "--done" {
            return Some(InteractiveCommand::Zip { path: None, and_done: true });
        }
        // rest 是路径，检查是否以 /done 结尾
        if let Some(path_part) = rest.strip_suffix(" /done")
            .or_else(|| rest.strip_suffix(" --done"))
        {
            return Some(InteractiveCommand::Zip {
                path: Some(path_part.trim().to_string()),
                and_done: true,
            });
        }
        return Some(InteractiveCommand::Zip {
            path: Some(rest.to_string()),
            and_done: false,
        });
    }
    // ... 原有匹配
}
```

### `zip.rs` 顶层入口

```rust
/// 组装 zip 并返回输出路径
/// 返回 None 表示用户取消（警告确认时选 n）
fn assemble_zip(
    sources: &[Source],
    cwd: &Path,
    respect_gitignore: bool,
    output_path: Option<PathBuf>,
) -> Result<Option<PathBuf>>;

/// 收集警告：二进制文件 + 大文件
/// 返回 (warnings, user_confirmed)
fn collect_warnings_and_confirm(
    file_entries: &[FileEntry],
) -> Result<bool>;

/// 调用外部 CLI 创建 zip
fn create_zip_archive(staging_dir: &Path, output: &Path) -> Result<()>;
```

## Structural Blueprint

```
src/builtins/gather/
├── mod.rs              ← 声明 mod zip;
├── model.rs            ← 可能新增 FileEntry, Manifest, ManifestEntry
├── parser.rs           ← 不变
├── sources.rs          ← 不变（复用 expand_dir/expand_glob/render_tree）
├── template.rs         ← 不变
├── interactive.rs      ← 新增 Zip variant + 处理逻辑
└── zip.rs              ← [NEW] 全部 zip 组装逻辑
```

## Data Architecture

### `FileEntry` — zip 文件组装中间表示

```rust
/// 去重后的文件条目
struct FileEntry {
    /// zip 内路径（如 "files/src/main.rs"）
    zip_path: String,
    /// 磁盘上的源路径（如 "H:\project\src\main.rs"），用于文件复制
    disk_path: PathBuf,
    /// 文件大小（bytes）
    size: u64,
    /// 是否为二进制（heuristic）
    is_binary: bool,
    /// 来源类型（用于 manifest）
    origin: FileOrigin,
}

enum FileOrigin {
    FileDirect,
    DirExpanded,
    GlobExpanded,
    SelectedGlob,
}
```

### `ArtifactEntry` — 文本产物

```rust
struct ArtifactEntry {
    /// "artifacts/cmd-0-git-diff.txt"
    zip_path: String,
    /// 文本内容（已生成）
    content: String,
    /// manifest 元数据
    meta: ArtifactMeta,
}

enum ArtifactMeta {
    Cmd { command: String, index: usize },
    Tree { path: PathBuf, index: usize },
    RangedFile { original_path: PathBuf, range: LineRange, index: usize },
}
```

### `manifest.json` Schema

```json
{
  "schema": 1,
  "cwd": "H:/SrcCode/playground/agent-squire",
  "created": "2026-06-30T00:40:14",
  "sources": [
    {
      "type": "file",
      "path": "src/main.rs",
      "inZip": "files/src/main.rs"
    },
    {
      "type": "file",
      "path": "src/main.rs",
      "range": { "start": 10, "end": 20 },
      "inZip": "artifacts/file-0-src-main.rs-L10-20.txt"
    },
    {
      "type": "dir",
      "path": "src/builtins",
      "fileCount": 8,
      "files": ["files/src/builtins/mod.rs", "files/src/builtins/gather/mod.rs"]
    },
    {
      "type": "glob",
      "pattern": "src/*.rs",
      "fileCount": 3,
      "files": ["files/src/main.rs", "files/src/lib.rs"]
    },
    {
      "type": "cmd",
      "command": "git diff HEAD~1",
      "inZip": "artifacts/cmd-0-git-diff.txt"
    },
    {
      "type": "tree",
      "path": "src",
      "inZip": "artifacts/tree-0-src.txt"
    }
  ]
}
```

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema: u8,
    cwd: String,
    created: String,
    sources: Vec<ManifestEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum ManifestEntry {
    #[serde(rename_all = "camelCase")]
    File {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<RangeField>,
        in_zip: String,
    },
    #[serde(rename_all = "camelCase")]
    Dir {
        path: String,
        file_count: usize,
        files: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Glob {
        pattern: String,
        file_count: usize,
        files: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Cmd {
        command: String,
        in_zip: String,
    },
    #[serde(rename_all = "camelCase")]
    Tree {
        path: String,
        in_zip: String,
    },
}

#[derive(Serialize)]
struct RangeField {
    start: usize,
    end: usize,
}
```

## Behavioral Spec

### `/zip` 命令处理流程

```text
用户输入 "/zip [path] [/done]"
    │
    ▼
parse_interactive_command() → Some(Zip { path, and_done })
    │
    ▼
sources.is_empty()? ──YES──→ eprintln!("No sources to package") → 回到循环
    │ NO
    ▼
assemble_zip(sources, cwd, respect_gitignore, path)
    │
    ├─ 1. 遍历 sources，分类为 FileEntry + ArtifactEntry
    │     - File/Dir/Glob/SelectedGlob → 展开为 FileEntry 列表 → 按 zip_path 去重
    │     - Cmd → 执行命令 → ArtifactEntry::Cmd
    │     - Tree → render_tree() → ArtifactEntry::Tree
    │     - File(range) → 读取原文件 → 切片 → ArtifactEntry::RangedFile
    │
    ├─ 2. collect_warnings_and_confirm(file_entries)
    │     - 检测二进制: 读前 8KB，含 null 字节 → 标记
    │     - 检测 >10MB: metadata.len() > 10_485_760
    │     - 打印合并警告 → stdin 读确认
    │     - 用户选 n → return Ok(None)
    │
    ├─ 3. 创建 tempdir() staging
    │     - 创建 staging/files/ + staging/artifacts/
    │     - FileEntry → fs::copy 到 staging/files/<zip_path>
    │     - ArtifactEntry → fs::write 到 staging/artifacts/<filename>
    │     - serde_json::to_string_pretty(manifest) → staging/manifest.json
    │
    ├─ 4. create_zip_archive(staging, output)
    │     ┌─ Windows ─→ powershell -NoProfile -Command Compress-Archive -Path "<staging>\*" -DestinationPath "<output>" -Force
    │     └─ Unix ────→ zip -r "<output>" .
    │     - 检查 exit code ≠ 0 → bail with stderr
    │
    ├─ 5. mv output 到目标路径
    │     - fs::rename(staging_zip, dest) → Ok
    │     - Err → fs::copy + fs::remove_file (跨卷 fallback)
    │
    └─ 6. 清理: tempdir drop 自动删除 staging
        返回 Ok(Some(dest_path))
```

### 文件名 sanitize

Ranged 文件切片和 cmd artifact 的文件名包含原始路径/命令，需要 sanitize：

```rust
fn sanitize_filename(raw: &str) -> String {
    raw.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '], "-")
        .trim_matches('-')
        .to_string()
}
```

- `file:src/main.rs:10-20` → `file-0-src-main.rs-L10-20.txt`
- `cmd:git diff HEAD~1` → `cmd-0-git-diff-HEAD-1.txt`

### 二进制检测 heuristic

```rust
const BINARY_CHECK_BYTES: usize = 8192;

fn is_binary(path: &Path) -> Result<bool> {
    let mut buf = vec![0u8; BINARY_CHECK_BYTES];
    let mut f = File::open(path)?;
    let n = f.read(&mut buf)?;
    Ok(buf[..n].contains(&0))
}
```

### 文件去重逻辑

```rust
fn dedup_files(entries: Vec<FileEntry>) -> Vec<FileEntry> {
    let mut seen = HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.zip_path.clone())).collect()
}
```

去重 key = `zip_path`（即 `files/<relative_path>`），同一文件被多个 source 引用时保留第一个。

## Outcome Preview

### 正常流程

```
gather> file:src/main.rs
  ✓ Added: file:src/main.rs
gather> file:src/main.rs:10-20
  ✓ Added: file:src/main.rs:10-20
gather> dir:src/builtins
  ✓ Added: dir:src/builtins
gather> cmd:git diff
cmd body> git diff HEAD~1
  ✓ Added: cmd:git diff HEAD~1
gather> tree:src
  ✓ Added: tree:src
gather> /zip
  Collecting 15 files, 1 command, 1 tree, 1 ranged file...
  ✓ Zip written: H:\SrcCode\playground\agent-squire\asq-gather-20260630T004014.zip (1.2 MB)
gather>
```

### 带警告流程

```
gather> /zip
  Collecting 18 files, 1 command...
  ⚠ Warnings:
    2 binary files detected:
      - assets/logo.png (245 KB)
      - data/model.onnx (52 MB) ← also exceeds 10 MB
    1 large file (>10 MB):
      - logs/debug.log (15 MB)
  Continue? [Y/n]: y
  ✓ Zip written: H:\SrcCode\playground\agent-squire\asq-gather-20260630T004014.zip (53 MB)
gather>
```

### 无文件类 Source

```
gather> cmd:echo hello
  ✓ Added: cmd:echo hello
gather> /zip
  error: No file sources to package. Use /list to review.
gather>
```

### 空 session

```
gather> /zip
  error: No sources to package. Use `file:`, `dir:`, `glob:`, `tree:`, or `cmd:` to add sources.
gather>
```
