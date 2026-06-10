use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::Local;
use clap::Args;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::cli::CommandContext;

const INDEX_HTML: &str = include_str!("web/index.html");
const MAX_UPLOAD_MB: usize = 25;

#[derive(Args, Debug)]
#[command(
    long_about = "Start a local web UI for composing multi-image prompts.\n\nThe server binds to 127.0.0.1, stores uploaded images in a temporary session directory, and keeps files after exit so generated file:// prompt references remain usable.",
    after_help = "Examples:\n  squire imgweb\n  squire imgweb --no-open"
)]
pub struct ImgWebArgs {
    #[arg(long, help = "Do not open the browser automatically")]
    pub no_open: bool,

    #[arg(
        long,
        value_name = "MB",
        default_value_t = MAX_UPLOAD_MB,
        help = "Maximum request body size in MB"
    )]
    pub max_mb: usize,
}

#[derive(Debug, Clone)]
struct App {
    token: String,
    session_dir: PathBuf,
    images_dir: PathBuf,
    inner: Arc<Mutex<AppInner>>,
}

#[derive(Debug, Default)]
struct AppInner {
    images: Vec<ImageItem>,
    next_order: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ImageItem {
    id: String,
    slug: String,
    hint: String,
    filename: String,
    path: String,
    uri: String,
    preview_url: String,
    order: usize,
    source: String,
    size_bytes: usize,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    data: T,
}

#[derive(Debug, Serialize)]
struct SessionData {
    session_dir: String,
    images: Vec<ImageItem>,
}

#[derive(Debug, Deserialize)]
struct UpdateImageRequest {
    slug: Option<String>,
    hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReorderRequest {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AuthQuery {
    token: Option<String>,
}

#[derive(Debug, Serialize)]
struct PromptData {
    format: &'static str,
    prompt: String,
}

pub fn run(args: ImgWebArgs, _ctx: &CommandContext) -> Result<u8> {
    if args.max_mb == 0 {
        anyhow::bail!("--max-mb must be >= 1");
    }

    let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    rt.block_on(run_server(args))
}

async fn run_server(args: ImgWebArgs) -> Result<u8> {
    let app = App::new()?;
    let router = router(app.clone(), args.max_mb);
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .context("failed to bind local web server")?;
    let addr = listener.local_addr()?;
    let url = format!("http://{addr}/?token={}", app.token);

    println!("Image prompt composer: {url}");
    println!("Storage: {}", app.session_dir.display());
    println!("Press Ctrl+C to stop.");

    if !args.no_open
        && let Err(err) = open::that(&url)
    {
        eprintln!("warning: failed to open browser: {err}");
    }

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("web server failed")?;

    Ok(0)
}

fn router(app: App, max_mb: usize) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/v1/img/session", get(get_session))
        .route("/api/v1/img/images", post(upload_image))
        .route("/api/v1/img/images/reorder", post(reorder_images))
        .route(
            "/api/v1/img/images/{id}",
            patch(update_image).delete(delete_image),
        )
        .route("/api/v1/img/clear", post(clear_images))
        .route("/api/v1/img/prompt", get(get_prompt))
        .route("/api/v1/img/files/{id}", get(get_file))
        .layer(DefaultBodyLimit::max(max_mb * 1024 * 1024))
        .with_state(app)
}

impl App {
    fn new() -> Result<Self> {
        let token = Uuid::new_v4().simple().to_string();
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let session_name = format!("web-{stamp}-{}", &token[..8]);
        let session_dir = std::env::temp_dir()
            .join("agent-temp")
            .join("images")
            .join(session_name);
        let images_dir = session_dir.join("images");
        fs::create_dir_all(&images_dir)
            .with_context(|| format!("failed to create {}", images_dir.display()))?;
        Ok(Self {
            token,
            session_dir,
            images_dir,
            inner: Arc::new(Mutex::new(AppInner::default())),
        })
    }
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn get_session(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }
    let images = sorted_images(&app);
    ok(SessionData {
        session_dir: app.session_dir.display().to_string(),
        images,
    })
}

async fn upload_image(
    State(app): State<App>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }

