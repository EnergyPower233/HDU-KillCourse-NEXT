use super::{Api, ApiError, SettingsArg};
use crate::{
    client::{self, SchoolClient},
    model::{Credentials, Settings},
    state::{AppState, QrSession},
};
use axum::{Extension, Json};
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct LoginArg {
    auth: Credentials,
    settings: Settings,
}

#[derive(Serialize)]
pub(crate) struct QrImage {
    /// Base64 PNG of the QR code.
    image: String,
}

#[derive(Serialize)]
pub(crate) struct QrPoll {
    status: String,
    message: String,
}

fn begin_login(state: &AppState, qr: bool) -> Result<u64, ApiError> {
    let mut inner = state.lock();
    if inner.cancel.is_some() || inner.authenticating {
        return Err(ApiError("请先停止当前账号任务或等待登录完成".into()));
    }
    inner.auth_generation += 1;
    inner.authenticating = true;
    inner.qr_starting = qr;
    inner.client = None;
    inner.qr = None;
    Ok(inner.auth_generation)
}

fn validate_term(settings: &Settings) -> Result<(), ApiError> {
    if !(2000..=2100).contains(&settings.year) || ![1, 2].contains(&settings.term) {
        return Err(ApiError("请先设置正确的学年学期".into()));
    }
    Ok(())
}

pub(crate) async fn login(
    Extension(state): Extension<AppState>,
    Json(arg): Json<LoginArg>,
) -> Api<()> {
    validate_term(&arg.settings)?;
    let generation = begin_login(&state, false)?;
    let ua = state.store.load_ua_config().unwrap_or_default();
    let result = SchoolClient::login(arg.auth, &arg.settings, &ua).await;
    let mut inner = state.lock();
    if inner.auth_generation != generation {
        return Err(ApiError("本次登录已取消".into()));
    }
    inner.authenticating = false;
    inner.qr_starting = false;
    inner.client = Some(result.map_err(ApiError)?);
    Ok(Json(()))
}

pub(crate) async fn logout(Extension(state): Extension<AppState>) -> Api<()> {
    let mut inner = state.lock();
    if inner.cancel.is_some() {
        return Err(ApiError("请先停止当前账号任务".into()));
    }
    inner.auth_generation += 1;
    inner.authenticating = false;
    inner.qr_starting = false;
    inner.client = None;
    inner.qr = None;
    Ok(Json(()))
}

pub(crate) async fn login_qr_start(
    Extension(state): Extension<AppState>,
    Json(arg): Json<SettingsArg>,
) -> Api<QrImage> {
    validate_term(&arg.settings)?;
    let generation = begin_login(&state, true)?;
    let result = async {
        let ua = state.store.load_ua_config().unwrap_or_default();
        let client = SchoolClient::anonymous(&ua)?;
        let execution = client.cas_execution().await?;
        let (csrf_key, csrf_value) = client::generate_csrf();
        let id = client.qr_login_id(&csrf_key, &csrf_value).await?;
        let bytes = client.qr_code(&id).await?;
        Ok::<_, String>((
            QrSession {
                client,
                id,
                execution,
                csrf_key,
                csrf_value,
                settings: arg.settings,
            },
            bytes,
        ))
    }
    .await;
    let mut inner = state.lock();
    if inner.auth_generation != generation {
        return Err(ApiError("本次扫码登录已取消".into()));
    }
    inner.authenticating = false;
    inner.qr_starting = false;
    let (session, bytes) = result.map_err(ApiError)?;
    inner.qr = Some(session);
    Ok(Json(QrImage {
        image: base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

pub(crate) async fn login_qr_poll(Extension(state): Extension<AppState>) -> Api<QrPoll> {
    let (session, generation) = {
        let inner = state.lock();
        (
            inner
                .qr
                .clone()
                .ok_or(ApiError("请先获取登录二维码".into()))?,
            inner.auth_generation,
        )
    };
    let scan = session
        .client
        .qr_scan(&session.id, &session.csrf_key, &session.csrf_value)
        .await?;
    let confirmed = if scan.code == 200 && !scan.data.trim().is_empty() {
        let mut client = session.client.clone();
        Some(
            client
                .qr_login_complete(&session.execution, &scan.data, &session.settings)
                .await
                .map(|_| client),
        )
    } else {
        None
    };
    let mut inner = state.lock();
    if inner.auth_generation != generation || inner.qr.as_ref().map(|q| &q.id) != Some(&session.id)
    {
        return Err(ApiError("本次扫码登录已取消或被替换".into()));
    }
    if let Some(result) = confirmed {
        inner.qr = None;
        if inner.cancel.is_some() {
            return Err(ApiError("账号任务正在运行，无法替换会话".into()));
        }
        inner.client = Some(result.map_err(ApiError)?);
        Ok(Json(QrPoll {
            status: "confirmed".into(),
            message: "扫码登录成功".into(),
        }))
    } else if ["过期", "失效", "无效", "不存在"]
        .iter()
        .any(|k| scan.message.contains(k))
    {
        inner.qr = None;
        Ok(Json(QrPoll {
            status: "expired".into(),
            message: scan.message,
        }))
    } else {
        Ok(Json(QrPoll {
            status: "waiting".into(),
            message: if scan.message.trim().is_empty() {
                "等待扫码".into()
            } else {
                scan.message
            },
        }))
    }
}

pub(crate) async fn login_qr_cancel(Extension(state): Extension<AppState>) -> Api<()> {
    let mut inner = state.lock();
    // Only cancel QR work owned by this account; password/automatic login is independent.
    if inner.qr.is_some() || inner.qr_starting {
        inner.auth_generation += 1;
        inner.authenticating = false;
        inner.qr_starting = false;
        inner.qr = None;
    }
    Ok(Json(()))
}
