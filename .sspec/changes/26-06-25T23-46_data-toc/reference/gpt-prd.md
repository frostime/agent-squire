# PRD: `data-toc` — Structured Data TOC for Agent Squire

## 1. 背景

Agent 在处理未知 JSON / YAML / JSONL 文件时，经常面临一个低效问题：

> 为了理解结构，被迫读取大量原始内容。

这会造成几个直接问题：

1. 上下文浪费：大量重复数组元素、日志行、配置块进入上下文。
2. 结构判断困难：Agent 看到的是内容片段，不是结构地图。
3. 后续读取不精确：不知道主数据路径在哪里，也不知道应该抽样哪个字段。
4. 解析代码容易写错：没有先确认字段分布、数组结构、异构记录类型。

`agent-squire` 已经有 `file-tree` 和 `md-toc` 这类“先看结构，再决定读取”的命令。`data-toc` 的定位是把这种工作流扩展到结构化数据文件。

一句话定义：

> `data-toc` 是面向 Agent 的结构化数据目录生成器，用于在读取原始内容前快速理解 JSON / YAML / JSONL 的大致内部结构。

---

## 2. 产品定位

### 2.1 是什么

`data-toc` 生成类似 LSP symbols / Markdown TOC 的结构目录。

它应该帮助 Agent 回答：

1. 根结构是什么？
2. 顶层有哪些字段？
3. 主数组或主记录流在哪里？
4. 数组元素是否高度重复？
5. JSONL 行之间是否存在多种结构类型？
6. 哪些字段稳定出现？
7. 哪些字段只在部分样本中出现？
8. 下一步应该读取哪些代表路径或代表行？

### 2.2 不是什么

`data-toc` 不是：

1. JSON Schema generator。
2. 数据验证器。
3. 完整类型推断器。
4. YAML 语义保真解析器。
5. jq / yq 替代品。
6. 数据查询语言。
7. 需要用户理解大量参数的数据挖掘工具。

它只做一件事：

> 以有限预算生成结构 TOC，并清楚说明这个 TOC 的不确定性。

---

## 3. 设计原则

### 3.1 小接口，深模块

`data-toc` 面向 Agent，不应暴露大量内部调参项。

推荐公共接口：

```bash
squire data-toc <path>
squire data-toc <path> --format auto|json|jsonl|yaml
squire data-toc <path> --budget small|normal|large
squire data-toc <path> --examples
```

不建议公开以下一类参数作为主接口：

```text
--group-by
--shape-depth
--max-groups
--max-array-items
--dynamic-keys
--max-children
--max-shapes
```

这些属于内部算法策略。默认策略应足够稳健；无法判断时，直接在输出中说明不确定性，而不是要求 Agent 通过复杂参数修正。

### 3.2 输出不伪装成事实

`data-toc` 的输出来自采样和预算扫描，因此必须避免过度断言。

应该写：

```text
email string? 41/64 observed
```

不应该写：

```text
email optional
```

应该写：

```text
JSONL groups are approximate structural clusters.
```

不应该写：

```text
JSONL has exactly 3 record types.
```

### 3.3 结构优先，值默认隐藏

默认不输出具体值。

原因：

1. 结构理解通常不需要值。
2. JSON / YAML 可能包含 token、secret、邮箱、路径、业务数据。
3. Agent 先看结构，后续再定向读取样本。

只有显式 `--examples` 时，才输出少量经过截断和脱敏的示例值。

### 3.4 自然语言比复杂结构更适合 Agent

`data-toc` 的 compact 输出应优先服务 Agent 阅读。

JSON envelope 是给机器消费的；compact 输出应直接表达：

1. 结构树；
2. 主要发现；
3. 不确定性；
4. 建议下一步读取方式。

不要把所有信息都压成深层 JSON，让 Agent 再二次解释。

---

## 4. 支持范围

### 4.1 JSON

JSON 是核心支持对象。

处理目标：

1. 支持普通 JSON object。
2. 支持顶层 JSON array。
3. 支持深层嵌套 object / array。
4. 支持大文件的预算扫描。
5. 不要求完整加载整个 JSON。

JSON 处理依赖 `jq` 的 stream 能力。

### 4.2 YAML

YAML 是可选支持。

边界：

1. `data-toc` 不内置 YAML parser。
2. 不新增 Rust YAML 解析依赖。
3. 仅当用户环境存在 `yq` 时支持 YAML。
4. YAML 通过 `yq` 转换为 JSON 后进入同一结构分析流程。
5. YAML 输出必须标注近似性。

