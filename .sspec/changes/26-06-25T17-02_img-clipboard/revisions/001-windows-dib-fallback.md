---
revision: 1
date: 2026-06-25T18:05:11
trigger: "review-feedback"
---

# windows-dib-fallback

## Reason

User reported that Windows clipboard history (`Win+V`) and `Ctrl+V` could paste an image, while `asq img` failed with `arboard::get_image()` conversion error. Local diagnostics showed the clipboard exposed `DeviceIndependentBitmap` (`CF_DIB`), `Format17` (`CF_DIBV5`), and a screenshot-tool custom format (`PixPinData`).

## Changes

### Spec Impact

BC-2 remains the same user-visible contract: `asq img` reads clipboard images and writes PNG paths. The Windows implementation must support clipboard images exposed as `CF_DIB` when `arboard` cannot convert `CF_DIBV5`.

### Design Impact

Add Windows fallback after `arboard::get_image()` failure:

```text
arboard::get_image()
  ├─ ok → encode RGBA as PNG
  └─ err on Windows → clipboard-win read CF_DIB / CF_DIBV5
        → image::codecs::bmp::BmpDecoder::new_without_file_header
        → RGBA bytes
        → encode PNG
```

Dependency delta:

```toml
image = { version = "0.25", default-features = false, features = ["png", "bmp"] }

[target.'cfg(windows)'.dependencies]
clipboard-win = "5.4"
```

### Task Impact

Added and completed review feedback work in `tasks.md`: Windows DIB fallback implementation plus quality-gate re-run.
