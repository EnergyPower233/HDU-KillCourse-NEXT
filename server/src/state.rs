use crate::{
    client::SchoolClient,
    model::{Progress, Settings},
    run_log,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const HISTORY_LIMIT: usize = 1000;

#[derive(Default)]
pub(crate) struct Inner {
    pub(crate) client: Option<SchoolClient>,
    pub(crate) cancel: Option<CancellationToken>,
    pub(crate) authenticating: bool,
    pub(crate) history: Vec<Progress>,
    pub(crate) run_log: Option<run_log::RunLog>,
    pub(crate) log_error: Option<String>,
    /// Pending DingTalk QR login session (id, execution and the shared HTTP
    /// client with its cookie jar).
    pub(crate) qr: Option<QrSession>,
    /// Progress of an in-flight course fetch, surfaced to the UI.
    pub(crate) fetch_progress: Option<FetchProgress>,
}

#[derive(Clone, Serialize)]
pub(crate) struct FetchProgress {
    pub(crate) page: usize,
    pub(crate) courses: usize,
    pub(crate) total: Option<usize>,
}

#[derive(Clone)]
pub(crate) struct QrSession {
    pub(crate) client: SchoolClient,
    pub(crate) id: String,
    pub(crate) execution: String,
    pub(crate) csrf_key: String,
    pub(crate) csrf_value: String,
    pub(crate) settings: Settings,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) inner: Arc<Mutex<Inner>>,
    pub(crate) shutdown: watch::Sender<bool>,
}

impl AppState {
    pub(crate) fn new() -> (Self, watch::Receiver<bool>) {
        let (shutdown, rx) = watch::channel(false);
        (
            Self {
                inner: Arc::new(Mutex::new(Inner::default())),
                shutdown,
            },
            rx,
        )
    }
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }
}

#[derive(Serialize)]
pub(crate) struct Snapshot {
    pub(crate) current_run: Option<run_log::RunInfo>,
    pub(crate) log_error: Option<String>,
    pub(crate) running: bool,
    pub(crate) logged_in: bool,
    pub(crate) history: Vec<Progress>,
    pub(crate) fetch_progress: Option<FetchProgress>,
}

pub(crate) fn publish(state: &AppState, event: Progress) {
    let mut inner = state.lock();
    if inner.log_error.is_none() {
        if let Some(log) = inner.run_log.as_mut() {
            if let Err(error) = log.append(&event) {
                inner.log_error = Some(error);
                if let Some(token) = &inner.cancel {
                    token.cancel();
                }
            }
        }
    }
    inner.history.push(event);
    if inner.history.len() > HISTORY_LIMIT {
        inner.history.remove(0);
    }
}
