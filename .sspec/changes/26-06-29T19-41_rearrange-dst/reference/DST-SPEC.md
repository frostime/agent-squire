# Arrange 状态转换 DSL 行为规格草案

状态：**草案,等待人工审阅**。

本文定义 Arrange DSL 的行为模型。本文不定义 CLI 参数、Rust 实现、JSON 输出、错误码名称、dry-run、原子写入或哈希校验机制；这些属于实现者注意事项,见附录。

## 1. 目标

Arrange DSL 用于这类 agent 编辑场景：agent 已经通过阅读文件、`rg`、`asq read-range`、LSP 等方式,把要移动/保留/删除的文本映射成精确行号；DSL 只负责把这些已知文本片段重新组合成目标文件状态。

核心目标：

- 一眼看出哪些文件会被改写。
- 一眼看出每个改写文件的完整 `before -> after`。
- 一眼看出 `after` 中每段文本来自哪里。
- 一眼看出明显失败条件,例如未完整覆盖、引用未声明、同一文件被安排多次。
- 不使用顺序 action,避免行号漂移与动作编排歧义。

非目标：

- 不根据函数名、类名、符号名自动发现范围。
- 不解析 AST / LSP 语义。
- 不自动修复 `mod`、`use`、import、export、格式化。
- 不生成任意新文本；新文本应来自当前 `before`、`share`、或带 slug `arrange` 的 before 材料。

## 2. 核心模型

Arrange DSL 是**文件级状态转换语言**。

```text
share   = 只读共享材料
arrange = 单个文件的完整 before -> after 状态转换
```

### 2.1 Pre-state snapshot

一次 DSL 应用有且只有一个执行前快照(pre-state snapshot)。

- 所有 `share` range 都在这个快照上解析。
- 所有 `arrange before` range 都在这个快照上解析。
- 所有 `after` item 都只能引用这个快照中已经声明的材料。
- DSL 中不存在 `arrange` 执行顺序。即使 `arrange B` 引用 `arrange A` 的 slug,也只表示引用 `A` 的 before 命名材料,不表示 `A` 先执行。

### 2.2 `share`: 只读共享材料

`share` 声明外部可复用材料。

```text
share <slug> = <file>
  <name> = <range>
  <name> = <range>
end share
```

语义：

- `share` 只读,不会改写 `<file>`。
- `<name>` 绑定到 `<file>` pre-state snapshot 中的一个 line range。
- `after` 中可用 `<slug>::<name>` 引用该材料。
- `share` 适合模板、外部片段、只读来源文件。
- 同一个规范化文件路径最多只能有一个 `share` block。
- 同一个规范化文件路径不能同时作为 `share` 来源与 `arrange` 目标。若会被改写的文件需要向其他 `arrange` 提供 before 材料,应使用带 slug 的 `arrange`。

### 2.3 `arrange`: 文件级改写声明

`arrange` 声明一个文件的完整状态转换。

```text
arrange <file>
  before <file-state>
  after  <file-state>
end arrange
```

也可给 `arrange` 命名,把它的 `before` 材料暴露给其他 `arrange` 使用：

```text
arrange <slug> = <file>
  before <file-state>
  after  <file-state>
end arrange
```

语义：

- 出现在 `arrange` 的 `<file>` 会被改写或校验为 no-op。
- 未出现在 `arrange` 的文件不会被改写。
- 同一个规范中,同一个规范化后的文件路径最多只能出现一个 `arrange`。
- `arrange <slug> = <file>` 暴露的是该文件 **before 状态中的命名 chunk**,不是 after 结果。
- 带 slug 的 `arrange` 只导出 `before` 中的 `<name> = <range>`。匿名 range 与 `<gap:name>` 不导出,不能被其他 `arrange` 通过 `<slug>::...` 引用。
- 若需要跨文件复用 gap 文本,应把该文本声明为普通命名 chunk,而不是 gap。

## 3. 基本语法

本文使用 `|` 表示多种可选形式之一。

### 3.1 range

```text
<range> = N | A-B | A-end
```

规则：

- 行号 1-based。
- 区间 inclusive。
- `A-B` 要求 `A <= B`。
- `A-end` 表示从 `A` 到文件末尾。
- 空文件不能用 `1-end` 表示；空文件状态写 `<empty>`。

### 3.2 文件路径身份

文件身份按规范化路径判断。

规范化至少包括：

- 以执行基准目录解析相对路径。
- 消除 `.` 与 `..`。
- 统一路径分隔符。
- 禁止路径逃逸执行基准目录。

实现必须明确大小写敏感性与 symlink 解析策略。若实现无法可靠判断两个路径是否指向同一文件,应保守失败。

### 3.3 file state

