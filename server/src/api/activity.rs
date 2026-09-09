use super::{Api, ApiError};
use crate::{
    run_log,
    state::{AppState, Snapshot},
    storage::data_path,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct RunQuery {
    before: Option<usize>,
}

pub(crate) async fn snapshot(State(state): State<AppState>) -> Api<Snapshot> {
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

pub(crate) async fn runs_get() -> Api<Vec<run_log::RunInfo>> {
    run_log::list_runs(&data_path("logs")?)
        .map(Json)
        .map_err(ApiError)
}

pub(crate) async fn run_get(
    Path(id): Path<String>,
    Query(query): Query<RunQuery>,
) -> Api<run_log::RunPage> {
    run_log::read_run(&data_path("logs")?, &id, query.before)
        .map(Json)
        .map_err(ApiError)
}
