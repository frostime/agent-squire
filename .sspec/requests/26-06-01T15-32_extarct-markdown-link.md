---
name: extarct-markdown-link
created: 2026-06-01 15:32:24
status: DONE
kind: directive
attach-change: .sspec/changes/26-06-01T15-46_extarct-markdown-link/spec.md
tldr: 'Added md-links CLI to extract Markdown references and resolve file targets for graph building.'
---
<!-- MUST follow frontmatter schema:
status: OPEN | DOING | DONE | CLOSED
tldr: One-sentence summary for list views — fill this! -->

# Request: extarct-markdown-link

## Requirement
<!-- What is not working or missing -->

我希望给 asq 增加一个提取 markdown 中 Link 引用的 CLI，包括:

- URL
- 文件引用 (需要核实文件存在性)

**核心想法**: 希望 AGENT 利用这个 CLI 能快速建立各个 markdown 文件之间的引用关联网络

## Initial Direction
<!-- Your rough idea or preferred direction — details are fine but not required.
This becomes the starting point for the change's spec.md Approach. -->

**匹配模式**
- markdown 自带的链接格式， `[]()` 和 `![]()`
- 类似 Ob 风格的 Wiki 引用 `[[]]`， `(())`
- Inline 代码块, `<path>`

**链接模式**
- 最简单的 URL, `http:// | https://`
- 绝对路径 `C:/User/a.txt`, `/home/me/.bashrc`
- 相对路径
  - 相对于文件所在的地方， `./assets/a.png`
  - 相对于 workspace 目录所在的路径，`src/index.js` + `/src/index.js` 两种

最后用合适的路径打印出来

**CLI**

- 类似 md-toc 那样支持单、多文件的输入
- 要求有一个可选的 `--workspace` 参数，默认是 AGENT `cwd`

"@附加参考" {
我记得 use-project-memory SKILL 中有 script 脚本会解析 memory 文件中关联的引用
也许他的 script 代码逻辑有一定参考价值。（py 代码，但是可以参考思路做法？）
}

## Success Criteria
<!-- Conditions that indicate the problem has been resolved and meets the user's intention -->

本质上：就是实现 markdown file based 的 ref graph

---

## @AGENT
<!-- What should Agent do to implement this request -->
Adhere to the SSPEC protocol and commence development from the current Request file, following the SSPEC Change Lifecycle.
Next step: Read `sspec-clarify` SKILL + `sspec-design` SKILL + `sspec change new --from <this>`.