    let mut file_bytes = None;
    let mut original_filename = None;
    let mut content_type = None;
    let mut slug = None;
    let mut hint = String::new();
    let mut source = String::from("upload");

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(err) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    format!("invalid multipart body: {err}"),
                );
            }
        };
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                original_filename = field.file_name().map(ToOwned::to_owned);
                content_type = field.content_type().map(ToOwned::to_owned);
                let bytes = match field.bytes().await {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        return error(
                            StatusCode::BAD_REQUEST,
                            format!("failed to read file: {err}"),
                        );
                    }
                };
                file_bytes = Some(bytes.to_vec());
            }
            "slug" => slug = read_text_field(field).await,
            "hint" => hint = read_text_field(field).await.unwrap_or_default(),
            "source" => source = read_text_field(field).await.unwrap_or(source),
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        return error(StatusCode::BAD_REQUEST, "missing file field".to_string());
    };
    if bytes.is_empty() {
        return error(StatusCode::BAD_REQUEST, "empty file".to_string());
    }

    let ext = extension_from(content_type.as_deref(), original_filename.as_deref());
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return error(
            StatusCode::BAD_REQUEST,
            "unsupported image type".to_string(),
        );
    }

    let id = format!("img_{}", Uuid::new_v4().simple());
    let mut inner = app.inner.lock().expect("app state lock poisoned");
    inner.next_order += 1;
    let order = inner.next_order;
    let slug = sanitize_slug(slug.as_deref()).unwrap_or_else(|| format!("image-{order}"));
    let filename_slug = slug_to_filename(&slug);
    let filename = format!("{order:03}-{filename_slug}-{id}.{ext}");
    let path = app.images_dir.join(&filename);
    if let Err(err) = fs::write(&path, &bytes) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to save image: {err}"),
        );
    }
    let item = ImageItem {
        id: id.clone(),
        slug,
        hint,
        filename,
        path: path.display().to_string(),
        uri: file_uri(&path),
        preview_url: format!("/api/v1/img/files/{id}"),
        order,
        source,
        size_bytes: bytes.len(),
    };
    inner.images.push(item.clone());
    ok(item)
}

async fn update_image(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateImageRequest>,
) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }
    let mut inner = app.inner.lock().expect("app state lock poisoned");
    let Some(item) = inner.images.iter_mut().find(|item| item.id == id) else {
        return error(StatusCode::NOT_FOUND, "image not found".to_string());
    };
    if let Some(slug) = req.slug
        && let Some(slug) = sanitize_slug(Some(&slug))
    {
        item.slug = slug;
    }
    if let Some(hint) = req.hint {
        item.hint = hint;
    }
    ok(item.clone())
}

async fn delete_image(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }
    let mut inner = app.inner.lock().expect("app state lock poisoned");
    let Some(index) = inner.images.iter().position(|item| item.id == id) else {
        return error(StatusCode::NOT_FOUND, "image not found".to_string());
    };
    let item = inner.images.remove(index);
    let _ = fs::remove_file(app.images_dir.join(&item.filename));
    ok(sorted_images_locked(&inner))
}

async fn reorder_images(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ReorderRequest>,
) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }
    let mut inner = app.inner.lock().expect("app state lock poisoned");
    for (index, id) in req.ids.iter().enumerate() {
        if let Some(item) = inner.images.iter_mut().find(|item| &item.id == id) {
            item.order = index + 1;
        }
    }
    ok(sorted_images_locked(&inner))
}

async fn clear_images(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }
    let mut inner = app.inner.lock().expect("app state lock poisoned");
    for item in &inner.images {
        let _ = fs::remove_file(app.images_dir.join(&item.filename));
    }
    inner.images.clear();
    ok(sorted_images_locked(&inner))
}

async fn get_prompt(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(resp) = require_token(&app, &headers) {
        return *resp;
    }
    ok(PromptData {
        format: "markdown",
        prompt: render_prompt(&sorted_images(&app)),
    })
}