```text
<file-state> = <missing> | <empty> | <sequence>
```

| 状态 | 含义 |
|---|---|
| `<missing>` | 文件不存在 |
| `<empty>` | 文件存在且内容为空(0 bytes) |
| `<sequence>` | 文件内容由一组已声明材料按顺序组成 |

`before` 与 `after` 都是**文件级状态**。因此：

- `before <sequence>` 必须完整覆盖目标文件全文。
- `after <sequence>` 表示目标文件最终全文。
- `after <empty>` 表示目标文件最终为空文件。
- `after <missing>` 表示目标文件最终不存在。
- `<sequence>` 必须至少包含一个 item。空文件必须写 `<empty>`,不能写空 sequence。

`<empty>` 表示 0-byte 文件。只包含换行符、空格、制表符或空白行的文件不是 `<empty>`。

### 3.4 before sequence item

`before <sequence>` 中可写：

```text
<range>
<name> = <range>
<gap:name>
```

含义：

| 写法 | 含义 |
|---|---|
| `<range>` | 匿名材料,来自当前 arrange 目标文件的原始快照 |
| `<name> = <range>` | 命名材料,来自当前 arrange 目标文件的原始快照 |
| `<gap:name>` | 显式 gap,自动绑定相邻 range 之间的原始文本 |

### 3.5 after sequence item

`after <sequence>` 中可写：

```text
<range>
<name>
<gap:name>
<slug>::<name>
```

引用规则：

- `<range>` 只能引用同一个 `arrange before` 中已出现过的匿名 range,且 range 表达式必须字面完全相同。
- 如果 `before` 中写了 `<name> = <range>`,则 `after` 必须用 `<name>` 引用它,不能再用裸 `<range>` 引用同一材料。
- `<name>` 只能引用同一个 `arrange before` 中声明的命名材料。
- `<gap:name>` 只能引用同一个 `arrange before` 中声明的 gap。
- `<slug>::<name>` 只能引用 `share` 中的命名材料,或带 slug 的 `arrange before` 中的命名 chunk。
- `after` 不能凭空写一个未在 `before` / `share` / `arrange slug` 中声明过的新 range。

## 4. 核心不变量

### 4.1 File Cover Invariant

`arrange before` 必须完整描述目标文件的原始全文。

若目标文件不存在：

```text
before <missing>
```

若目标文件存在但为空：

```text
before <empty>
```

若目标文件存在且非空：

```text
before <sequence>
```

此时 `<sequence>` 必须满足：

- 第一个 range 从 line 1 开始。
- 最后一个 range 到 `end` 结束。
- range 按物理行号升序排列。
- range 不重叠。
- 文件中任何两段相邻 range 之间若存在文本,必须写一个 `<gap:name>`。
- 文件全文内不能有未声明文本。

有效：

```text
arrange src/foo.rs
  before 1-20, 21-end
  after  21-end, 1-20
end arrange
```

有效：

```text
arrange src/foo.rs
  before head = 1-20, <gap:middle>, body = 40-90, tail = 91-end
  after  body, <gap:middle>, head, tail
end arrange
```

无效：

```text
arrange src/foo.rs
  before head = 1-20, body = 40-90, tail = 91-end
  after  body, head, tail
end arrange
```

原因：`21-39` 是隐藏 gap,没有在 `before` 中声明。

无效：

```text
arrange src/foo.rs
  before dead = 120-160
  after  <empty>
end arrange
```

原因：`before` 未覆盖整个文件。局部删除必须显式写出保留的 prefix/suffix：

```text
arrange src/foo.rs
  before prefix = 1-119, dead = 120-160, suffix = 161-end
  after  prefix, suffix
end arrange
```

### 4.2 After Provenance Invariant

`arrange after` 中每个 item 必须追溯到已声明材料。

允许来源：

1. 当前 `arrange before`。
2. `share` 声明的 `<slug>::<name>`。
3. 其他带 slug 的 `arrange` 在 `before` 中声明的命名 chunk。

跨 `arrange` 引用不表示执行顺序。`main::parser` 这类引用永远指向 `main` 的 pre-state before 材料。

无效：

```text
arrange src/foo.rs
  before body = 1-end
  after  10-20, body
end arrange
```

原因：`10-20` 没有在当前 `before` 中作为匿名 range 出现,也没有被命名引用。`after` 不是继续从文件任意取 range 的脚本。

有效：

```text
share tpl = snippets/header.rs
  header = 1-end
end share

arrange src/foo.rs
  before body = 1-end
  after  tpl::header, body
end arrange
```

### 4.3 One Arrange Per File Invariant

同一个规范化文件路径最多只能出现一个 `arrange`。

无效：

