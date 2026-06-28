---
change: "rearange"
updated: "2026-06-28"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

> 实现阶段先读 SKILL: `architecture-design`(模块边界/复杂度取舍)、`write-self-explain-code`(命名/结构自解释)。用 `view-skill` 检索。
> 全程: `cargo test` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo fmt`。

## Tasks

### Phase 1: 骨架 + 模型 + 文本 IO ⏳
- [ ] 新建 `src/builtins/rearrange/mod.rs`:`RearrangeArgs`(clap)、`run`、`--prompt` 占位 `src/builtins/rearrange/mod.rs`
- [ ] `src/builtins/mod.rs` 挂 `pub mod rearrange;`
- [ ] `src/cli.rs` 加 `CliCommand::Rearrange(...)` + dispatch(`name="rearrange"`,考虑 alias)
- [ ] 定义 `model.rs` 类型:`Spec/ChunkDef/Action/Region/Anchor/Gap/Plan`(见 design §3)`src/builtins/rearrange/model.rs`
- [ ] `textio.rs`:读(编码/换行/末尾换行检测+拆逻辑行)+ 原子写(见 design §5)`src/builtins/rearrange/textio.rs`

**Verification**:
- Agent: `cargo build` 通过;`asq rearrange --prompt` 打印占位文本;`asq rearrange --help` 列出 flags。

### Phase 2: DSL 解析器 ⏳
- [ ] `parser.rs`:解析 `file` / `chunk` / 四种 action / target / `gap=`,产出 `Spec`(见 design §2)`src/builtins/rearrange/parser.rs`
- [ ] 单 action 约束:>1 action → `MULTIPLE_ACTIONS`;缺/多 `file` → `INVALID_SPEC`
- [ ] 内联区间与 chunk 名两种 `Region` 解析
- [ ] in-module 单测:各 action 正常解析 + 语法错误码

**Verification**:
- Agent: `cargo test rearrange::parser` 通过(正常 spec → 正确 `Spec`;非法 spec → 对应错误码)。

### Phase 3: Planner(resolve + validate + materialize)⏳
- [ ] `plan.rs`:resolve(chunk 名/内联 → 行索引,读文件)`src/builtins/rearrange/plan.rs`
- [ ] validate:越界 / 重叠 / 集合不一致 / 锚点越界 / 锚点落在被移动区间内(见 design §4 + §7 错误码)
- [ ] materialize:move/copy/delete(§4.2-4.3)+ rearrange slot/drop/error(§4.4),全部基于原始行数组
- [ ] in-module 单测:§4.4 case3 物理槽位结果 `B,hidden1,D,C,hidden2,A`

**Verification**:
- Agent: `cargo test rearrange::plan` 通过;含 gap=slot/drop/error 三分支与各错误码用例。

### Phase 4: 输出 + --prompt + 集成测试 ⏳
- [ ] `output.rs`:compact(chunk/action/gap 摘要 + unified diff + 状态行,见 Mock A/C/D/E)+ json(`Envelope`,Mock F)`src/builtins/rearrange/output.rs`
- [ ] 补全 `--prompt`:DSL + CLI 用法 + 安全提示(design §1,仿 `patch_edit::PATCH_PROMPT`)
- [ ] `run` 串联:读源 → parse → plan → 输出;无 `--yes` 不写,`--yes` 经 `textio` 原子写
- [ ] `tests/rearrange.rs`:RFC case 1-5 + newline 保持 + MULTIPLE_ACTIONS(见 design §8)

**Verification**:
- Agent: `cargo test`(含 `tests/rearrange.rs`)全绿;`cargo clippy ... -D warnings` 无告警;`cargo fmt --check` 干净。

**User Check**:
1. BC-1: `asq rearrange --stdin < spec`(无 `--yes`)→ 打印预览+diff,文件**不变**;加 `--yes` → 文件改写。
2. BC-2: Mock C 的 `rearrange A,B,C,D => B,D,C,A gap=slot` → 结果为 `B,hidden1,D,C,hidden2,A`。
3. BC-3: 对 CRLF 文件执行 move 后,文件仍为 CRLF。
4. BC-4: 重叠 chunk → 退出码 1,`OVERLAPPING_CHUNKS`,文件未改。

---

## Progress

**Overall**: 0%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 骨架/模型/IO | 0% | ⏳ |
| Phase 2 解析器 | 0% | ⏳ |
| Phase 3 Planner | 0% | ⏳ |
| Phase 4 输出/测试 | 0% | ⏳ |

**Recent**:
- (none yet)
