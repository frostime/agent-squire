---
name: smart-indent
created: 2026-06-14T22:54:10
---

# smart-indent — Design

## Behavioral Spec

### Matching Chain (with `--smart-indent` off)

```
exact → loose → indent_shift(报告 only)
```

| Stage | Match found? | Outcome |
|-------|-------------|---------|
| exact | yes → | `match_mode: "exact"`, 正常替换 |
| exact → loose | yes → | `match_mode: "loose"`, 正常替换 |
| exact → loose → indent_shift | exactly 1 → | `status: "indent_mismatch"`, `error` 含 indent_delta 信息和 `--smart-indent` 提示 |
| exact → loose → indent_shift | >1 → | `status: "search_indent_ambiguous"`, `related_lines` 列出所有匹配位置 |
| exact → loose → indent_shift | 0 → | `status: "search_not_found"` (现状不变) |

### Matching Chain (with `--smart-indent` on)

```
exact → loose → indent_shift(应用)
```

| Stage | Match found? | Outcome |
|-------|-------------|---------|
| exact → loose | yes → | 正常替换（不变） |
| exact → loose → indent_shift | exactly 1 → | `match_mode: "indent_shift"`, `indent_delta` 附加到 replace |
| exact → loose → indent_shift | >1 → | `status: "search_indent_ambiguous"`, 报错 |
| exact → loose → indent_shift | 0 → | `status: "search_not_found"` |

### Common Indent Prefix Computation

```
fn compute_common_indent_prefix(lines: &[String]) -> Option<String>
```

- 仅非空行（去除行尾后还有内容的行）参与计算
- 每个非空行提取前导空白（空格/tab 混合视为一个完整单位）
- 取所有非空行前导空白的最长公共前缀
- 空 search → 返回 None
- 全部行空 → 返回 None
- 公共前缀为空串 → 返回 None（无缩进差异）

示例：
```
lines: ["    fn foo() {", "        42", "    }"]
non_empty: ["    fn foo() {", "        42", "    }"]
prefixes: ["    ", "        ", "    "]
common: "    "  → Some("    ")

lines: ["fn foo() {", "    42", "}"]
prefixes: ["", "    ", ""]
common: ""  → None
```

### Indent-Shift Match Algorithm

```
fn find_indent_matches(region: &[String], needle: &[String]) -> (Vec<usize>, Option<String>)
```

1. `delta = compute_common_indent_prefix(needle)`
2. 若 delta 为 None → 返回空
3. 从 needle 每行去除 delta：空行保留原样不去除、行尾保留
4. 用去除 delta 后的 needle 调用 `find_block_matches(region, stripped_needle, loose=false)`（exact 匹配，不含 indent 修正本身再用 loose）
5. 返回 (匹配位置列表, Some(delta))

### Indent-Shift Replace Adjustment

```
fn adjust_replace_lines(replace_lines: &[String], delta: &str) -> Vec<String>
```

- 对 replace 每行：去除行尾后的内容非空 → 在行首插入 `delta`
- 空行（去除行尾后为空）保持原样不动
- 行尾保留原样

### Already-Applied Check

`--smart-indent` 开启时，already_applied 检查（"REPLACE 是否已存在于文件"）仍然走 exact/loose 路径，不做 indent-shift 调整。
原因：replace 串以省略缩进的形式写入，不可能在文件中找到原文匹配。

## Interface Contract

### CLI

```
asq patch-edit [--smart-indent] ...
```

- `--smart-indent`: boolean flag, default false
- 无 .short option（非高频使用）

### New Status Values

| Status | Context | Meaning |
|--------|---------|---------|
| `indent_mismatch` | `--smart-indent` off | search 加一致缩进后可匹配，但未开启 flag |
| `search_indent_ambiguous` | any | search 加一致缩进后多处匹配 |
| `indent_shift` (match_mode) | `--smart-indent` on | 匹配成功，使用了 indent-shift 调整 |

### PatchApplyResult JSON additions

```json
{
  "indent_delta": "    ",
  "error": "SEARCH content matches with indent prefix \"    \" but not as-is. Use --smart-indent to apply with auto-indent"
}
```

