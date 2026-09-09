mod activity;
mod assets;
mod auth;
mod courses;
mod settings;
mod tasks;
use crate::{model::Settings, state::AppState};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub(super) struct ApiError(pub(super) String);

impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for ApiError {
    fn from(message: &str) -> Self {
        Self(message.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": self.0 })),
        )
            .into_response()
    }
}

pub(super) type Api<T> = Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub(super) struct SettingsArg {
    pub(super) settings: Settings,
}

async fn health() -> Api<serde_json::Value> {
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn shutdown(State(state): State<AppState>) -> Api<()> {
    let _ = state.shutdown.send(true);
    Ok(Json(()))
}

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/snapshot", get(activity::snapshot))
        .route("/api/runs", get(activity::runs_get))
        .route("/api/runs/{id}", get(activity::run_get))
        .route(
            "/api/settings",
            get(settings::settings_get).post(settings::settings_save),
        )
        .route("/api/courses", get(courses::courses_get))
        .route("/api/courses/import", post(courses::courses_import))
        .route("/api/courses/fetch", post(courses::courses_fetch))
        .route(
            "/api/credentials",
            get(settings::credentials_get).post(settings::credentials_save),
        )
        .route("/api/credentials/clear", post(settings::credentials_clear))
        .route("/api/ua", get(settings::ua_get).post(settings::ua_save))
        .route("/api/login", post(auth::login))
        .route("/api/login/qr/start", post(auth::login_qr_start))
        .route("/api/login/qr/poll", post(auth::login_qr_poll))
        .route("/api/login/qr/cancel", post(auth::login_qr_cancel))
        .route("/api/logout", post(auth::logout))
        .route("/api/tasks/import", post(tasks::tasks_import))
        .route("/api/tasks/start", post(tasks::tasks_start))
        .route("/api/tasks/stop", post(tasks::tasks_stop))
        .route("/api/shutdown", post(shutdown))
        .fallback(assets::static_fallback)
        .with_state(state)
}