```text
arrange src/foo.rs
  before first = 1-20, rest = 21-end
  after  rest
end arrange

arrange ./src/foo.rs
  before head = 1-40, tail = 41-end
  after  tail, head
end arrange
```

原因：两个 path 规范化后指向同一文件。允许多个 arrange 会重新引入执行顺序、行号漂移、窗口交叠、隐式保留等问题。

## 5. gap 规则

核心 DSL 不使用隐式 slot 语义。

如果 `before` 中两个 range 不相邻,中间文本必须显式出现为 `<gap:name>`。

```text
arrange src/foo.rs
  before A = 1-10, <gap:comment>, B = 20-end
  after  B, <gap:comment>, A
end arrange
```

`<gap:comment>` 自动绑定原始文件的 `11-19`。

gap 与普通材料一样参与 `after`：

| before 中有 | after 中 | 结果 |
|---|---|---|
| `<gap:g>` | 出现一次 | 保留/移动这段 gap 文本 |
| `<gap:g>` | 不出现 | 删除这段 gap 文本 |
| `<gap:g>` | 出现多次 | 复制这段 gap 文本 |

规则：

- gap 只能出现在两个 range item 之间。
- gap 名在当前 arrange 内唯一。
- gap 名与当前 arrange 的 chunk 名共用命名空间,不得冲突。
- 空 gap 无效；相邻 range 不需要 gap。

## 6. `<missing>` / `<empty>` 转换矩阵

| before | after | 语义 |
|---|---|---|
| `<missing>` | `<sequence>` | 创建文件 |
| `<missing>` | `<empty>` | 创建空文件 |
| `<missing>` | `<missing>` | 无效,通常是误写 |
| `<empty>` | `<sequence>` | 填充空文件 |
| `<empty>` | `<empty>` | 校验为空,no-op |
| `<empty>` | `<missing>` | 删除空文件 |
| `<sequence>` | `<sequence>` | 重写整个文件为 after 序列 |
| `<sequence>` | `<empty>` | 清空文件,文件保留 |
| `<sequence>` | `<missing>` | 删除文件 |

`<missing>` 和 `<empty>` 必须单独出现,不能与 sequence item 混用。

## 7. 派生效果

DSL 不写 `move` / `copy` / `delete` 动作。工具从 `before` 与 `after` 的差异推导效果。

| 状态差异 | 派生效果 |
|---|---|
| `before <missing>` → `after <sequence>` | 创建文件 |
| `before <sequence>` → `after <missing>` | 删除文件 |
| before item 在该文件 after 中位置变化 | 重排/移动 |
| before item 在该文件 after 中不存在,但出现在其他文件 after 中 | 提取/跨文件移动 |
| before item 在原文件 after 中保留,又出现在其他文件 after 中 | 复制到其他文件 |
| before item 在所有 after 中都不存在 | 删除该材料 |
| share item 出现在 after 中 | 从只读材料复制进入目标 |
| 同一 item 在 after 中出现多次 | 复制该材料 |
| before 与 after 等价 | no-op,但仍要求 before 校验通过 |

## 8. 文本与换行模型

- range 选择完整逻辑行,不选择半行。
- sequence item 渲染为逻辑行序列。
- 改写已有文件时,输出使用目标文件原有 newline style(`LF` 或 `CRLF`)。
- 改写已有文件时,默认保留目标文件原有 final-newline 状态。
- 创建新文件时,默认使用 `LF` 且带 final newline。新文件的 newline style 不受 `share` 来源文件影响。
- `after <empty>` 创建或保留一个 0-byte 空文件。
- `after <missing>` 删除文件,不产生文本输出。

## 9. 示例

### 9.1 单文件重排

```text
arrange src/foo.rs
  before api = 1-60, parser = 61-140, rest = 141-end
  after  api, rest, parser
end arrange
```

效果：`parser` 移到文件末尾。`before` 覆盖整个文件。

### 9.2 带 gap 的重排

```text
arrange src/foo.rs
  before A = 1-10, <gap:comment>, B = 20-end
  after  B, <gap:comment>, A
end arrange
```

效果：原始 `11-19` 被显式命名为 `comment`,并在 after 中保留。

### 9.3 删除文件中的一个块

```text
arrange src/foo.rs
  before prefix = 1-119, dead = 120-160, suffix = 161-end
  after  prefix, suffix
end arrange
```

效果：`dead` 从最终文件中消失。没有隐式 prefix/suffix。

### 9.4 从只读材料插入内容

```text
share tpl = snippets/header.rs
  header = 1-end
end share

arrange src/foo.rs
  before body = 1-end
  after  tpl::header, body
end arrange
```

效果：把只读模板 `header` 放到 `src/foo.rs` 开头。

