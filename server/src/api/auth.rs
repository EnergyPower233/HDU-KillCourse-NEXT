use super::{Api, ApiError, SettingsArg};
use crate::{
    client::{self, SchoolClient},
    model::{Credentials, Settings},
    state::{AppState, QrSession},
    storage::load_ua_config,
};
use axum::{extract::State, Json};
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

pub(crate) async fn login(State(state): State<AppState>, Json(arg): Json<LoginArg>) -> Api<()> {
    if !(2000..=2100).contains(&arg.settings.year) || ![1, 2].contains(&arg.settings.term) {
        return Err(ApiError("请先设置正确的学年学期".into()));
    }
    {
        let mut i = state.lock();
        if i.cancel.is_some() || i.authenticating {
            return Err(ApiError("请先停止任务或等待登录完成".into()));
        }
        i.authenticating = true;
        i.client = None;
        i.qr = None;
    }
    let ua = load_ua_config().unwrap_or_default();
    let result = SchoolClient::login(arg.auth, &arg.settings, &ua).await;
    let mut i = state.lock();
    i.authenticating = false;
    match result {
        Ok(c) => {
            i.client = Some(c);
            Ok(Json(()))
        }
        Err(e) => Err(ApiError(e)),
    }
}

pub(crate) async fn logout(State(state): State<AppState>) -> Api<()> {
    let mut i = state.lock();
    if i.cancel.is_some() || i.authenticating {
        return Err(ApiError("请先停止任务或等待登录完成".into()));
    }
    i.client = None;
    i.qr = None;
    Ok(Json(()))
}

pub(crate) async fn login_qr_start(
    State(state): State<AppState>,
    Json(arg): Json<SettingsArg>,
) -> Api<QrImage> {
    if !(2000..=2100).contains(&arg.settings.year) || ![1, 2].contains(&arg.settings.term) {
        return Err(ApiError("请先设置正确的学年学期".into()));
    }
    let ua = load_ua_config().unwrap_or_default();
    let client = SchoolClient::anonymous(&ua).map_err(ApiError)?;
    let execution = client.cas_execution().await?;
    let (csrf_key, csrf_value) = client::generate_csrf();
    let id = client.qr_login_id(&csrf_key, &csrf_value).await?;
    let bytes = client.qr_code(&id).await?;
    let mut i = state.lock();
    if i.cancel.is_some() {
        return Err(ApiError("请先停止任务".into()));
    }
    i.qr = Some(QrSession {
        client,
        id,
        execution,
        csrf_key,
        csrf_value,
        settings: arg.settings,
    });
    Ok(Json(QrImage {
        image: base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

pub(crate) async fn login_qr_poll(State(state): State<AppState>) -> Api<QrPoll> {
    let session = state
        .lock()
        .qr
        .clone()
        .ok_or(ApiError("请先获取登录二维码".into()))?;
    let scan = session
        .client
        .qr_scan(&session.id, &session.csrf_key, &session.csrf_value)
        .await?;
    if scan.code == 200 && !scan.data.trim().is_empty() {
        let mut client = session.client.clone();
        match client
            .qr_login_complete(&session.execution, &scan.data, &session.settings)
            .await
        {
            Ok(()) => {
                let mut i = state.lock();
                i.client = Some(client);
                i.qr = None;
                Ok(Json(QrPoll {
                    status: "confirmed".into(),
                    message: "扫码登录成功".into(),
                }))
            }
            Err(e) => {
                state.lock().qr = None;
                Err(ApiError(e))
            }
        }
    } else if ["过期", "失效", "无效", "不存在"]
        .iter()
        .any(|k| scan.message.contains(k))
    {
        Ok(Json(QrPoll {
            status: "expired".into(),
            message: if scan.message.trim().is_empty() {
                "二维码已过期，请重新获取".into()
            } else {
                scan.message
            },
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

pub(crate) async fn login_qr_cancel(State(state): State<AppState>) -> Api<()> {
    state.lock().qr = None;
    Ok(Json(()))
}