YAML 不承诺保留：

1. comments；
2. anchors；
3. aliases；
4. tags；
5. formatting；
6. 原始 merge 结构。

YAML 支持的目标不是保真，而是快速理解配置结构。

### 4.3 JSONL / NDJSON

JSONL 是重点支持对象。

JSONL 不应被视为同构数组。很多 JSONL 文件更接近事件流、日志流、消息流或混合记录流，每一行可能有不同 shape。

因此 `data-toc` 对 JSONL 的目标不是简单输出：

```text
$[] object
```

而是：

1. 输出整体记录流结构；
2. 判断记录是否异构；
3. 尝试把结构相近的记录分组；
4. 为每个主要组提供代表 shape；
5. 提供该组首次出现的行号；
6. 如果找不到稳定分类依据，明确说明。

---

## 5. 用户故事

### 5.1 Agent 读取未知实验结果 JSON

用户或 Agent 执行：

```bash
squire data-toc result.json
```

输出应该让 Agent 迅速知道：

```text
主数据路径是 $.runs[]
每个 run 有 config、metrics、artifacts
metrics 下有 acc、f1、auc
notes 字段只在部分样本中出现
```

Agent 下一步可以只读：

```bash
jq '.runs[0:5] | map({config, metrics})' result.json
```

而不是读取整个文件。

### 5.2 Agent 读取未知 JSONL 日志

用户或 Agent 执行：

```bash
squire data-toc logs.jsonl
```

输出应该让 Agent 迅速知道：

```text
这是异构 JSONL
大致存在 message / error / metric 三类结构
error 类第一次出现在第 37 行
metric 类第一次出现在第 84 行
```

Agent 下一步可以读代表行：

```bash
sed -n '37p' logs.jsonl | jq .
sed -n '84p' logs.jsonl | jq .
```

### 5.3 Agent 读取 YAML 配置

用户或 Agent 执行：

```bash
squire data-toc compose.yaml
```

如果环境存在 `yq`，输出服务、镜像、端口、环境变量等结构。

如果环境没有 `yq`，明确报错，不引入内部 YAML parser。

---

## 6. 输出形态

### 6.1 Compact 输出

默认输出应类似：

```text
# data-toc result.json
format=json mode=structure-toc complete=false budget=normal

$ object
├─ metadata object
│  ├─ version string
│  └─ created_at string
├─ runs array<object> observed_items≈64 shape≈2
│  └─ [] object
│     ├─ id string 64/64
│     ├─ config object 64/64
│     │  ├─ model string
│     │  └─ seed number
│     ├─ metrics object 64/64
│     │  ├─ acc number
│     │  ├─ f1 number
│     │  └─ auc number?
│     └─ notes string? 11/64
└─ summary object

Notes:
- Output is based on bounded structural scanning.
- `?` means not present in all observed samples.
- Array indexes are collapsed into [].

Suggested reads:
- jq '.runs[0:5]' result.json
- jq '.runs[0:20] | map({id, config, metrics})' result.json
```

### 6.2 JSONL compact 输出

JSONL 输出应显式体现异构记录：

```text
# data-toc logs.jsonl
format=jsonl mode=record-stream-toc complete=false budget=normal
sampled_lines=1000

$ array<record> virtual=jsonl groups≈4
└─ [] object
   ├─ type string 1000/1000 enum≈4
   ├─ timestamp string 1000/1000
   ├─ payload object? 716/1000
   ├─ error object? 91/1000
   └─ metrics object? 233/1000

Record groups:
- type=message rows=521 first_line=1
  shape: object{type,timestamp,user,payload}
- type=error rows=91 first_line=37
  shape: object{type,timestamp,error}
- type=metric rows=233 first_line=84
  shape: object{type,timestamp,name,value,tags}
- other rows=155 first_line=12 minor_shapes≈7

Notes:
- JSONL records appear heterogeneous.
- Groups are approximate structural clusters.
- The field `type` appears to explain most observed groups.

Suggested reads:
- sed -n '1p' logs.jsonl | jq .
- sed -n '37p' logs.jsonl | jq .
- sed -n '84p' logs.jsonl | jq .
```

如果没有稳定解释字段：

