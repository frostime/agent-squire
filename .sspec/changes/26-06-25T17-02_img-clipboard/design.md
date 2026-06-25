---
change: "img-clipboard"
created: 2026-06-25T17:02:39
---

# Design: img-clipboard

## CLI Surface

| Command | Mode | Behavior |
|---|---|---|
| `asq img` | clipboard | Save clipboard image as PNG, print path |
| `asq img --clipboard` | clipboard | Same as default; explicit mode flag |
| `asq img --web` | web | Start existing `imgweb` server |
| `asq img --web --no-open` | web | Existing web behavior without browser open |
| `asq img --web --max-mb 50` | web | Existing web behavior with body limit |
| `asq imgweb ...` | hidden web legacy | Existing command remains supported but hidden from primary discovery |

Mode constraints:

```rust
#[derive(clap::Args, Debug)]
pub struct ImgArgs {
    #[arg(long, conflicts_with = "clipboard")]
    pub web: bool,

    #[arg(long)]
    pub clipboard: bool,

    #[arg(long, requires = "web")]
    pub no_open: bool,

    #[arg(long, requires = "web", default_value_t = imgweb::MAX_UPLOAD_MB)]
    pub max_mb: usize,
}
```

`clipboard` is a no-op selector because clipboard mode is the default; it exists to make scripts self-documenting.

## Module Structure

```text
src/builtins/
├── img/
│   └── mod.rs          # new general image CLI
└── imgweb/
    └── mod.rs          # existing web UI, retained
```

Dispatch:

```text
CliCommand::Img(args)
  → builtins::img::run(args, ctx)
      ├─ args.web == true  → builtins::imgweb::run(ImgWebArgs { no_open, max_mb }, ctx)
      └─ args.web == false → save_clipboard_image(ctx)

CliCommand::ImgWeb(args)
  → builtins::imgweb::run(args, ctx)   # hidden legacy compatibility path
```

## Clipboard Flow

```text
asq img
  → arboard::Clipboard::new()
  → clipboard.get_image()
  → validate RGBA byte length
  → image::save_buffer_with_format(..., ImageFormat::Png)
  → fs::metadata(path).len()
  → print compact path OR JSON envelope
```

Core helpers:

```rust
#[derive(Debug, serde::Serialize)]
struct ImgData {
    path: String,
    uri: String,
    mime: &'static str,      // "image/png"
    size_bytes: u64,
}

fn save_clipboard_image(ctx: &CommandContext) -> anyhow::Result<ImgData>;
fn write_png(bytes_rgba: &[u8], width: usize, height: usize, path: &Path) -> anyhow::Result<()>;
fn clipboard_session_dir() -> PathBuf;
fn file_uri(path: &Path) -> String;
fn print_img_data(data: &ImgData, ctx: &CommandContext) -> anyhow::Result<()>;
```

Storage layout:

```text
<system-temp>/agent-temp/images/
└── clip-YYYYMMDD-HHMMSS-<uuid8>/
    └── clipboard-<uuid>.png
```

The generated file is persistent for the session after process exit, matching the existing `imgweb` principle of keeping image paths usable.

## Output Contract

Compact/text/raw/ndjson modes:

```text
C:\Users\...\Temp\agent-temp\images\clip-20260625-170200-ab12cd34\clipboard-....png
```

JSON mode:

```json
{
  "ok": true,
  "command": "img",
  "data": {
    "path": "C:\\Users\\...\\clipboard-....png",
    "uri": "file:///C:/Users/.../clipboard-....png",
    "mime": "image/png",
    "size_bytes": 12345
  },
  "warnings": [],
  "meta": {}
}
```

## Dependency Plan

```toml
arboard = "3"
image = { version = "0.25", default-features = false, features = ["png"] }
```

Responsibilities:

| Crate | Use | Boundary |
|---|---|---|
| `arboard` | Read system clipboard image | No clipboard writes in this change |
| `image` | Encode RGBA bytes to PNG | No image decoding/transcoding feature beyond PNG output |

## Error Boundaries

| Case | Behavior |
|---|---|
| Clipboard unavailable | non-zero CLI error: failed to open/read clipboard |
| Clipboard has text/no image | non-zero CLI error: clipboard does not contain an image |
| Image buffer dimensions invalid | non-zero CLI error before writing |
| PNG encode/write failure | non-zero CLI error with target path context |
| `asq img --web --max-mb 0` | same validation as existing `imgweb` |
| `asq img --no-open` without `--web` | clap usage error |

## Compatibility

- `imgweb` command name, flags, server routes, browser UI, and prompt rendering remain executable for existing users/scripts.
- `imgweb` becomes hidden legacy: omitted from help/list/README as a recommended public entrypoint.
- New `img` command may shadow an external mapping named `img`; built-ins already take precedence in the CLI enum model.
