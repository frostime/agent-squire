---
name: rearange
status: PLANNING
change-type: single
created: 2026-06-28 22:20:29
reference:
- source: .sspec/requests/26-06-28T18-48_rearange.md
  type: request
  note: Linked from request
---

# rearange

## Problem Statement

Agent 用 `asq patch-edit` 做"大块移动 / 多块重排"时三类摩擦:
1. `move` 必须写一对 SEARCH(删)+ SEARCH(插)块,且 SEARCH 需复制整段原文 —— 块越大越冗长,越易因复制不全而匹配失败。
2. "重排"本质是删除 + 插入,patch 语义不直接;多块重排时 SEARCH 区间易重叠,导致整批失败或误删。
3. Agent 想表达的只是"把第 X-Y 行挪到 Z 之后",却被迫处理底层文本匹配。

**User need**: 一个**按 1-based 行区间声明、按位置操作**的命令,把行号计算 / 换行 / 编码细节吸收进 CLI;Agent 只声明 chunk 行区间 + 一个动作。

## Proposed Solution

### Approach

新增 `asq rearrange`:输入一段 **DSL**(`file` 指令 + 可选 `chunk` 声明 + **恰好一条 action**),CLI 解析行区间、在**原始文件快照坐标系**下计算结果、生成 diff、`--yes` 时原子写回。

核心动作:
- `rearrange`(卖点):同文件内按槽位置换多个 chunk 的内容,槽位间的未声明行(gap)默认留在原位(`gap=slot`)。
- `move` / `copy` / `delete`:对单个行区间的搬移 / 复制 / 删除,行区间可内联书写,无需命名。

相比 RFC 原稿,v1 **大幅瘦身**(经 clarify 决策):单文件、每次恰好一条 action、仅 DSL、裸数字行号(对齐 `read-range`)、无 backup/journal/rollback、无 FILE_CHANGED 快照检查。砍掉的能力(JSON 输入、跨文件、事务子系统)推 v2。

为何独立命令而非扩展 `patch-edit`:`patch-edit` 是内容匹配模型(SEARCH/REPLACE),`rearrange` 是行坐标模型,两者输入契约与失败模式不同,混入同一命令会污染 patch 的语义。

详见 [design.md](./design.md)。

### Behavior Contract

**BC-1 命令表面与安全门**
- Surface: CLI 新命令 `asq rearrange [SPEC]`,输入源 `--stdin` / `-f <PATH>` / 位置参数;全局 `--cwd` / `--print` / `--json` 继承。
- 默认 **dry-run**:无 `--yes` 时只解析、校验、打印预览 + diff,**绝不写文件**。
- `--yes` 才写入。`--prompt` 打印 DSL 指南。
- Boundary: 任何已有命令 / flag 行为不变(向后兼容)。

**BC-2 操作语义(单文件,原始坐标系)**
- 行区间 1-based inclusive,`A-B` 或单行 `N`,裸数字(无 `L` 前缀)。
- 所有 chunk 行区间与 anchor 均按**操作前的原始文件**解释,不受同一 spec 内其它效果影响(本 v1 每次仅一条 action,坐标污染天然不存在)。
- anchor: `start` / `end` / `before N` / `after N`。
- `rearrange` gap 策略:`slot`(默认,gap 留原槽位间)/ `drop`(丢弃 gap)/ `error`(有 gap 即失败)。span 外(首槽之前、末槽之后)的行永不改动。
- `move`/`copy`/`delete` 作用于单个行区间。

**BC-3 文本保真**
- 写回保持原文件换行风格(CRLF/LF)与编码(UTF-8 / UTF-8-BOM / GBK / Windows-1252),及末尾换行有无。

**BC-4 失败行为**
- 校验失败一律**不写任何文件**。
- 错误经 stderr / 非零退出码暴露;JSON 模式在 `meta.error_code` 给结构化码:`INVALID_SPEC` `UNKNOWN_CHUNK` `INVALID_RANGE` `RANGE_OUT_OF_BOUNDS` `OVERLAPPING_CHUNKS` `ANCHOR_OUT_OF_BOUNDS` `ANCHOR_INSIDE_MOVED_CHUNK` `REARRANGE_SET_MISMATCH` `FILE_NOT_FOUND` `MULTIPLE_ACTIONS`。
- 目标文件不存在 → `FILE_NOT_FOUND`(不创建)。

**BC-5 输出**
- compact:chunk 摘要 + action 摘要 + gap 提示 + unified diff + dry-run/written 状态行。
- json:`Envelope<T>` 形状(`ok/command/data/warnings/meta`),`data` 含 file/chunks/action/written/diff。

### Implementation Changes

- **feat(cli): 注册 `rearrange` 子命令** — `src/cli.rs` 加 `CliCommand::Rearrange`,`src/builtins/mod.rs` 挂模块。服务 BC-1。
- **feat(rearrange): DSL 解析器** — `file` / `chunk` / 四种 action / target / gap 的纯文本解析,单 action 约束(>1 → `MULTIPLE_ACTIONS`)。服务 BC-1/BC-2/BC-4。
- **feat(rearrange): planner** — resolve(读文件、切行)+ validate(越界/重叠/集合/锚点)+ materialize(原坐标下生成新内容)。服务 BC-2/BC-4。
- **feat(rearrange): 文本 IO** — 换行/编码检测与保持的读写(自包含模块,复用 `encoding_rs`,原子写)。服务 BC-3。
- **feat(rearrange): 输出** — compact + json + unified diff 渲染。服务 BC-5。

### Scope Summary

| File | Change | Effort |
|------|--------|--------|
| `src/cli.rs` | 注册子命令 + dispatch | XS |
| `src/builtins/mod.rs` | 挂 `rearrange` 模块 | XS |
| `src/builtins/rearrange/mod.rs` | Args / run / `--prompt` 指南 | M |
| `src/builtins/rearrange/parser.rs` | DSL 解析 | M |
| `src/builtins/rearrange/model.rs` | Spec/Chunk/Action/Anchor/Plan 类型 | S |
| `src/builtins/rearrange/plan.rs` | resolve + validate + materialize | L |
| `src/builtins/rearrange/textio.rs` | 换行/编码保持读写 | S |
| `src/builtins/rearrange/output.rs` | compact/json/diff 渲染 | S |
| `tests/rearrange.rs` | 集成测试覆盖 RFC case 1-5 | M |

#### What Stays Unchanged
- `patch-edit` / `read-range` 等现有命令的代码与行为不动(textio 自包含,不重构 `patch_edit/io.rs`)。
- 不引入 backup 目录、journal、`.asq/` 写入。

### Design Reference

See [design.md](./design.md)
