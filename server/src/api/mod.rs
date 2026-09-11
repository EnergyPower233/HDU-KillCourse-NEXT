mod accounts;
mod activity;
mod assets;
mod auth;
mod courses;
mod settings;
mod tasks;
use crate::{
    accounts::{Accounts, DEFAULT_ACCOUNT},
    model::Settings,
};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
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

async fn shutdown(State(accounts): State<Accounts>) -> Api<()> {
    if accounts
        .states()
        .iter()
        .any(|(_, state)| state.lock().cancel.is_some())
    {
        return Err(ApiError(
            "仍有账号正在运行，请先停止各账号任务后退出".into(),
        ));
    }
    let _ = accounts.get(DEFAULT_ACCOUNT)?.shutdown.send(true);
    Ok(Json(()))
}

async fn select_account(
    State(accounts): State<Accounts>,
    mut request: Request,
    next: Next,
) -> Response {
    let id = match request.headers().get("x-hdu-account") {
        Some(value) => match value.to_str() {
            Ok(id) => id,
            Err(_) => return ApiError("账号标识无效".into()).into_response(),
        },
        None => DEFAULT_ACCOUNT,
    };
    match accounts.get(id) {
        Ok(state) => {
            request.extensions_mut().insert(state);
            next.run(request).await
        }
        Err(error) => ApiError(error).into_response(),
    }
}

pub(crate) fn router(state: Accounts) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/accounts", get(accounts::list).post(accounts::create))
        .route("/api/accounts/{id}", post(accounts::rename))
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            select_account,
        ))
        .with_state(state)
}