```text
Record groups:
- shape#1 rows=620 first_line=1
  shape: object{id,timestamp,message}
- shape#2 rows=218 first_line=17
  shape: object{id,timestamp,error,stack}
- shape#3 rows=91 first_line=42
  shape: object{id,timestamp,metrics}
- other rows=71 first_line=103 minor_shapes≈12

Notes:
- No stable discriminator field was found.
- Groups are labeled by structural shape.
```

### 6.3 JSON 输出

`--print json` 应继续遵守 Agent Squire 的稳定 envelope 风格。

必须包含：

1. `ok`
2. `command`
3. `data`
4. `warnings`
5. `meta`

`data` 至少包括：

```text
path
format
mode
complete
root
summary
notes
suggested_reads
```

对于 JSONL，额外包括：

```text
record_groups
```

JSON 输出供机器消费；compact 输出供 Agent 直接阅读。

---

## 7. 核心概念

### 7.1 TOC，不是 Schema

`data-toc` 输出的是结构目录，不是正式 schema。

区别：

| 结构 TOC         | JSON Schema |
| -------------- | ----------- |
| 帮 Agent 快速理解结构 | 帮验证器验证数据    |
| 接受采样和近似        | 追求形式化定义     |
| 输出应短、可读        | 输出可能很长      |
| 重点是导航和下一步读取    | 重点是约束和验证    |
| 可显示不确定性        | 需要明确规则      |

`data-toc` 不应该试图承担 JSON Schema 的职责。

### 7.2 Normalized Path

数组索引必须压缩。

原始路径：

```text
$.runs[0].metrics.acc
$.runs[1].metrics.acc
$.runs[2].metrics.acc
```

TOC 路径：

```text
$.runs[].metrics.acc
```

这一步是 `data-toc` 的核心价值之一。没有数组压缩，输出会迅速退化成重复内容列表。

### 7.3 Sample Presence

字段出现率是样本观察，不是全局真理。

示例：

```text
notes string? 11/64
```

含义：

```text
在观察到的 64 个数组元素中，11 个有 notes 字段。
```

不表示：

```text
notes 在完整数据中一定 optional。
```

### 7.4 Structural Group

JSONL 中的记录组是结构组，不一定是语义类型。

记录组可以由字段值解释，例如：

```text
type=error
```

也可以只能由结构解释，例如：

```text
shape#2
```

工具应该优先诚实表达：

```text
看起来可以由 type 字段解释。
```

或者：

```text
没有找到稳定解释字段。
```

---

## 8. 核心算法

本节只描述必须明确的算法边界。实现细节由 Agent 根据项目上下文决定。

### 8.1 JSON 结构扫描

JSON 使用外部 `jq` 生成结构事件流。

核心思路：

```text
JSON
→ jq stream events
→ normalize path
→ aggregate path/type/count
→ build TOC tree
→ render compact/json output
```

算法要求：

1. 不全量读取大 JSON 为 Rust DOM。
2. 必须有预算边界。
3. 必须将数组索引归一化为 `[]`。
4. 必须统计路径上的类型分布。
5. 必须能从叶子路径反推出父级 object / array 结构。
6. 必须标注扫描是否完整。
7. 必须在输出中解释 `?`、`[]`、采样和截断。

### 8.2 Array Compression

数组压缩不是展示细节，而是结构归纳。

对于：

```json
{
  "items": [
    {"id": 1, "name": "A"},
    {"id": 2, "name": "B"}
  ]
}
```

输出应是：

```text
items array<object>
└─ [] object
   ├─ id number
   └─ name string
```

而不是：

```text
items[0].id
items[1].id
```

对数组需要尽量判断：

1. array of scalar；
2. array of object；
3. array of array；
4. mixed array；
5. observed element count；
6. approximate shape count。

### 8.3 Field Presence

对数组元素或 JSONL records 中的对象字段，统计 sample presence。

例如：

```text
email string? 41/64
```

显示规则：

1. 全部或几乎全部出现：不加 `?`。
2. 部分出现：加 `?`。
3. 极少出现：可标记 rare。
4. 具体阈值属于内部策略，不作为公共接口暴露。

### 8.4 JSONL Structural Clustering

JSONL 的重点是异构记录处理。

不要简单假设所有行同 shape。

推荐内部算法：

```text
sample JSONL lines
→ parse each line as JSON value
→ build per-row structural feature set
→ exact shape grouping
→ merge highly similar groups
→ look for explanatory discriminator field
→ output major groups with first_line
```

#### 8.4.1 Per-row structural features

每一行提取结构特征，而不是内容特征。

示例行：

