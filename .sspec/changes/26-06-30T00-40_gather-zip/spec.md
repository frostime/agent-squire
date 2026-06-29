---
name: gather-zip
status: PLANNING
change-type: single
created: 2026-06-30T00:40:14
reference: null
---

# gather-zip

## Problem Statement

`asq gather -i` 交互模式能收集文件、目录、glob、命令输出等 Source，但当前只能渲染为 Markdown 文本。用户无法将收集到的文件集合打包为可移植的 zip 资产直接交给 AI Agent。

## Proposed Solution

### Approach

在 gather 交互模式中新增 `/zip` 命令。执行时将当前 session 已添加的 Source 解析为实际文件 + 文本产物，按结构化目录打包为 zip：

- `files/` — 实体文件（保持原相对路径），去重
- `artifacts/` — 文本产物（ranged 文件切片、cmd 输出、tree 输出）
- `manifest.json` — 机器可读的 source 索引

使用外部 CLI 创建 zip（Windows: `powershell Compress-Archive`，Unix: `zip -r`），构建在 temp 目录中，完成后 mv 到 cwd。

选择结构化打包而非纯文本 zip 的理由：Agent 需要直接操作文件（读取/编辑），而非解析 markdown fenced block。

### Behavior Contract

**BC-1: `/zip` 交互命令**

- 在 gather 交互循环中新增 `/zip` 命令
- 输入 `/zip` → 使用默认文件名 `asq-gather-<timestamp>.zip` 输出到 cwd
- 输入 `/zip <path>` → 输出到指定路径
- 输入 `/zip /done` 或 `/zip --done` → 打包后退出交互循环
- 当前 session 无任何 Source → 输出错误 "No sources to package"，不创建 zip
- 当前 session 无文件类 Source（仅有 cmd/tree） → 输出错误 "No file sources to package"

**BC-2: Zip 内部结构**

```
<name>.zip
├── files/                         ← 实体文件，保持相对路径，去重
│   └── src/main.rs
├── artifacts/                     ← 文本产物
│   ├── cmd-0-<sanitized-command>.txt
│   ├── tree-0-<path>.txt
│   └── file-0-src-main.rs-L10-20.txt   ← ranged 切片
└── manifest.json                  ← 唯一索引
```

- 文件路径使用 `/` 分隔符（zip 内部规范）
- 同一文件被多个 Source 引用时，`files/` 中仅存一份；manifest 中多条目指向同一 `inZip`
- cwd 之外的文件（绝对路径或 `../`）允许包含，但 flatten 到安全文件名并记录警告

**BC-3: 警告确认 UX**

- 二进制文件：检测到则列出文件路径和大小，要求用户确认
- 单文件 >10MB：检测到则列出，要求用户确认
- 两种警告合并为一次提示：`Continue? [Y/n]:`
- 用户选择 `n` 或 Ctrl+C → 取消打包，回到交互循环

**BC-4: 输出位置**

- zip 在 temp 目录中构建（用 `tempfile::tempdir()`）
- 构建完成后 rename 到 cwd（或用户指定路径）
- 跨卷 rename 失败时 fallback 到 copy + delete
- temp 目录在 drop 时自动清理

**BC-5: 错误处理**

- 外部 zip 工具未找到 → 清晰错误信息："zip not found. Install zip (Unix) or use PowerShell (Windows)."
- zip 工具执行失败 → 报告 exit code + stderr
- 输出路径已存在 → 错误 "output file exists: <path>"（不覆盖，需用户自行处理）
- `--no-gitignore` / `/all` 状态：zip 使用当前 session 已 resolve 的 sources，不在打包时重新 expand

### Implementation Changes

- **feat(gather): Add `/zip` interactive command**
  - 修改 `InteractiveCommand` enum，新增 `Zip` variant
  - 修改 `parse_interactive_command()` 识别 `/zip` 及参数
  - 修改 `read_sources()` 主循环，处理 Zip 命令

- **feat(gather): Implement zip assembly pipeline**
  - 新建 `src/builtins/gather/zip.rs`
  - 实现 `assemble_zip(sources, cwd, respect_gitignore) -> Result<PathBuf>`
  - 文件收集：去重、resolve 路径、复制到 staging temp dir
  - Ranged 切片：读取原文件 → 截取行范围 → 写入 `artifacts/`
  - Cmd 执行：复用 compose `render_program` 或直接 `std::process::Command`
  - Tree 生成：复用 `sources::render_tree()`
  - 调用外部 CLI 打包 staging dir → 输出 zip
  - mv zip 到 cwd，处理跨卷 fallback

- **feat(gather): Generate manifest.json**
  - 定义 `Manifest` / `ManifestEntry` 结构体（serde Serialize）
  - 在 zip 组装过程中构建 manifest，写入 `manifest.json`
  - Schema 字段：type, path/command, range, inZip, fileCount, files

- **feat(gather): Add file safety checks**
  - 二进制检测：检查文件前 8KB 是否含 null 字节（heuristic）
  - 大文件检测：`fs::metadata().len() > 10 * 1024 * 1024`
  - 外部路径检测：路径以 `..` 开头或是绝对路径

### Scope Summary

| File | Change | Effort |
|---|---|---|
| `src/builtins/gather/interactive.rs` | 新增 `Zip` variant + 参数解析 + 主循环处理 | S |
| `src/builtins/gather/zip.rs` (new) | zip 组装管线、文件收集、manifest 生成、外部 CLI 调用 | M |
| `src/builtins/gather/mod.rs` | 声明 `mod zip;` | XS |
| `src/builtins/gather/model.rs` | 可能新增 zip 相关辅助类型 | XS |
| `tests/gather.rs` | 新增 `/zip` 集成测试 | M |

### Design Reference

See [design.md](./design.md)
