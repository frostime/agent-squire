---
change: "rearange"
created: 2026-06-28T22:20:29
---

# Design: rearrange

> v1 scope (clarify 决策): 单文件 · 每次恰好 1 条 action · 仅 DSL · 裸数字行号 · 无 backup · gap 默认 slot。

## 1. CLI 表面

```text
asq rearrange [SPEC]

  SPEC                位置参数:字面 DSL,或 @stdin / @file:path / @env:NAME
  -f, --file <PATH>   从文件读 spec
      --stdin         从 stdin 读 spec
  -y, --yes           写入(无则 dry-run)
      --dry-run       显式 dry-run(默认行为,可省)
      --prompt        打印 DSL 指南
  # 继承全局: --cwd <DIR>  --print <compact|json|...>  --json
```

默认 dry-run(与 RFC 一致;比 patch-edit 的"必须 --yes 或 --dry-run"更宽松,因 rearrange 预览导向)。

`--prompt` 面向 AGENT(仿 `patch_edit::PATCH_PROMPT`):含 (a) DSL 语法 (第 2 节) + (b) target/gap 速查 + (c) **CLI 用法**(`--stdin` / `-f` + `asq tmp` 临时文件 / 位置参数 / dry-run→`--yes` 工作流) + (d) 安全提示(默认 dry-run、单 action、行号 1-based)。

## 2. DSL 语法

```text
file <path>                       # 必需,且单文件;出现第二个 file → INVALID_SPEC

chunk <name> = <N>                # 单行
chunk <name> = <A>-<B>            # 区间, 1-based inclusive, 裸数字

# 恰好一条 action:
move   <range-or-chunk> to <anchor>
copy   <range-or-chunk> to <anchor>
delete <range-or-chunk>
rearrange <name>, <name>, ... => <name>, <name>, ...  [gap=slot|drop|error]

# range-or-chunk : 内联 "10-20" / "42"  或  已声明的 <name>
# anchor         : start | end | before <N> | after <N>
```

设计取舍:
- `move/copy/delete` 只作用单区间 → 允许**内联** `10-20`,免去声明,最省。也可引用已声明 chunk。
- `rearrange` 需按名字置换 → **必须**用 `chunk` 声明命名。
- `from` 列表语义 = 参与置换的 chunk **集合**(顺序不敏感,物理槽位由行号升序决定);`to` = 新顺序。`from` 与 `to` 集合不等 → `REARRANGE_SET_MISMATCH`;名字重复亦 → `REARRANGE_SET_MISMATCH`。

## 2.1 DSL 形式文法(rev-001)

> 原 §2 散文留有歧义(review 发现);此处固化为 EBNF + 词法规则。`prompt.md` 同步。

```ebnf
spec       = { line } ;
line       = blank | comment | file-decl | chunk-decl | action ;
comment    = "#" , { any } ;                  (* 仅整行;行内 # 视为字面量 *)
file-decl  = "file" , ws , path ;             (* 恰好一个 *)
chunk-decl = "chunk" , ws , name , "=" , range ;
action     = move | copy | delete | rearrange ;  (* 恰好一条 *)
move       = "move"   , ws , region , ws , "to" , ws , anchor ;
copy       = "copy"   , ws , region , ws , "to" , ws , anchor ;
delete     = "delete" , ws , region ;
rearrange  = "rearrange" , ws , namelist , "=>" , namelist , [ ws , "gap=" , gap ] ;
region     = range | name ;                   (* 首字符为数字时按 range 解析 *)
range      = number | number , "-" , number ; (* 1-based inclusive, start <= end *)
anchor     = "start" | "end" | "before" , ws , number | "after" , ws , number ;
namelist   = name , { "," , name } ;
gap        = "slot" | "drop" | "error" ;
name       = ( letter | "_" ) , { letter | digit | "_" } ;  (* 标识符,非关键字 *)
number     = nonzero-digit , { digit } ;       (* >= 1 *)
```

词法规则(消歧依据):
- **chunk 名 = 标识符** `[A-Za-z_][A-Za-z0-9_]*`。首字符强制为字母/下划线,使其与内联 range(数字开头)不相交,杜绝 `1A` 被误解析为区间。
- **关键字保留**,不可作 chunk 名:`file chunk move copy delete rearrange to start end before after gap`。消除 ` to ` 分隔符冲突与 `to`/`gap` 同名歧义。
- 非法名(违反标识符规则或撞关键字)→ `INVALID_SPEC`(声明时即拒)。
- `#` 仅行首注释,行内 `#` 不剥离。

## 3. 核心数据模型