```json
{
  "type": "error",
  "timestamp": "2026-06-25T10:00:00Z",
  "error": {
    "code": 500,
    "message": "failed"
  }
}
```

结构特征类似：

```text
$.type:string
$.timestamp:string
$.error:object
$.error.code:number
$.error.message:string
```

结构特征应当是 depth-bounded 的，避免超深对象支配聚类。

#### 8.4.2 Shape grouping

先对完全相同或近似相同的结构签名分组。

这一步是确定性的，不需要真正 ML 依赖。

不建议引入 ML crate。原因：

1. 结构聚类可以由确定性相似度完成。
2. ML 增加依赖和随机性。
3. Agent 工具需要可解释、稳定、可复现。
4. 输出应该能说明为什么这样分组。

可以把它看作“ML-like unsupervised clustering”，但实现上是 deterministic structural clustering。

#### 8.4.3 Similarity merge

对于相近但不完全相同的 shape，可以合并为一组。

例如：

```text
object{type,timestamp,user,payload}
object{type,timestamp,user,payload,trace_id}
```

它们可以归为同一类，`trace_id` 在组内显示为部分出现字段。

阈值和细节属于内部策略，不暴露为参数。

#### 8.4.4 Explanatory label

结构分组完成后，再尝试寻找解释字段。

优先检查这类顶层字段：

```text
type
kind
event_type
event
action
op
role
level
category
```

但字段名不是绝对依据。

正确逻辑是：

```text
先看结构是否形成组；
再看某个字段是否能够解释这些组。
```

不是：

```text
看到 type 字段就直接按 type 分组。
```

如果字段能够解释大部分结构组：

```text
type=error
type=message
type=metric
```

如果不能：

```text
shape#1
shape#2
shape#3
```

#### 8.4.5 First line reference

每个 JSONL record group 必须输出首个代表行号。

这是高价值信息。

示例：

```text
type=error rows=91 first_line=37
```

Agent 可以直接读取：

```bash
sed -n '37p' logs.jsonl | jq .
```

这比复杂聚类指标更有用。

### 8.5 Dynamic Key Compression

动态 key 会导致 TOC 爆炸。

例如：

```json
{
  "users": {
    "user_001": {"name": "A"},
    "user_002": {"name": "B"}
  }
}
```

理想输出：

```text
users object<dynamic_key>
└─ {dynamic_key} object
   └─ name string
```

而不是：

```text
users.user_001.name
users.user_002.name
```

动态 key 检测属于内部启发式，不需要暴露参数。

工具只需在输出中说明：

```text
Some sibling keys were compressed as {dynamic_key}.
```

### 8.6 Suggested Reads

`data-toc` 不只是展示结构，还应给出下一步读取建议。

建议应简单、可复制、少量。

JSON 示例：

```bash
jq '.runs[0:5]' result.json
jq '.runs[0:20] | map({id, config, metrics})' result.json
```

JSONL 示例：

```bash
sed -n '37p' logs.jsonl | jq .
sed -n '84p' logs.jsonl | jq .
```

YAML 示例可以保守处理，不必强行生成复杂命令。

---

## 9. 公共接口

### 9.1 推荐接口

```bash
squire data-toc <path>
```

### 9.2 可选参数

```bash
--format auto|json|jsonl|yaml
--budget small|normal|large
--examples
```

### 9.3 参数含义

`--format`：

```text
仅用于格式自动判断失败或用户想强制指定格式。
```

`--budget`：

```text
控制扫描预算和输出长度。
```

它是高层语义参数，不要求用户理解 events、depth、groups 等内部变量。

`--examples`：

```text
允许输出少量示例值。默认关闭。
```

### 9.4 不建议公开的参数

这些参数不应作为主帮助中的一等选项：

```text
--max-events
--max-lines
--shape-depth
--max-groups
--group-by
--dynamic-keys
--max-array-items
```

如果实现确实需要，可作为 hidden/debug 参数，但不作为 Agent-facing interface 的设计目标。

---

## 10. 预算策略

`data-toc` 必须 bounded。

推荐用高层预算：

```text
small
normal
large
```

语义：

| Budget | 用途                |
| ------ | ----------------- |
| small  | 快速看一眼结构           |
| normal | 默认 Agent 使用       |
| large  | 文件复杂、结构异构、需要更完整观察 |

具体内部数值由实现决定。

输出必须包含预算结果：

```text
complete=false
budget=normal
sampled_lines=1000
stopped=max_budget
```