## Code Change Assessment

按文件梳理改动量、接口变更、算法流程。

### 1. `model.rs` — 小改

**已改**: `PatchMatch` 和 `PatchApplyResult` 各加 `indent_delta: Option<String>` 字段。

**接口变更**: `PatchApplyResult` 是 `pub` 的（serde 序列化），加字段是 additive change，向后兼容。

**后续必须改**: 所有构造 `PatchApplyResult` / `PatchMatch` 的地方必须加 `indent_delta: None` 或 `Some(...)`。

### 2. `text.rs` — 小改

**新增两个纯函数**:

```
pub fn compute_common_indent_prefix(lines: &[String]) -> Option<String>
```
- 输入: 带行尾的行数组（`split_lines_keepends` 产出的格式）
- 仅对非空行提取前导空白，返回最长公共前缀
- 公共前缀为空串时返回 None

```
pub fn adjust_line_indent(lines: &[String], delta: &str) -> Vec<String>
```
- 输入: 带行尾的行数组 + 缩进 delta
- 非空行行首插入 delta，空行不变
- 返回新的行数组

无接口变更，纯新增。

### 3. `match_apply.rs` — 核心改动

**接口变更**:

| 函数 | 当前签名 | 新签名 |
|------|---------|--------|
| `apply_patches` | `(patch_text, project_root, dry_run)` | `(patch_text, project_root, dry_run, smart_indent: bool)` |
| `apply_parsed_patches` | `(&[PatchBlock], dry_run)` | `(&[PatchBlock], dry_run, smart_indent: bool)` |
| `apply_patch` | `(&PatchBlock, dry_run)` | `(&PatchBlock, dry_run, smart_indent)` |
| `apply_patch_inner` | `(&PatchBlock, dry_run)` | `(&PatchBlock, dry_run, smart_indent)` |
| `apply_search_patches_batch` | `(&[PatchBlock], dry_run)` | `(&[PatchBlock], dry_run, smart_indent)` |
| `match_patch` | `(&PatchBlock, &[String], &str, &str)` | `(&PatchBlock, &[String], &str, &str, smart_indent: bool)` |

**新增函数**:

```
fn find_indent_matches(region: &[String], needle: &[String]) -> (Vec<usize>, Option<String>)
```
- 调用 `compute_common_indent_prefix` → 若有 delta → strip delta from needle → `find_block_matches(region, stripped, false)`

**核心算法流程变更 — `match_patch` 函数**:

当前 `match_patch` 的流程后段（exact → loose 匹配后）:
```
find_preferred_matches → 1 match / >1 match / 0 match → 进一步 replace 检查 → 最终状态
```

新流程:
```
find_preferred_matches → 1 match / >1 match → (不变)
                    → 0 match → find_indent_matches → 1 delta match → smart_indent?
                                                                    yes → matched (indent_shift) + 记录 delta
                                                                    no  → indent_mismatch (报告)
                                                         >1 delta match → search_indent_ambiguous
                                                         0 delta match → search_not_found (不变)
```

后面 already_applied 检查 `find_preferred_matches(&region, &replace_lines)` 不变（不做 indent-shift）。

**indent_shift 匹配生效后的替换逻辑**:

`apply_patch_inner` 和 `apply_search_patches_batch` 中现有：
```rust
let replace_lines = split_lines_keepends(&convert_newlines(&patch.replace_content, newline));
```

需要在 `matched.match_mode == Some("indent_shift")` 时，改为：
```rust
let replace_lines = adjust_line_indent(&replace_lines, delta);
```

**`base_result` 函数**:

签名加 `indent_delta: Option<String>` 参数。所有调用点（18处）需补 `None`，涉及 indent_shift 的 2-3 处改 `Some(delta.clone())`。

**`fail` 闭包**在 `match_patch` 内部:

构造 `PatchMatch` 加 `indent_delta: None`。indent_mismatch 和 search_indent_ambiguous 场景设 `indent_delta: Some(delta)`。

**`match_to_result` 函数**:

加 `result.indent_delta = m.indent_delta.clone()`。

**`apply_search_patches_batch` 批量检查**:

