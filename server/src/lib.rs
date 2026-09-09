//! HDU-KillCourse NEXT local server: REST API + embedded frontend.
//!
//! The HTTP layer replaces the previous Tauri/WebView shell. All school
//! protocol code (login, course fetch, select/cancel) lives in `client.rs`
//! and is unchanged; the task scheduler below keeps the same safety rules:
//! bounded parallel queries, serialized submissions, stop-on-unknown,
//! credentials only in memory.
mod client;
mod model;
mod run_log;

use axum::{
    extract::{State, Path, Query},
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use client::{Outcome, SchoolClient};
use model::{Action, Course, Credentials, Mode, Progress, Settings, TaskList};
use base64::Engine;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const MAX_IMPORT_BYTES: usize = 30 * 1024 * 1024;
const HISTORY_LIMIT: usize = 1000;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Inner {
    client: Option<SchoolClient>,
    cancel: Option<CancellationToken>,
    authenticating: bool,
    history: Vec<Progress>,
    run_log: Option<run_log::RunLog>,
    log_error: Option<String>,
    /// Pending DingTalk QR login session (id, execution and the shared HTTP
    /// client with its cookie jar).
    qr: Option<QrSession>,
    /// Progress of an in-flight course fetch, surfaced to the UI.
    fetch_progress: Option<FetchProgress>,
}

#[derive(Clone, Serialize)]
struct FetchProgress {
    page: usize,
    courses: usize,
    total: Option<usize>,
    finished: bool,
}

#[derive(Clone)]
struct QrSession {
    client: SchoolClient,
    id: String,
    execution: String,
    csrf_key: String,
    csrf_value: String,
    settings: Settings,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<Inner>>,
    shutdown: watch::Sender<bool>,
}

impl AppState {
    fn new() -> (Self, watch::Receiver<bool>) {
        let (shutdown, rx) = watch::channel(false);
        (Self { inner: Arc::new(Mutex::new(Inner::default())), shutdown }, rx)
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }
}

#[derive(Serialize)]
struct Snapshot {
    current_run: Option<run_log::RunInfo>,
    log_error: Option<String>,
    running: bool,
    logged_in: bool,
    history: Vec<Progress>,
    fetch_progress: Option<FetchProgress>,
}

fn publish(state: &AppState, event: Progress) {
    let mut inner = state.lock();
    if inner.log_error.is_none() {
        if let Some(log) = inner.run_log.as_mut() {
            if let Err(error) = log.append(&event) {
                inner.log_error = Some(error);
                if let Some(token) = &inner.cancel { token.cancel(); }
            }
        }
    }
    inner.history.push(event);
    if inner.history.len() > HISTORY_LIMIT { inner.history.remove(0); }
}

// ---------------------------------------------------------------------------
// Data files (settings / course cache / credentials / UA).
// Portable layout: everything lives in a `data` folder next to the
// executable, so the whole folder can be copied between machines. The
// HDU_DATA_DIR environment variable overrides the location.
// ---------------------------------------------------------------------------

fn data_path(file: &str) -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("HDU_DATA_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|_| "无法创建数据目录")?;
        return Ok(dir.join(file));
    }
    let exe = std::env::current_exe().map_err(|_| "无法确定程序位置")?;
    let base = exe.parent().ok_or("无法确定程序目录")?;
    let dir = base.join("data");
    std::fs::create_dir_all(&dir)
        .map_err(|_| "无法创建数据目录（程序目录下的 data 文件夹），请把程序放到可写的文件夹中运行")?;
    Ok(dir.join(file))
}

/// One-time migration: copy data from the old per-user app data directory
/// into the portable `data` folder when the portable copy does not exist yet.
fn migrate_portable_data() {
    let Ok(portable_file) = data_path("settings.json") else {
        return;
    };
    let Some(portable) = portable_file.parent().map(|p| p.to_path_buf()) else {
        return;
    };
    let Some(legacy) = directories::ProjectDirs::from("cn", "hducourse", "studio")
        .map(|p| p.data_dir().to_path_buf())
    else {
        return;
    };
    if !legacy.exists() {
        return;
    }
    for file in ["settings.json", "courses.json", "credentials.json", "ua.json"] {
        let src = legacy.join(file);
        let dst = portable.join(file);
        if !dst.exists() && src.exists() {
            if let Ok(bytes) = std::fs::read(&src) {
                let _ = std::fs::write(&dst, bytes);
            }
        }
    }
}

