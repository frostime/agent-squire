# Handover — 2026-06-13 gather 命令设计

## 1. Current Status
🟡 **设计阶段完成，待用户确认后进入规划阶段**

已完成澄清和设计两个阶段，spec.md 和 design.md 已填写完整。用户已确认设计规范的完整性，但尚未正式确认进入规划阶段。

## 2. Task Context

用户希望在 agent-squire CLI 中增加一个类似 VS Code 扩展 "vscode-files-to-prompt" 的 `gather` 命令，用于将多个文件、代码片段、目录树、命令行结果组装成一个 Prompt 文件，方便发送给 LLM。

核心需求：
- CLI flags 和交互式两种使用模式
- 交互式模式支持 fzf 补全
- 输出格式使用自定义分隔符（非 MD，避免与内容混淆）
- 基于现有 compose 命令实现（生成模板后委托 compose 渲染）

## 3. What Was Done

- **澄清阶段**：通过多轮对话确定了需求
  - 命令名称：`gather`
  - 输入语法：前缀标记法（`file:path`, `dir:path`, `tree:path`, `glob:pattern`, `cmd:command`）
  - 输出格式：`====== FILE-START: path ======` 格式
  - 交互式模式：`--interactive` 触发，Tab 触发 fzf 补全，Ctrl+D 结束

- **设计阶段**：创建了完整的规范文档
  - `sspec change new gather --scaffold design` 创建了 change
  - `spec.md`：问题陈述、方案、关键变更、范围
  - `design.md`：接口契约、解析规则、交互式模式、输出格式、错误处理、完整示例

- **关键规范细节已补充**：
  - 冒号后空格裁剪规则
  - 内容可包含冒号的解析规则
  - 行范围语法（`file:path:start-end`）
  - 自动检测规则（无前缀时）
  - CLI 参数引号处理
  - 来源顺序处理

## 4. Key Decisions & Reasoning

- **决策**：基于 compose 实现，不重复造轮子
  - 原因：compose 已有完整的模板渲染、源解析、错误处理能力，gather 只需生成模板
  - 替代方案：独立实现渲染引擎（复杂度高，维护成本大）

- **决策**：使用自定义分隔符格式（非 MD/XML）
  - 原因：当内容本身是 Markdown 时，MD 格式无法区分边界；用户明确要求避免混淆
  - 替代方案：XML + CDATA（结构化强但可能与 XML 内容混淆）

- **决策**：前缀标记法（`file:path`）而非空格分隔
  - 原因：空格分隔在内容含空格时解析困难；用户倾向前缀法
  - 替代方案：智能检测（误判风险高）

- **决策**：冒号后裁剪前导空格
  - 原因：用户友好的输入习惯（`file: path` 比 `file:path` 更自然）
  - 用户明确确认此需求

- **决策**：行范围使用 `file:path:start-end` 语法
  - 原因：复用 file 前缀，无需额外 snippet 类型
  - 用户明确确认此需求

## 5. Known Issues & Pitfalls

- **FZF 依赖**：交互式模式硬依赖 fzf，需检测并给出友好错误
- **Windows 路径**：`file:C:\Users\test.rs` 中的冒号需正确处理（解析规则已覆盖）
- **行范围解析**：需找最后一个符合 `<number>-<number>` 模式的冒号，避免与路径中的冒号混淆
- **tree 命令**：Windows 可能没有 `tree` 命令，需 fallback 到 `find`

## 6. Next Steps

1. **等待用户正式确认**：确认设计后进入规划阶段
2. **执行 `sspec-plan`**：将设计拆解为具体任务（tasks.md）
3. **执行 `sspec-implement`**：按任务逐步实现
   - 注册 CLI 命令（`src/cli.rs`）
   - 创建 `src/builtins/gather/` 模块
   - 实现解析器（`parser.rs`）
   - 实现交互式模式（`interactive.rs`）
   - 实现模板生成（`template.rs`）
   - 集成 compose（`output.rs`）
   - 编写测试（`tests/gather.rs`）
   - 更新文档（README.md, CHANGELOG.md）

## 7. Relevant Files & References

- `.sspec/changes/26-06-13T18-06_gather/spec.md` — 变更规范
- `.sspec/changes/26-06-13T18-06_gather/design.md` — 技术设计（已完善，包含所有实现细节）
- `.sspec/changes/26-06-13T18-06_gather/reference/handover.md` — 本文档
- `.sspec/tmp/26-06-13T17-30_clarify_files-to-prompt.md` — 澄清阶段笔记
- `.sspec/changes/26-06-10T01-22_compose/design.md` — compose 命令设计（gather 的依赖）
- `.sspec/spec-docs/compose-template-engine.md` — compose 模板引擎规范
- `H:\SrcCode\开源项目\vscode-files-to-prompt\README.md` — 用户参考的 VS Code 扩展