### 9.5 跨文件提取

```text
arrange main = src/foo.rs
  before api = 1-60, parser = 61-140, rest = 141-end
  after  api, rest
end arrange

arrange src/parser.rs
  before <missing>
  after  main::parser
end arrange
```

效果：

- `src/foo.rs` 删除 `parser` 段。
- `src/parser.rs` 被创建,内容为原始 `src/foo.rs` 的 `parser` 段。
- `main::parser` 指向 `main` 的 before 材料,不是 after 结果。

### 9.6 删除文件,同时导出 before 材料

```text
arrange old = src/old.rs
  before useful = 1-50, rest = 51-end
  after  <missing>
end arrange

arrange src/new.rs
  before <missing>
  after  old::useful
end arrange
```

效果：`src/old.rs` 被删除,但它的 pre-state `useful` 段被写入 `src/new.rs`。

### 9.7 删除文件

```text
arrange src/old.rs
  before all = 1-end
  after  <missing>
end arrange
```

效果：删除 `src/old.rs`。

### 9.8 清空文件

```text
arrange src/generated.rs
  before all = 1-end
  after  <empty>
end arrange
```

效果：`src/generated.rs` 仍存在,但内容变为 0 bytes。

### 9.9 创建空文件

```text
arrange src/empty.rs
  before <missing>
  after  <empty>
end arrange
```

效果：创建 0-byte 文件。

## 10. 失败条件

任何失败都必须使该 DSL 文档无效。具体写入策略属于实现层。

### 10.1 名称与路径

- share slug 重复。
- arrange slug 与 share slug 或其他 arrange slug 重复。
- 同一个规范化文件路径出现在多个 `share` 中。
- 同一个规范化文件路径出现在多个 `arrange` 中。
- 同一个规范化文件路径同时作为 `share` 来源与 `arrange` 目标出现。若被改写文件需要向其他 `arrange` 暴露 before 材料,必须使用 `arrange <slug> = <file>`。该规则用于避免同一文件材料出现两个声明入口。
- 同一 block 内 chunk 名重复。
- 同一 arrange 内 gap 名重复。
- 同一 arrange 内 chunk 名与 gap 名冲突。

### 10.2 share 失败

- share 文件不存在。
- share range 语法无效。
- share range 越界。
- 同一 share 内 range 重叠。
- share 文件为空却使用 `1-end`。

### 10.3 arrange before 失败

- `before <missing>` 但目标文件存在。
- `before <empty>` 但目标文件不存在或非空。
- `before <sequence>` 但目标文件不存在或为空。
- before range 语法无效。
- before range 越界。
- `before <sequence>` 第一个 range 不从 line 1 开始。
- `before <sequence>` 最后一个 range 不到 `end`。
- `before <sequence>` range 非升序或重叠。
- `before <sequence>` 存在未声明 gap。
- `<gap:name>` 没放在两个 range item 之间。
- `<gap:name>` 对应空区间。

### 10.4 arrange after 失败

- `after` 引用未声明材料。
- 命名 before item 在 after 中被裸 range 引用。
- `after <missing>` 与其他 item 混用。
- `after <empty>` 与其他 item 混用。
- `after <sequence>` 引用的 share/arrange slug 不存在。
- `after <sequence>` 引用另一个 arrange 中未命名的匿名 range。
- `after <sequence>` 引用另一个 arrange 中的 gap。

## 11. 附录: 实现者注意事项(不属于 DSL 语义)

这些事项不改变 DSL 的含义,但实现 CLI 时应考虑。

- 默认 dry-run,显式确认后写入,能降低 agent 误操作风险。
- 多文件变更应尽量全有或全无;无法保证时应在写入前失败或明确报告。
- 预览报告应列出每个目标文件、before/after 状态类别、派生效果、gap 实际范围与行数、重复引用、删除项、跨文件移动、创建/删除文件、清空文件、no-op。大 gap 应醒目标注。
- 实现必须满足本文定义的 pre-state snapshot 语义。文件/范围哈希属于可选校验机制,不属于核心 DSL。不要把哈希写成理解 DSL 所必需的语法。
- 格式化、import/module 修复、合成新代码应交给其他工具或独立步骤。

## 12. 设计总结

核心 DSL：

```text
share   = 外部只读材料
arrange = 单文件完整 before -> after 状态转换
```

核心不变量：

1. `arrange before` 必须完整覆盖目标文件状态。
2. `arrange after` 只能引用当前 before、share、或其他带 slug arrange 的 before 材料。
3. 同一文件最多一个 arrange。
4. gap 必须显式命名,不存在隐式 slot。

这使 DSL 保持为可审查的状态声明,而不是顺序动作脚本。