fn save_json<T: Serialize>(file: &str, value: &T) -> Result<(), String> {
    let path = data_path(file)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "无法编码配置")?;
    std::fs::write(path, bytes).map_err(|_| "无法保存本地数据".into())
}

// ---------------------------------------------------------------------------
// Error helper: all user-facing errors become `{"error": "..."}` with 400
// ---------------------------------------------------------------------------

struct ApiError(String);

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

type Api<T> = Result<Json<T>, ApiError>;

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SettingsArg {
    settings: Settings,
}

#[derive(Deserialize)]
struct LoginArg {
    auth: Credentials,
    settings: Settings,
}

#[derive(Deserialize)]
struct ImportArg {
    text: String,
}

async fn health() -> Api<serde_json::Value> {
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn snapshot(State(state): State<AppState>) -> Api<Snapshot> {
    let i = state.lock();
    Ok(Json(Snapshot {
        current_run: i.run_log.as_ref().map(|log| log.info.clone()),
        log_error: i.log_error.clone(),
        running: i.cancel.is_some(),
        logged_in: i.client.is_some(),
        history: i.history.clone(),
        fetch_progress: i.fetch_progress.clone(),
    }))
}

fn load_settings_file() -> Result<Settings, String> {
    let path = data_path("settings.json")?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取配置")?)
        .map_err(|_| "保存的配置已损坏".into())
}

async fn runs_get() -> Api<Vec<run_log::RunInfo>> {
    run_log::list_runs(&data_path("logs")?).map(Json).map_err(ApiError)
}

#[derive(Deserialize)]
struct RunQuery { before: Option<usize> }

async fn run_get(Path(id): Path<String>, Query(query): Query<RunQuery>) -> Api<run_log::RunPage> {
    run_log::read_run(&data_path("logs")?, &id, query.before).map(Json).map_err(ApiError)
}

async fn settings_get() -> Api<Settings> {
    load_settings_file().map(Json).map_err(ApiError)
}

async fn settings_save(Json(arg): Json<SettingsArg>) -> Api<()> {
    arg.settings.validate(false, arg.settings.active_list)?;
    save_json("settings.json", &arg.settings)?;
    Ok(Json(()))
}

async fn courses_get() -> Api<Vec<Course>> {
    let path = data_path("courses.json")?;
    if !path.exists() {
        return Ok(Json(vec![]));
    }
    model::parse_courses(&std::fs::read_to_string(path).map_err(|_| "无法读取课程缓存")?)
        .map(Json)
        .map_err(ApiError)
}

async fn courses_import(Json(arg): Json<ImportArg>) -> Api<Vec<Course>> {
    if arg.text.len() > MAX_IMPORT_BYTES {
        return Err(ApiError("课程文件不能超过 30 MB".into()));
    }
    let courses = model::parse_courses(&arg.text)?;
    save_json("courses.json", &courses)?;
    Ok(Json(courses))
}

// ---------------------------------------------------------------------------
// Stored login credentials (user-requested persistence, like the original
// Go config.json; the file lives in the local data directory and is never
// logged).
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CredentialsArg {
    credentials: model::StoredCredentials,
}

fn load_stored_credentials() -> Result<model::StoredCredentials, String> {
    let path = data_path("credentials.json")?;
    if !path.exists() {
        return Ok(model::StoredCredentials::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取登录凭证")?)
        .map_err(|_| "保存的登录凭证已损坏，请在偏好设置中清除后重新填写".into())
}

async fn credentials_get() -> Api<model::StoredCredentials> {
    load_stored_credentials().map(Json).map_err(ApiError)
}

async fn credentials_save(Json(arg): Json<CredentialsArg>) -> Api<()> {
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
    save_json("credentials.json", &c)?;
    // Restrict the file to the current user where the platform allows it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(path) = data_path("credentials.json") {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
    Ok(Json(()))
}

async fn credentials_clear() -> Api<()> {
    let path = data_path("credentials.json")?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|_| ApiError("无法清除凭证文件".into()))?;
    }
    Ok(Json(()))
}

// ---------------------------------------------------------------------------
// User-Agent configuration
// ---------------------------------------------------------------------------

fn load_ua_config() -> Result<model::UaConfig, String> {
    let path = data_path("ua.json")?;
    if !path.exists() {
        return Ok(model::UaConfig::default());
    }
    let mut config: model::UaConfig =
        serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取 User-Agent 配置")?)
            .map_err(|_| "User-Agent 配置已损坏，请在设置中重置".to_string())?;
    config.normalize();
    Ok(config)
}

#[derive(Deserialize)]
struct UaArg {
    ua: model::UaConfig,
}

async fn ua_get() -> Api<model::UaConfig> {
    load_ua_config().map(Json).map_err(ApiError)
}

async fn ua_save(Json(arg): Json<UaArg>) -> Api<()> {
    let mut ua = arg.ua;
    ua.normalize();
    if ua.browser_ua.len() > 512 || ua.fixed_version.len() > 128 {
        return Err(ApiError("User-Agent 内容过长".into()));
    }
    if ua.rotate_list.len() > 50 || ua.rotate_list.iter().any(|s| s.len() > 512) {
        return Err(ApiError("轮换列表最多 50 条，每条不超过 512 字符".into()));
    }
    save_json("ua.json", &ua)?;
    Ok(Json(()))
}

async fn login(State(state): State<AppState>, Json(arg): Json<LoginArg>) -> Api<()> {
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

async fn logout(State(state): State<AppState>) -> Api<()> {
    let mut i = state.lock();
    if i.cancel.is_some() || i.authenticating {
        return Err(ApiError("请先停止任务或等待登录完成".into()));
    }
    i.client = None;
    i.qr = None;
    Ok(Json(()))
}

// ---------------------------------------------------------------------------
// DingTalk QR login endpoints (protocol from the original Go CasQrLogin)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct QrImage {
    /// Base64 PNG of the QR code.
    image: String,
}

#[derive(Serialize)]
struct QrPoll {
    status: String,
    message: String,
}

async fn login_qr_start(State(state): State<AppState>, Json(arg): Json<SettingsArg>) -> Api<QrImage> {
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

async fn login_qr_poll(State(state): State<AppState>) -> Api<QrPoll> {
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

async fn login_qr_cancel(State(state): State<AppState>) -> Api<()> {
    state.lock().qr = None;
    Ok(Json(()))
}

async fn courses_fetch(State(state): State<AppState>, Json(arg): Json<SettingsArg>) -> Api<Vec<Course>> {
    let c = state.lock().client.clone().ok_or(ApiError("请先登录".into()))?;
    state.lock().fetch_progress = Some(FetchProgress { page: 0, courses: 0, total: None, finished: false });
    let progress_state = state.clone();
    // Per-request timeout is generous for slow networks; this is a hard cap on
    // the whole paginated fetch so it always stops instead of spinning forever.
    let result = tokio::time::timeout(
        Duration::from_secs(1200),
        c.courses(&arg.settings, move |page, courses, total| {
            if let Ok(mut i) = progress_state.inner.lock() {
                i.fetch_progress = Some(FetchProgress { page, courses, total, finished: false });
            }
        }),
    )
    .await;
    let courses = match result {
        Ok(Ok(courses)) => courses,
        Ok(Err(e)) => {
            state.lock().fetch_progress = None;
            return Err(ApiError(e));
        }
        Err(_) => {
            state.lock().fetch_progress = None;
            return Err(ApiError(
                "获取课程整体超时（超过 20 分钟），请检查网络后重试".into(),
            ));
        }
    };
    state.lock().fetch_progress = None;
    save_json("courses.json", &courses)?;
    Ok(Json(courses))
}

async fn tasks_stop(State(state): State<AppState>) -> Api<()> {
    if let Some(cancel) = &state.lock().cancel {
        cancel.cancel();
    }
    Ok(Json(()))
}

#[derive(Deserialize)]
struct StartArg {
    settings: Settings,
    list_index: usize,
}

async fn tasks_start(State(state): State<AppState>, Json(arg): Json<StartArg>) -> Api<()> {
    arg.settings.validate(true, arg.list_index)?;
    let token = CancellationToken::new();
    let client;
    let stored = load_stored_credentials().unwrap_or_default();
    let ua = load_ua_config().unwrap_or_default();
    let list = arg.settings.list(arg.list_index).unwrap().clone();
    {
        let mut i = state.lock();
        if i.cancel.is_some() {
            return Err(ApiError("已有任务正在运行".into()));
        }
        client = i.client.clone().ok_or(ApiError("请先登录学校系统".into()))?;
        let log = run_log::RunLog::create(&data_path("logs")?, &list.name)?;
        i.run_log = Some(log);
        i.log_error = None;
        i.cancel = Some(token.clone());
        i.history.clear();
    }
    let shared = state.clone();
    tokio::spawn(async move {
        let result = run_tasks(&shared, client, &stored, &ua, &list, &arg.settings, &token).await;
        match result {
            Ok(()) => publish(
                &shared,
                Progress::new(
                    "",
                    "finished",
                    if token.is_cancelled() {
                        "任务已停止；已发送的请求不会被撤回"
                    } else {
                        "本轮任务已结束，请查看各课程结果"
                    },
                ),
            ),
            Err(e) => publish(&shared, Progress::new("", "error", &e)),
        }
        shared.lock().cancel = None;
    });
    Ok(Json(()))
}

async fn shutdown(State(state): State<AppState>) -> Api<()> {
    let _ = state.shutdown.send(true);
    Ok(Json(()))
}

// ---------------------------------------------------------------------------
// Task scheduler (ported from the Tauri version; same safety rules)
// ---------------------------------------------------------------------------

async fn wait_or_stop(token: &CancellationToken, millis: u64) -> bool {
    tokio::select! { biased; _ = token.cancelled() => false, _ = tokio::time::sleep(Duration::from_millis(millis)) => true }
}

fn method_label(method: model::LoginMethod) -> &'static str {
    match method {
        model::LoginMethod::Cas => "统一身份认证",
        model::LoginMethod::Newjw => "教务账号",
        model::LoginMethod::Qrcode => "钉钉扫码",
        model::LoginMethod::Cookie => "已有 Cookie",
    }
}

async fn run_tasks(
    state: &AppState,
    mut client: SchoolClient,
    stored: &model::StoredCredentials,
    ua: &model::UaConfig,
    list: &TaskList,
    settings: &Settings,
    token: &CancellationToken,
) -> Result<(), String> {
    publish(
        state,
        Progress::new("", "running", &format!("任务启动：清单「{}」", list.name)),
    );
    let delay = settings.delay_ms()?;
    if delay > 0 {
        for t in &list.tasks {
            if let Some(course) = &t.course {
                publish(
                    state,
                    Progress::new(&course.jxbmc, "waiting", "等待计划时间（UTC+8）"),
                );
            }
            for drop in &t.drops {
                publish(
                    state,
                    Progress::new(&drop.jxbmc, "waiting", "等待计划时间（UTC+8）"),
                );
            }
        }
        // Optional freshness re-login: N seconds before the scheduled start,
        // run the stored login methods in the user's preferred order so the
        // session is fresh when the task fires.
        let pre = if settings.relogin_before_secs > 0 {
            (settings.relogin_before_secs * 1000).min(delay)
        } else {
            0
        };
        if pre > 0 {
            let first = delay - pre;
            if first > 0 && !wait_or_stop(token, first).await {
                return Ok(());
            }
            publish(
                state,
                Progress::new(
                    "",
                    "running",
                    &format!(
                        "距离开始还有 {} 秒，按你的顺序重新登录教务系统",
                        settings.relogin_before_secs
                    ),
                ),
            );
            let attempt = tokio::select! {
                biased;
                _ = token.cancelled() => None,
                r = client::SchoolClient::login_with_order(stored, settings, ua) => Some(r),
            };
            match attempt {
                Some(Ok((fresh, method))) => {
                    client = fresh;
                    publish(
                        state,
                        Progress::new(
                            "",
                            "success",
                            &format!("提前重新登录成功（{}），任务将使用新会话", method_label(method)),
                        ),
                    );
                }
                Some(Err(e)) => publish(
                    state,
                    Progress::new(
                        "",
                        "error",
                        &format!("提前重新登录失败：{e}，将沿用现有会话"),
                    ),
                ),
                None => return Ok(()),
            }
            if !wait_or_stop(token, pre).await {
                return Ok(());
            }
        } else if !wait_or_stop(token, delay).await {
            return Ok(());
        }
    }
    let config = tokio::select! { biased; _ = token.cancelled() => return Ok(()), result = client.body_config() => result? };
    let mut pending = list.tasks.clone();
    loop {
        if token.is_cancelled() {
            return Ok(());
        }
        if matches!(list.mode, Mode::Watch) {
            let mut ready: Vec<Course> = Vec::new();
            // Bounded parallel reads. Mutations below are deliberately serialized.
            for batch in pending.chunks(4) {
                let mut queries = tokio::task::JoinSet::new();
                for task in batch {
                    // Watch mode is validated to contain pure select tasks only.
                    let course = task.course.as_ref().expect("watch tasks always select a course");
                    publish(
                        state,
                        Progress::new(&course.jxbmc, "querying", "正在查询余量"),
                    );
                    let (c, s, t) = (client.clone(), settings.clone(), course.clone());
                    queries.spawn(async move {
                        let result = c.available(&s, &t).await;
                        (t, result)
                    });
                }
                while !queries.is_empty() {
                    let row = tokio::select! { biased; _ = token.cancelled() => return Ok(()), row = queries.join_next() => row };
                    let (course, result) = row.unwrap().map_err(|_| "查询任务异常结束")?;
                    match result {
                        Ok(true) => ready.push(course),
                        Ok(false) => publish(
                            state,
                            Progress::new(
                                &course.jxbmc,
                                "waiting",
                                "暂无余量，等待下一次查询",
                            ),
                        ),
                        Err(e) => {
                            publish(state, Progress::new(&course.jxbmc, "error", &e));
                            return Err("查询失败，已停止本轮；请检查网络或重新登录".into());
                        }
                    }
                }
            }
            for course in ready {
                if token.is_cancelled() {
                    return Ok(());
                }
                submit_select(state, &client, settings, &config, &course, token).await?;
                pending.retain(|t| t.course.as_ref().map(|c| c.jxbmc != course.jxbmc).unwrap_or(true));
            }
        } else {
            // Once mode: process tasks strictly in list order, dropping first
            // and selecting afterwards. A consumed task is removed as we go.
            while !pending.is_empty() {
                let task = pending.remove(0);
                if token.is_cancelled() {
                    return Ok(());
                }
                run_once_task(state, &client, settings, &config, &task, token).await?;
            }
        }
        if pending.is_empty() || matches!(list.mode, Mode::Once) {
            return Ok(());
        }
        let wait = client::jittered_wait(settings.interval_ms, settings.jitter_ms, settings.jitter_seed);
        if !wait_or_stop(token, wait).await {
            return Ok(());
        }
    }
}

/// Submit one select request. Err means "stop the whole round" (result
/// unknown); a rejected select is logged and the task is consumed.
async fn submit_select(
    state: &AppState,
    client: &SchoolClient,
    settings: &Settings,
    config: &std::collections::HashMap<String, String>,
    course: &Course,
    token: &CancellationToken,
) -> Result<(), String> {
    let body = tokio::select! { biased; _ = token.cancelled() => return Ok(()), result = client.prepare(settings, course, config) => result };
    let body = match body {
        Ok(body) => body,
        Err(e) => {
            publish(state, Progress::new(&course.jxbmc, "error", &e));
            return Ok(());
        }
    };
    if token.is_cancelled() {
        return Ok(());
    }
    publish(
        state,
        Progress::new(&course.jxbmc, "submitting", "正在提交，请等待学校回复"),
    );
    // Do not drop a mutation future on cancellation: retain its result, then stop.
    let outcome = client.submit(&Action::Select, &body).await;
    match outcome {
        Outcome::Success => {
            publish(
                state,
                Progress::new(&course.jxbmc, "success", "学校已返回成功，请以教务系统已选记录为准"),
            );
            Ok(())
        }
        Outcome::Rejected(message) => {
            publish(state, Progress::new(&course.jxbmc, "rejected", &message));
            Ok(())
        }
        Outcome::Unknown => {
            publish(
                state,
                Progress::new(&course.jxbmc, "unknown", "提交结果不明，已暂停全部任务。请先在教务系统核实，勿直接重复提交"),
            );
            Err("出现结果不明的提交，请核实真实课表后再操作".into())
        }
    }
}

/// Submit one drop request. Ok(true) = dropped, Ok(false) = failed (skip the
/// paired select), Err = stop the whole round (result unknown).
async fn submit_drop(
    state: &AppState,
    client: &SchoolClient,
    settings: &Settings,
    config: &std::collections::HashMap<String, String>,
    drop: &Course,
    token: &CancellationToken,
) -> Result<bool, String> {
    let body = tokio::select! { biased; _ = token.cancelled() => return Ok(false), result = client.prepare(settings, drop, config) => result };
    let body = match body {
        Ok(body) => body,
        Err(e) => {
            publish(
                state,
                Progress::new(&drop.jxbmc, "error", &format!("退课准备失败：{e}")),
            );
            return Ok(false);
        }
    };
    if token.is_cancelled() {
        return Ok(false);
    }
    publish(
        state,
        Progress::new(&drop.jxbmc, "submitting", "正在提交退课，请等待学校回复"),
    );
    let outcome = client.submit(&Action::Cancel, &body).await;
    match outcome {
        Outcome::Success => {
            publish(
                state,
                Progress::new(&drop.jxbmc, "success", "退课：学校已返回成功，请以教务系统已选记录为准"),
            );
            Ok(true)
        }
        Outcome::Rejected(message) => {
            publish(
                state,
                Progress::new(&drop.jxbmc, "rejected", &format!("退课被拒绝：{message}")),
            );
            Ok(false)
        }
        Outcome::Unknown => {
            publish(
                state,
                Progress::new(&drop.jxbmc, "unknown", "退课结果不明，已暂停全部任务。请先在教务系统核实"),
            );
            Err("出现结果不明的退课，请核实真实课表后再操作".into())
        }
    }
}

/// One once-mode task: drop everything first, then select. Err = stop round.
async fn run_once_task(
    state: &AppState,
    client: &SchoolClient,
    settings: &Settings,
    config: &std::collections::HashMap<String, String>,
    task: &model::CourseTask,
    token: &CancellationToken,
) -> Result<(), String> {
    for drop in &task.drops {
        if token.is_cancelled() {
            return Ok(());
        }
        let dropped = submit_drop(state, client, settings, config, drop, token).await?;
        if !dropped {
            if let Some(course) = &task.course {
                publish(
                    state,
                    Progress::new(&course.jxbmc, "error", "前置退课未全部成功，跳过本次选课"),
                );
            }
            return Ok(());
        }
    }
    if let Some(course) = &task.course {
        if token.is_cancelled() {
            return Ok(());
        }
        submit_select(state, client, settings, config, course, token).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Embedded frontend
// ---------------------------------------------------------------------------

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

async fn static_fallback(uri: Uri) -> Response {
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

fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/snapshot", get(snapshot))
        .route("/api/runs", get(runs_get))
        .route("/api/runs/{id}", get(run_get))
        .route("/api/settings", get(settings_get).post(settings_save))
        .route("/api/courses", get(courses_get))
        .route("/api/courses/import", post(courses_import))
        .route("/api/courses/fetch", post(courses_fetch))
        .route("/api/credentials", get(credentials_get).post(credentials_save))
        .route("/api/credentials/clear", post(credentials_clear))
        .route("/api/ua", get(ua_get).post(ua_save))
        .route("/api/login", post(login))
        .route("/api/login/qr/start", post(login_qr_start))
        .route("/api/login/qr/poll", post(login_qr_poll))
        .route("/api/login/qr/cancel", post(login_qr_cancel))
        .route("/api/logout", post(logout))
        .route("/api/tasks/start", post(tasks_start))
        .route("/api/tasks/stop", post(tasks_stop))
        .route("/api/shutdown", post(shutdown))
        .fallback(static_fallback)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Entry: bind, print, open browser, serve until Ctrl+C or /api/shutdown
// ---------------------------------------------------------------------------

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
/// credentials, so the session is ready when the browser opens. QR login is
/// skipped (it needs interactive scanning). Failure is logged to the console
/// and the user can still log in manually.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let (state, _rx) = AppState::new();
        router(state)
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
