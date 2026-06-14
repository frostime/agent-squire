---
name: smart-indent
status: DOING
change-type: single
created: 2026-06-14T22:54:10
reference: null
---

# smart-indent

## Problem Statement

`asq patch` 的 SEARCH 块因缩进差异匹配失败时，统一返回 `search_not_found`。用户无法区分"内容对但缩进前缀统一缺少"和"内容完全对不上"——前者是可修复的，后者才是真正的匹配失败。

## Proposed Solution

### Approach

在现有匹配链路（exact → loose）之后增加一级 indent-shift 匹配：提取 search 串各非空行的公共空白前缀作为 `indent_delta`，去除后重新匹配。不加 `--smart-indent` 时，匹配结果以 `indent_mismatch` / `search_indent_ambiguous` 报错并附 indent_delta 信息；加 `--smart-indent` 时自动修正匹配并对 replace 内容做对应缩进调整。

### Key Change

- **Feat: indent-shift 匹配逻辑** — `match_apply.rs` 中 `find_preferred_matches` 之后增加 `find_indent_matches`，计算公共前缀并尝试去除后匹配；返回新状态 `indent_mismatch` / `search_indent_ambiguous` / `indent_shift`
- **Feat: `--smart-indent` CLI flag** — `PatchEditArgs` 增加 `smart_indent: bool`，传入 `apply_patches` → `match_patch`；启用时 `indent_shift` 匹配成功，replace 内容加 `indent_delta`
- **Feat: replace 缩进调整** — 匹配成功后对 replace 各行（含空行）右移 `indent_delta`
- **Feat: `indent_delta` 字段** — `PatchMatch` 和 `PatchApplyResult` 增加 `indent_delta: Option<String>`，JSON 输出可见
- **Feat: 错误提示含缩进前缀** — `indent_mismatch` 错误消息展示缺少的缩进前缀（可读 repr），提示使用 `--smart-indent`
- **Refactor: 空行处理** — indent-shift 匹配时空行不参与公共前缀计算，匹配时空行视为匹配任意缩进的行

### Scope Summary

| File | Change |
|------|--------|
| `src/builtins/patch_edit/model.rs` | `PatchMatch` + `PatchApplyResult` 加 `indent_delta` 字段 |
| `src/builtins/patch_edit/text.rs` | 新增 `compute_common_indent_prefix` + `adjust_line_indent` |
| `src/builtins/patch_edit/match_apply.rs` | `find_indent_matches` + 修改 `match_patch` 链路 + replace 缩进调整 |
| `src/builtins/patch_edit/mod.rs` | CLI `--smart-indent` flag + thread 到调用链 |
| `src/builtins/patch_edit/output.rs` | `indent_mismatch` / `search_indent_ambiguous` 输出 |
| `tests/patch_edit_compat.rs` | 新增 indent-shift 相关集成测试 |

### Design Reference

→ See [design.md](./design.md)