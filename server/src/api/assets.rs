use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../dist/"]
struct Assets;

fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "json" => "application/json",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

pub(super) async fn static_fallback(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(file) = Assets::get(path) {
        let mut res = Response::new(axum::body::Body::from(file.data.into_owned()));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static(content_type(path)),
        );
        if path.starts_with("assets/") {
            res.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
        } else {
            res.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-cache"),
            );
        }
        res
    } else if path.starts_with("assets/") {
        (StatusCode::NOT_FOUND, "asset not found").into_response()
    } else if let Some(index) = Assets::get("index.html") {
        // SPA fallback for any non-asset path.
        let mut res = Response::new(axum::body::Body::from(index.data.into_owned()));
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("text/html; charset=utf-8"),
        );
        res.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-cache"),
        );
        res
    } else {
        (StatusCode::NOT_FOUND, "frontend not built").into_response()
    }
}
