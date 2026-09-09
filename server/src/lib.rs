//! Local HTTP server startup. Routes, storage and task execution live in separate modules.
mod api;
mod client;
mod model;
mod run_log;
mod scheduler;
mod state;
mod storage;
mod task_import;

use api::router;
use client::SchoolClient;
use scheduler::method_label;
use state::AppState;
use storage::{load_settings_file, load_stored_credentials, load_ua_config, migrate_portable_data};

pub async fn serve(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let (state, mut rx) = AppState::new();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let url = format!("http://127.0.0.1:{port}");
    println!("HDU-KillCourse NEXT 已启动：{url}");
    println!("按 Ctrl+C 或在界面中点击「退出程序」关闭本地服务。");
    migrate_portable_data();
    spawn_auto_login(state.clone());
    if std::env::var("HDU_NO_OPEN").is_err() {
        // Opening the default browser is a convenience; failure is not fatal.
        let url2 = url.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = open::that(&url2) {
                println!("自动打开浏览器失败（{e}），请手动访问 {url2}");
            }
        });
    }
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = rx.changed() => {}
            }
        })
        .await?;
    println!("本地服务已关闭。");
    Ok(())
}

/// Attempt an ordered login in the background at startup, using the saved
/// credentials. Browser startup runs concurrently, so the page may open before
/// login finishes. QR login is skipped because it needs user interaction.
fn spawn_auto_login(state: AppState) {
    let stored = load_stored_credentials().unwrap_or_default();
    let ua = load_ua_config().unwrap_or_default();
    let settings = load_settings_file().unwrap_or_default();
    tokio::spawn(async move {
        match SchoolClient::login_with_order(&stored, &settings, &ua).await {
            Ok((client, method)) => {
                let mut i = state.lock();
                if i.client.is_none() {
                    i.client = Some(client);
                }
                println!("启动自动登录成功（{}）", method_label(method));
            }
            Err(e) => println!("启动自动登录未成功：{e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Settings, TaskList},
        scheduler::wait_or_stop,
    };
    use axum::body::Body;
    use axum::http::Request;
    use axum::{
        http::{header, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let (state, _rx) = AppState::new();
        router(state)
    }

    #[tokio::test]
    async fn task_import_returns_lists_without_starting_a_run() {
        let app = test_app();
        let body = serde_json::json!({
            "text": include_str!("../../examples/task-lists.json"),
            "settings": Settings::default(),
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/import")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let lists: Vec<TaskList> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].tasks.len(), 1);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(snapshot["running"], false);
        assert!(snapshot["current_run"].is_null());
    }

    #[tokio::test]
    async fn stop_interrupts_scheduled_wait() {
        let token = CancellationToken::new();
        token.cancel();
        let result =
            tokio::time::timeout(Duration::from_millis(100), wait_or_stop(&token, 3_600_000)).await;
        assert_eq!(result.unwrap(), false);
    }

    #[tokio::test]
    async fn health_endpoint_responds() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes, r#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn snapshot_starts_idle_and_logged_out() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["running"], false);
        assert_eq!(v["logged_in"], false);
        assert!(v["history"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn index_page_serves_html_with_fallback() {
        for uri in ["/", "/index.html", "/any/deep/link"] {
            let response = test_app()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "uri: {uri}");
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(content_type.starts_with("text/html"), "uri: {uri}");
        }
    }

    #[tokio::test]
    async fn missing_asset_is_not_html() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/assets/not-real.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn invalid_settings_are_rejected() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"settings":{"year":2026,"term":1,"interval_ms":1,"start_at":"","relogin_before_secs":0,"lists":[{"name":"默认清单","mode":"watch","tasks":[]}],"active_list":0}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["error"].as_str().unwrap().contains("间隔"));
    }

    #[tokio::test]
    async fn qr_poll_without_session_is_rejected() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/login/qr/poll")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["error"].as_str().unwrap().contains("二维码"));
    }
}
