use super::{Api, ApiError, SettingsArg};
use crate::{
    model::{self, Course},
    state::{AppState, FetchProgress},
    storage::{data_path, save_json},
};
use axum::{extract::State, Json};
use serde::Deserialize;
use std::time::Duration;
const MAX_IMPORT_BYTES: usize = 30 * 1024 * 1024;

#[derive(Deserialize)]
pub(crate) struct ImportArg {
    text: String,
}

pub(crate) async fn courses_get() -> Api<Vec<Course>> {
    let path = data_path("courses.json")?;
    if !path.exists() {
        return Ok(Json(vec![]));
    }
    model::parse_courses(&std::fs::read_to_string(path).map_err(|_| "无法读取课程缓存")?)
        .map(Json)
        .map_err(ApiError)
}

pub(crate) async fn courses_import(Json(arg): Json<ImportArg>) -> Api<Vec<Course>> {
    if arg.text.len() > MAX_IMPORT_BYTES {
        return Err(ApiError("课程文件不能超过 30 MB".into()));
    }
    let courses = model::parse_courses(&arg.text)?;
    save_json("courses.json", &courses)?;
    Ok(Json(courses))
}

pub(crate) async fn courses_fetch(
    State(state): State<AppState>,
    Json(arg): Json<SettingsArg>,
) -> Api<Vec<Course>> {
    let c = state
        .lock()
        .client
        .clone()
        .ok_or(ApiError("请先登录".into()))?;
    state.lock().fetch_progress = Some(FetchProgress {
        page: 0,
        courses: 0,
        total: None,
    });
    let progress_state = state.clone();
    // Per-request timeout is generous for slow networks; this is a hard cap on
    // the whole paginated fetch so it always stops instead of spinning forever.
    let result = tokio::time::timeout(
        Duration::from_secs(1200),
        c.courses(&arg.settings, move |page, courses, total| {
            if let Ok(mut i) = progress_state.inner.lock() {
                i.fetch_progress = Some(FetchProgress {
                    page,
                    courses,
                    total,
                });
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