现有:
```rust
matches.iter().any(|m| !matches!(m.status.as_str(), "matched" | "already_applied" | "no_change_patch"))
```

`indent_mismatch` 状态（smart_indent off 时）不是 `matched/already_applied/no_change_patch`，会进入失败分支 → 正确拒写。`indent_shift`（smart_indent on）是 `matched` → 正常走批量逻辑。故**无需改动这个判断**。

但批量 apply 替换时需要对 `indent_shift` 匹配做 replace 缩进调整：
```rust
for m in &matched {
    let replace_lines = split_lines_keepends(&convert_newlines(&m.patch.replace_content, newline));
    // 需要判断:
    let replace_lines = if m.match_mode.as_deref() == Some("indent_shift") {
        adjust_line_indent(&replace_lines, m.indent_delta.as_deref().unwrap())
    } else {
        replace_lines
    };
    new_lines.splice(m.abs_start..m.abs_end, replace_lines);
}
```

**`apply_patch_inner` 中** `matched` 为 `indent_shift` 时同样需要调整 replace_lines。

### 4. `mod.rs` — 小改

**接口变更**: `PatchEditArgs` 增加 `--smart-indent` flag。

```rust
#[arg(long, help = "Enable smart indent matching for SEARCH blocks")]
pub smart_indent: bool,
```

**调用链打通**: `run` → `apply_patches(text, &ctx.cwd, dry_run, args.smart_indent)`，及 `run_interactive` → `apply_patches` 调用。

`run_once` 签名改为 `fn run_once(patch_text: &str, dry_run: bool, smart_indent: bool, ctx: &CommandContext)`。

### 5. `output.rs` — 小改

**`print_compact`**: 对 `indent_mismatch` 和 `search_indent_ambiguous` 状态增加友好展示（与现有状态同格式）。

**`print_json`**: 无需额外改动，`PatchApplyResult` 上 `indent_delta` 已加 `Serialize` 字段。

**`match_to_result` → `base_result`**: `indent_delta` 已在 `PatchApplyResult` 结构体内，JSON 自动序列化。

### 6. `tests/patch_edit_compat.rs` — 新增测试

所有现有测试需补走：`apply_patches` 签名加了 `smart_indent` 参数。

**新增测试用例**:

| Test | What |
|------|------|
| `indent_mismatch_without_flag` | search 缺 4空格缩进 → `indent_mismatch` + error 含前缀信息 |
| `smart_indent_applies_with_adjustment` | 加 `--smart-indent` → `indent_shift` + match_mode + replace 行有缩进 |
| `smart_indent_empty_lines_preserved` | search 含空行 → 空行不参与前缀计算、replace 空行不加缩进 |
| `smart_indent_tab_prefix` | tab 前缀 |
| `search_indent_ambiguous` | search 加缩进后多处匹配 → `search_indent_ambiguous` |
| `smart_indent_no_delta_needed` | search 已正确缩进 → 不触发 indent_shift，走 exact/loose |
| `smart_indent_mixed_whitespace_no_common` | 行前缀不一致（tab vs spaces）→ 无公共前缀 → fallback search_not_found |

### 总改动量估算

| File | Lines Added/Changed | Complexity |
|------|---------------------|---------|
| `model.rs` | ~4 (2 fields + existing `indent_delta` in PatchMatch) | Low |
| `text.rs` | ~50 (2 new functions) | Medium (algorithm) |
| `match_apply.rs` | ~80 (signatures + flow + find_indent_matches + replace adjust) | High (core flow change) |
| `mod.rs` | ~10 (1 arg + 3 call sites) | Low |
| `output.rs` | ~5 (2 status names in compact) | Low |
| `tests/patch_edit_compat.rs` | ~120 (7 tests + fix existing 12 call sites) | Medium |

**Total**: ~270 lines, Mostly additive.

## Outcome Preview

### Without --smart-indent (error case)

```
[X] indent_mismatch   # src/main.rs -- SEARCH content matches with indent prefix "    " but not as-is. Use --smart-indent to apply with auto-indent
[X] 1 patch(es) failed.
```

### With --smart-indent (success case)

```
[OK] applied           # src/main.rs -- (indent_shift @L5)
```