use super::{Api, ApiError};
use crate::{
    model::{Progress, Settings, TaskList},
    run_log,
    scheduler::run_tasks,
    state::{publish, AppState},
    storage::{data_path, load_stored_credentials, load_ua_config},
    task_import,
};
use axum::{extract::State, Json};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub(crate) struct TaskImportArg {
    text: String,
    settings: Settings,
}

#[derive(Deserialize)]
pub(crate) struct StartArg {
    settings: Settings,
    list_index: usize,
}

pub(crate) async fn tasks_import(Json(arg): Json<TaskImportArg>) -> Api<Vec<TaskList>> {
    task_import::parse_task_file(&arg.text, &arg.settings)
        .map(Json)
        .map_err(ApiError)
}

pub(crate) async fn tasks_start(
    State(state): State<AppState>,
    Json(arg): Json<StartArg>,
) -> Api<()> {
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
        client = i
            .client
            .clone()
            .ok_or(ApiError("请先登录学校系统".into()))?;
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

pub(crate) async fn tasks_stop(State(state): State<AppState>) -> Api<()> {
    if let Some(cancel) = &state.lock().cancel {
        cancel.cancel();
    }
    Ok(Json(()))
}