```rust
struct Spec { file: PathBuf, chunks: Vec<ChunkDef>, action: Action }
struct ChunkDef { name: String, start: usize, end: usize }   // 1-based inclusive

enum Action {
    Move   { src: Region, to: Anchor },
    Copy   { src: Region, to: Anchor },
    Delete { src: Region },
    Rearrange { from: Vec<String>, to: Vec<String>, gap: Gap },
}
enum Region { Inline { start: usize, end: usize }, Named(String) }
enum Anchor { Start, End, Before(usize), After(usize) }   // N = 原始行号
enum Gap { Slot, Drop, Error }

// planner 产物
struct Plan {
    file: ResolvedFile,            // 原始行数组 + newline + encoding
    new_lines: Vec<String>,        // materialize 结果(逻辑行,不含行尾符)
    summary: ActionSummary,        // 给 compact 渲染
}
```

## 4. Planner 算法(全部基于原始行数组,0-based 内部索引)

`lines: Vec<String>` = 原文按换行拆出的逻辑行(不含 `\n`/`\r\n`),`len = lines.len()`。

### 4.1 anchor → 插入点 `ins`(0-based,"在该索引前插入")

```text
start     -> 0
end       -> len
before N  -> N-1
after  N  -> N
```
校验:`N` ∈ [1, len];否则 `ANCHOR_OUT_OF_BOUNDS`。

### 4.2 move / copy  (src = [a,b], 1-based → 内部 [a-1, b-1])

```text
校验:
  1 <= a <= b <= len          否则 INVALID_RANGE / RANGE_OUT_OF_BOUNDS
  move 时 ins 不得落在区间内部: 错误当 a <= ins_line <= b 的内部
      (before/after 落在 (a, b) 内 → ANCHOR_INSIDE_MOVED_CHUNK;
       落在边界 before a / after b = 原位,视为 no-op 允许)

block = lines[a-1 ..= b-1]

materialize(单遍扫描):
  out = []
  for i in 0..=len:
      if i == ins: out.extend(block)
      if i < len:
          if move 且 i ∈ [a-1, b-1]: continue   # copy 不跳过
          out.push(lines[i])
  new_lines = out
```

### 4.3 delete (src = [a,b])

```text
new_lines = lines 去掉 [a-1, b-1]
```

### 4.4 rearrange (gap=slot)

```text
slots = chunks 按 start 升序  -> P1 < P2 < ... < Pk   (物理槽位)
校验:
  全部 name 已声明                       否则 UNKNOWN_CHUNK
  set(from) == set(to) == set(slots)     否则 REARRANGE_SET_MISMATCH
  相邻 slot 不重叠 (Pi.end < P{i+1}.start) 否则 OVERLAPPING_CHUNKS
  每个 slot 区间合法 inbounds             否则 RANGE_OUT_OF_BOUNDS

span     = [P1.start, Pk.end]                       # 1-based inclusive
gap[i]   = lines 位于 (Pi.end, P{i+1}.start)         # i = 1..k-1, 可空
content[name] = lines[name.start-1 ..= name.end-1]

# to[i] 指定物理槽位 i 放哪个 chunk 的内容
rebuilt = content[to[0]]
          + gap[0] + content[to[1]]
          + gap[1] + content[to[2]] + ...           # gap=drop: 省略所有 gap
                                                      # gap=error: 任一 gap 非空 → 失败
new_lines = lines[.. P1.start-1] ++ rebuilt ++ lines[Pk.end ..]
#           ^ span 前(含 g0)原样          ^ span 后(含 gk)原样
```

**验证示例**(RFC case 3):

```text
原始:  A  hidden1  B  (empty)  C  hidden2  D
槽位:  P1=A P2=B P3=C P4=D    gap1=hidden1 gap2=∅ gap3=hidden2
to  =  B, D, C, A
结果:  B  hidden1  D  (empty)  C  hidden2  A     ✓
       └slot1┘ g1   └s2┘  g2   └s3┘  g3    └s4┘
```

## 5. 文本 IO(BC-3)

```text
读: 字节 -> 检测 BOM/UTF-8/GBK/1252 (同 patch_edit/io 逻辑) -> String
    检测换行: 含 "\r\n" => CRLF 否则 LF
    检测末尾换行: 原文是否以换行结尾
拆行: 按换行符拆为逻辑行(不含行尾符)
写: new_lines.join(newline) (+ 末尾换行如原文有) -> 按原编码 -> 原子写(tempfile persist)
```
自包含于 `rearrange/textio.rs`,**不动** `patch_edit/io.rs`(surgical;接受 ~40 行编码检测重复)。

