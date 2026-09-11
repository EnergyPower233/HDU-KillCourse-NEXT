use super::{Api, ApiError, SettingsArg};
use crate::{
    model::{self, Settings},
    state::AppState,
};
use axum::{Extension, Json};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct CredentialsArg {
    credentials: model::StoredCredentials,
}

#[derive(Deserialize)]
pub(crate) struct UaArg {
    ua: model::UaConfig,
}

pub(crate) async fn settings_get(Extension(state): Extension<AppState>) -> Api<Settings> {
    state.store.load_settings_file().map(Json).map_err(ApiError)
}

pub(crate) async fn settings_save(
    Extension(state): Extension<AppState>,
    Json(arg): Json<SettingsArg>,
) -> Api<()> {
    arg.settings.validate(false, arg.settings.active_list)?;
    state.store.save_json("settings.json", &arg.settings)?;
    Ok(Json(()))
}

pub(crate) async fn credentials_get(
    Extension(state): Extension<AppState>,
) -> Api<model::StoredCredentials> {
    state
        .store
        .load_stored_credentials()
        .map(Json)
        .map_err(ApiError)
}

pub(crate) async fn credentials_save(
    Extension(state): Extension<AppState>,
    Json(arg): Json<CredentialsArg>,
) -> Api<()> {
    let mut c = arg.credentials;
    c.order = c.normalized_order();
    for field in [
        &c.cas_username,
        &c.cas_password,
        &c.newjw_username,
        &c.newjw_password,
        &c.session_id,
        &c.route,
    ] {
        if field.len() > 1024 {
            return Err(ApiError("凭证字段过长，请检查输入".into()));
        }
    }
    state.store.save_json("credentials.json", &c)?;
    // Restrict the file to the current user where the platform allows it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(path) = state.store.path("credentials.json") {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
    Ok(Json(()))
}

pub(crate) async fn credentials_clear(Extension(state): Extension<AppState>) -> Api<()> {
    let path = state.store.path("credentials.json")?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|_| ApiError("无法清除凭证文件".into()))?;
    }
    Ok(Json(()))
}

pub(crate) async fn ua_get(Extension(state): Extension<AppState>) -> Api<model::UaConfig> {
    state.store.load_ua_config().map(Json).map_err(ApiError)
}

pub(crate) async fn ua_save(
    Extension(state): Extension<AppState>,
    Json(arg): Json<UaArg>,
) -> Api<()> {
    let mut ua = arg.ua;
    ua.normalize();
    if ua.browser_ua.len() > 512 || ua.fixed_version.len() > 128 {
        return Err(ApiError("User-Agent 内容过长".into()));
    }
    if ua.rotate_list.len() > 50 || ua.rotate_list.iter().any(|s| s.len() > 512) {
        return Err(ApiError("轮换列表最多 50 条，每条不超过 512 字符".into()));
    }
    state.store.save_json("ua.json", &ua)?;
    Ok(Json(()))
}
