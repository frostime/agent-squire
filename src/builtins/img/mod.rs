use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use arboard::Clipboard as ArClipboard;
use chrono::Local;
use clap::Args;
use image::{ColorType, ImageFormat};
use serde::Serialize;
use uuid::Uuid;

use crate::builtins::imgweb;
use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

#[derive(Args, Debug)]
#[command(
    long_about = "Save clipboard images or start the image web UI.

By default, img reads the current system clipboard image, saves it as a persistent PNG under the system temp agent-temp directory, and prints the saved path.",
    after_help = "Examples:
    asq img
    asq img --clipboard
    asq img --web
    asq img --web --no-open"
)]
pub struct ImgArgs {
    #[arg(long, conflicts_with = "clipboard", help = "Start the image web UI")]
    pub web: bool,

    #[arg(
        long,
        help = "Read the current clipboard image and save it as PNG (default)"
    )]
    pub clipboard: bool,

    #[arg(long, requires = "web", help = "Do not open the browser automatically")]
    pub no_open: bool,

    #[arg(
        long,
        requires = "web",
        value_name = "MB",
        default_value_t = imgweb::MAX_UPLOAD_MB,
        help = "Maximum web request body size in MB"
    )]
    pub max_mb: usize,
}

#[derive(Debug, Serialize)]
struct ImgData {
    path: String,
    uri: String,
    mime: &'static str,
    size_bytes: u64,
}

struct RgbaImage {
    width: usize,
    height: usize,
    bytes: Vec<u8>,
}

pub fn run(args: ImgArgs, ctx: &CommandContext) -> Result<u8> {
    if args.web {
        return imgweb::run(
            imgweb::ImgWebArgs {
                no_open: args.no_open,
                max_mb: args.max_mb,
            },
            ctx,
        );
    }

    let data = save_clipboard_image()?;
    print_img_data(&data, ctx)?;
    Ok(0)
}

fn save_clipboard_image() -> Result<ImgData> {
    let image = read_clipboard_image()?;

    let dir = clipboard_session_dir();
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

    let filename = format!("clipboard-{}.png", Uuid::new_v4().simple());
    let path = dir.join(filename);
    write_png(&image.bytes, image.width, image.height, &path)
        .with_context(|| format!("failed to write {}", path.display()))?;

    let size_bytes = fs::metadata(&path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();

    Ok(ImgData {
        uri: file_uri(&path),
        path: path.display().to_string(),
        mime: "image/png",
        size_bytes,
    })
}

fn read_clipboard_image() -> Result<RgbaImage> {
    let mut clipboard = ArClipboard::new().context("failed to open clipboard")?;
    match clipboard.get_image() {
        Ok(image) => Ok(RgbaImage {
            width: image.width,
            height: image.height,
            bytes: image.bytes.into_owned(),
        }),
        Err(err) => read_clipboard_image_fallback()
            .with_context(|| format!("clipboard does not contain a supported image: {err}")),
    }
}

#[cfg(windows)]
fn read_clipboard_image_fallback() -> Result<RgbaImage> {
    use clipboard_win::{Clipboard as WinClipboard, formats, raw};

    let _clipboard = WinClipboard::new_attempts(5).context("failed to open Windows clipboard")?;
    let mut errors = Vec::new();

    for (format, name) in [
        (formats::CF_DIB, "DeviceIndependentBitmap/CF_DIB"),
        (formats::CF_DIBV5, "Format17/CF_DIBV5"),
    ] {
        if !clipboard_win::is_format_avail(format) {
            continue;
        }

        let mut data = Vec::new();
        if let Err(err) = raw::get_vec(format, &mut data) {
            errors.push(format!("{name}: read failed: {err}"));
            continue;
        }

        match decode_dib(&data) {
            Ok(image) => return Ok(image),
            Err(err) => errors.push(format!("{name}: decode failed: {err:#}")),
        }
    }

    if errors.is_empty() {
        bail!("Windows clipboard has no DIB image format");
    }
    bail!(
        "Windows clipboard image formats could not be decoded: {}",
        errors.join("; ")
    )
}

#[cfg(not(windows))]
fn read_clipboard_image_fallback() -> Result<RgbaImage> {
    bail!("no platform clipboard image fallback is available")
}

#[cfg(windows)]
fn decode_dib(data: &[u8]) -> Result<RgbaImage> {
    use std::io::Cursor;

    use image::codecs::bmp::BmpDecoder;
    use image::{DynamicImage, ImageDecoder};

    let decoder = BmpDecoder::new_without_file_header(Cursor::new(data))?;
    let (width, height) = decoder.dimensions();
    let bytes = DynamicImage::from_decoder(decoder)?.into_rgba8().into_raw();
    Ok(RgbaImage {
        width: width as usize,
        height: height as usize,
        bytes,
    })
}

fn write_png(bytes_rgba: &[u8], width: usize, height: usize, path: &Path) -> Result<()> {
    let expected = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .context("image dimensions overflow")?;
    if bytes_rgba.len() != expected {
        bail!(
            "invalid RGBA buffer length: got {}, expected {}",
            bytes_rgba.len(),
            expected
        );
    }

    let width = u32::try_from(width).context("image width exceeds u32")?;
    let height = u32::try_from(height).context("image height exceeds u32")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    image::save_buffer_with_format(
        path,
        bytes_rgba,
        width,
        height,
        ColorType::Rgba8,
        ImageFormat::Png,
    )?;
    Ok(())
}

fn clipboard_session_dir() -> PathBuf {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let session_name = format!("clip-{stamp}-{}", short_uuid());
    std::env::temp_dir()
        .join("agent-temp")
        .join("images")
        .join(session_name)
}

fn short_uuid() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

fn print_img_data(data: &ImgData, ctx: &CommandContext) -> Result<()> {
    match ctx.print {
        PrintMode::Json => {
            let payload = Envelope::new("img", data);
            output::print_json(&payload)?;
        }
        _ => {
            println!("{}", data.path);
        }
    }
    Ok(())
}

fn file_uri(path: &Path) -> String {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_png_writes_png_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("one.png");
        let rgba = [255, 0, 0, 255];

        write_png(&rgba, 1, 1, &path).unwrap();

        let bytes = fs::read(path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn write_png_rejects_invalid_buffer_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.png");
        let err = write_png(&[0, 0, 0], 1, 1, &path).unwrap_err();

        assert!(err.to_string().contains("invalid RGBA buffer length"));
        assert!(!path.exists());
    }

    #[test]
    fn file_uri_formats_windows_like_paths() {
        let uri = file_uri(Path::new("C:\\Temp\\image.png"));

        assert_eq!(uri, "file:///C:/Temp/image.png");
    }
}