不要让 Agent 误以为结果是完整扫描。

---

## 11. 依赖边界

### 11.1 jq

`jq` 是 JSON / YAML stream extraction 的外部依赖。

`data-toc` 可以要求用户环境存在 `jq`。

如果没有 `jq`，明确报错。

### 11.2 yq

`yq` 只用于 YAML。

规则：

1. JSON / JSONL 不依赖 yq。
2. YAML 只有在 yq 存在时支持。
3. 没有 yq 时，YAML 输入明确报错。
4. 不新增 Rust YAML parser 依赖。

### 11.3 Rust dependencies

不应为了 JSONL clustering 引入 ML 依赖。

JSONL 聚类应使用内部确定性结构算法完成。

---

## 12. 集成位置

`data-toc` 应作为 Agent Squire 内置命令，而不是外部 mapped command。

理由：

1. 它是通用 Agent discovery tool。
2. 它与 `md-toc` 定位对称。
3. 它需要稳定 JSON envelope。
4. 它需要与全局 `--print` 风格一致。
5. 它不是薄 shell wrapper，而是有内部结构聚合逻辑的深模块。

建议命令：

```text
data-toc
```

可选别名：

```text
datatoc
json-toc
jsontoc
```

是否保留多个别名可由项目整体命名风格决定。

---

## 13. 非目标

第一版不做：

1. JSON Schema 输出。
2. Pydantic / TypeScript / Rust 类型生成。
3. 完整 YAML 语义保真。
4. 完整读取大文件。
5. 用户可配置的复杂聚类参数。
6. TUI / 交互式浏览器。
7. 对所有边缘结构的强保证发现。

未来可以考虑：

1. `data-read`：按结构路径读取样本。
2. `data-schema`：从代表样本生成 schema。
3. `gather` 集成：把 `data-toc` 作为结构化文件默认预览。
4. 对 JSONL 输出每类局部 TOC。

---

## 14. 错误与不确定性表达

错误必须直接：

```text
error: YAML support requires yq
error: jq not found
error: invalid JSONL at line 37
error: format could not be detected
```

不确定性必须直接：

```text
Output is sampled.
Groups are approximate structural clusters.
No stable discriminator field was found.
Some dynamic keys were compressed.
Optional markers are based on observed samples.
```

不要用参数把不确定性交给 Agent 自行调试。

---

## 15. 验收标准

### 15.1 JSON

输入：

```json
{
  "runs": [
    {"id": "a", "metrics": {"acc": 0.9}},
    {"id": "b", "metrics": {"acc": 0.8}, "notes": "x"}
  ]
}
```

期望输出包含：

```text
runs array<object>
[] object
id string
metrics object
metrics.acc number
notes string? 1/2
```

### 15.2 JSONL 异构记录

输入：

```jsonl
{"type":"message","text":"hello"}
{"type":"error","error":{"code":500}}
{"type":"metric","name":"latency","value":31}
```

期望输出包含：

```text
format=jsonl
records appear heterogeneous
Record groups
type=message first_line=1
type=error first_line=2
type=metric first_line=3
```

或者在没有稳定解释字段时：

```text
shape#1
shape#2
shape#3
```

### 15.3 YAML 外部依赖

如果输入 YAML 且环境中没有 yq：

```text
error: YAML support requires yq
```

如果有 yq：

```text
format=yaml parsed_as=json
```

### 15.4 输出契约

必须满足：

1. compact 输出可直接给 Agent 阅读；
2. JSON 输出遵守稳定 envelope；
3. 输出中明确扫描预算和不确定性；
4. 数组索引被压缩为 `[]`；
5. JSONL group 输出包含 `first_line`；
6. 默认不输出敏感值示例。

---

## 16. 最终产品判断

`data-toc` 补齐的是 Agent Squire 当前工具链中的结构化数据预览能力：

```text
file-tree  → 目录结构
md-toc     → Markdown 文档结构
data-toc   → JSON / YAML / JSONL 数据结构
read-range → 精确读取文本范围
gather     → 组合上下文
```

它的价值不是“比 jq 更强”，而是把 jq / yq / JSONL 结构聚合封装成一个 Agent 友好的深模块：

```text
简单命令
稳定输出
数组压缩
异构记录识别
不确定性显式暴露
下一步读取建议
```

成功的 `data-toc` 应该让 Agent 在读取未知结构化文件前，先得到一张可靠但不装作完整的结构地图。