## 6. 实际使用 Mock(给你一眼判断)

### Mock A — rearrange 置换(dry-run 默认)

````text
$ asq rearrange --stdin <<'EOF'
file README.md
chunk A = 1-30
chunk B = 31-50
chunk C = 51-80
rearrange A, B, C => B, C, A
EOF

rearrange README.md  (dry-run)

  chunk A  1-30   30 lines
  chunk B  31-50  20 lines
  chunk C  51-80  30 lines

  action  rearrange A, B, C => B, C, A   gap=slot
  result  B, C, A   (80 lines)

  --- a/README.md
  +++ b/README.md
  @@ -1,80 +1,80 @@
  -<A 行...>
  -<B 行...>
  -<C 行...>
  +<B 行...>
  +<C 行...>
  +<A 行...>

  No file written. Pass --yes to apply.
````

### Mock B — move 内联区间(写入)

````text
$ asq rearrange --yes --stdin <<'EOF'
file README.md
move 40-90 to after 120
EOF

rearrange README.md  (written)
  action  move 40-90 (51 lines) -> after 120
  README.md modified
````

### Mock C — gap=slot 保留隐藏行 + 提示

````text
$ asq rearrange --stdin <<'EOF'
file a.md
chunk A = 1-10
chunk B = 15-20
chunk C = 21-30
chunk D = 40-50
rearrange A, B, C, D => B, D, C, A
EOF

rearrange a.md  (dry-run)

  chunk A  1-10    chunk B  15-20   chunk C  21-30   chunk D  40-50

  action  rearrange A, B, C, D => B, D, C, A   gap=slot
  gaps    11-14 kept (slot1|slot2)   31-39 kept (slot3|slot4)

  [unified diff ...]

  No file written. Pass --yes to apply.
````

### Mock D — gap=drop 提示丢弃

````text
  action  rearrange A, B => B, A   gap=drop
  dropped 11-14 (4 lines)
  [diff ...]
````

### Mock E — 错误(不写,结构化码)

````text
$ asq rearrange --yes --stdin <<'EOF'
file a.md
chunk A = 10-20
chunk B = 15-30
rearrange A, B => B, A
EOF

error: OVERLAPPING_CHUNKS: chunks A (10-20) and B (15-30) overlap in a.md
# 退出码 1, 未写文件
````

### Mock F — JSON 输出(节选)

````json
{
  "ok": true,
  "command": "rearrange",
  "data": {
    "written": false,
    "file": "README.md",
    "chunks": { "A": {"range":"1-30","lines":30}, "B": {"range":"31-50","lines":20} },
    "action": { "type":"rearrange", "from":["A","B"], "to":["B","A"], "gap":"slot" },
    "diff": "--- a/README.md\n+++ b/README.md\n@@ ..."
  },
  "warnings": [],
  "meta": {}
}
````

## 7. 错误码 → 触发点

| code | 触发 |
|------|------|
| `INVALID_SPEC` | 语法错 / 缺 `file` / 多个 `file` |
| `MULTIPLE_ACTIONS` | spec 含 >1 条 action |
| `UNKNOWN_CHUNK` | action 引用未声明 chunk |
| `INVALID_RANGE` | `A>B` / 非数字 / `N<1` |
| `RANGE_OUT_OF_BOUNDS` | 区间超出文件行数 |
| `OVERLAPPING_CHUNKS` | rearrange 槽位重叠 |
| `ANCHOR_OUT_OF_BOUNDS` | `before/after N` 的 N 越界 |
| `ANCHOR_INSIDE_MOVED_CHUNK` | move 锚点落在被移动区间内部 |
| `REARRANGE_SET_MISMATCH` | from/to/声明集合不一致 |
| `FILE_NOT_FOUND` | `file` 指向的文件不存在 |

## 8. 测试矩阵(tests/rearrange.rs,对照 RFC 验收)

| case | 内容 | 期望 |
|------|------|------|
| 1 | 单文件 move | 区间剪切插到 after N;dry-run 不写,--yes 写 |
| 2 | 连续 chunk rearrange `A,B,C=>C,A,B` | 前 30 行变 C/A/B |
| 3 | gap=slot 含 hidden | `B,hidden1,D,C,hidden2,A` |
| 4 | gap=drop | 丢弃 11-14,提示 dropped |
| 5 | overlapping | `OVERLAPPING_CHUNKS`,不写 |
| + | newline 保持 | CRLF 文件写回仍 CRLF |
| + | multiple actions | `MULTIPLE_ACTIONS` |