async fn get_file(
    State(app): State<App>,
    headers: HeaderMap,
    Query(auth): Query<AuthQuery>,
    Path(id): Path<String>,
) -> Response {
    if !token_matches(&app, &headers, auth.token.as_deref()) {
        return error(StatusCode::UNAUTHORIZED, "invalid token".to_string());
    }
    let item = {
        let inner = app.inner.lock().expect("app state lock poisoned");
        inner.images.iter().find(|item| item.id == id).cloned()
    };
    let Some(item) = item else {
        return error(StatusCode::NOT_FOUND, "image not found".to_string());
    };
    let path = app.images_dir.join(&item.filename);
    match fs::read(path) {
        Ok(bytes) => {
            let mut resp = Response::new(Body::from(bytes));
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                content_type_from_filename(&item.filename),
            );
            resp
        }
        Err(err) => error(
            StatusCode::NOT_FOUND,
            format!("failed to read image: {err}"),
        ),
    }
}

async fn read_text_field(field: axum::extract::multipart::Field<'_>) -> Option<String> {
    field
        .text()
        .await
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn require_token(app: &App, headers: &HeaderMap) -> std::result::Result<(), Box<Response>> {
    if token_matches(app, headers, None) {
        Ok(())
    } else {
        Err(Box::new(error(
            StatusCode::UNAUTHORIZED,
            "invalid token".to_string(),
        )))
    }
}

fn token_matches(app: &App, headers: &HeaderMap, query_token: Option<&str>) -> bool {
    let header_token = headers
        .get("x-squire-token")
        .and_then(|value| value.to_str().ok());
    header_token == Some(app.token.as_str()) || query_token == Some(app.token.as_str())
}

fn ok<T: Serialize>(data: T) -> Response {
    Json(ApiResponse { ok: true, data }).into_response()
}

fn error(status: StatusCode, message: String) -> Response {
    let body = serde_json::json!({ "ok": false, "error": message });
    (status, Json(body)).into_response()
}

fn sorted_images(app: &App) -> Vec<ImageItem> {
    let inner = app.inner.lock().expect("app state lock poisoned");
    sorted_images_locked(&inner)
}

fn sorted_images_locked(inner: &AppInner) -> Vec<ImageItem> {
    let mut images = inner.images.clone();
    images.sort_by_key(|item| item.order);
    images
}

fn render_prompt(images: &[ImageItem]) -> String {
    if images.is_empty() {
        return String::new();
    }
    let mut out = String::from("Image materials:\n\n");
    for (index, image) in images.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", index + 1, image.slug));
        out.push_str(&format!("   Path: {}\n", image.path));
        if !image.hint.trim().is_empty() {
            out.push_str(&format!("   Hint: {}\n", image.hint.trim()));
        }
        if index + 1 != images.len() {
            out.push('\n');
        }
    }
    out
}

/// Slug stored in memory: allow Unicode letters/digits + common punctuation,
/// strip only filesystem-dangerous characters (/ \ NUL : * ? " < > |).
fn sanitize_slug(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let slug: String = raw
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        })
        .collect();
    let slug = slug.trim().to_string();
    if slug.is_empty() { None } else { Some(slug) }
}

/// Safe ASCII filename component derived from slug (used only in filenames).
fn slug_to_filename(slug: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in slug.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "img".to_string()
    } else {
        out
    }
}

fn extension_from(content_type: Option<&str>, filename: Option<&str>) -> String {
    if let Some(ext) = content_type.and_then(ext_from_mime) {
        return ext.to_string();
    }
    filename
        .and_then(|name| {
            name.rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "png".to_string())
}

fn ext_from_mime(mime: &str) -> Option<&'static str> {
    match mime.split(';').next()?.trim() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

fn content_type_from_filename(filename: &str) -> HeaderValue {
    let value = match filename.rsplit_once('.').map(|(_, ext)| ext) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    };
    HeaderValue::from_static(value)
}

fn file_uri(path: &std::path::Path) -> String {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}
