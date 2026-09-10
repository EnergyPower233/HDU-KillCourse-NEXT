use super::*;
use axum::{
    extract::OriginalUri,
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

type Calls = Arc<Mutex<Vec<(String, String)>>>;

struct MockSchool {
    client: SchoolClient,
    calls: Calls,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for MockSchool {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl MockSchool {
    async fn start(
        reply: (StatusCode, &'static str),
        defer_good: bool,
        stop: Option<CancellationToken>,
    ) -> Self {
        let calls: Calls = Arc::default();
        let observed = calls.clone();
        let counts = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
        let app = Router::new()
            .route(
                "/xsxk/zzxkyzb_cxZzxkYzbIndex.html",
                get(|| async {
                    let fields = [
                        "ccdm", "bh_id", "jg_id_1", "xsbj", "xz", "mzm", "xslbdm", "xbm",
                        "zyfx_id", "xqh_id",
                    ];
                    let mut html = fields
                        .iter()
                        .map(|key| format!("<input name='{key}' value=''>"))
                        .collect::<String>();
                    html.push_str("<a onclick=\"queryCourse(this,'01','test-control')\">主修</a>");
                    html
                }),
            )
            .fallback(post(
                move |OriginalUri(uri): OriginalUri,
                      axum::Form(body): axum::Form<HashMap<String, String>>| {
                    let (observed, counts, stop) = (observed.clone(), counts.clone(), stop.clone());
                    async move {
                        let path = uri.path();
                        if path.ends_with("zzxkyzb_cxZzxkYzbPartDisplay.html") {
                            let id = &body["filter_list[0]"];
                            observed.lock().unwrap().push(("query".into(), id.clone()));
                            let mut counts = counts.lock().unwrap();
                            let n = counts.entry(id.clone()).or_default();
                            *n += 1;
                            let rows = if defer_good && id == "good" && *n == 1 {
                                vec![]
                            } else {
                                vec![serde_json::json!({"jxbmc": id})]
                            };
                            return (
                                StatusCode::OK,
                                serde_json::json!({"tmpList": rows}).to_string(),
                            );
                        }
                        let id = &body["kch_id"];
                        if path.ends_with("zzxkyzbjk_cxJxbWithKchZzxkYzb.html") {
                            observed
                                .lock()
                                .unwrap()
                                .push(("prepare".into(), id.clone()));
                            return (
                                StatusCode::OK,
                                serde_json::json!([{"jxb_id": id, "do_jxb_id": id}]).to_string(),
                            );
                        }
                        let action = if path.ends_with("zzxkyzbjk_xkBcZyZzxkYzb.html") {
                            "select"
                        } else if path.ends_with("zzxkyzb_tuikBcZzxkYzb.html") {
                            "cancel"
                        } else {
                            panic!("unexpected request: {path}");
                        };
                        assert_eq!(body["jxb_ids"], *id);
                        observed.lock().unwrap().push((action.into(), id.clone()));
                        if id.contains("bad") {
                            if let Some(stop) = stop {
                                stop.cancel();
                            }
                            (reply.0, reply.1.to_string())
                        } else {
                            (
                                StatusCode::OK,
                                if action == "select" {
                                    r#"{"flag":"1"}"#
                                } else {
                                    r#""1""#
                                }
                                .into(),
                            )
                        }
                    }
                },
            ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = SchoolClient::for_test(format!("http://{}", listener.local_addr().unwrap()));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            client,
            calls,
            server,
        }
    }

    fn submissions(&self) -> Vec<(String, String)> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(action, _)| action == "select" || action == "cancel")
            .cloned()
            .collect()
    }

    async fn run(
        &self,
        tasks: Vec<model::CourseTask>,
        mode: Mode,
        token: &CancellationToken,
    ) -> AppState {
        let (state, _) = AppState::new();
        state.lock().cancel = Some(token.clone());
        let settings = Settings {
            interval_ms: 100,
            jitter_ms: 0,
            relogin_before_secs: 0,
            ..Settings::default()
        };
        let list = TaskList {
            name: "mock".into(),
            mode,
            tasks,
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            run_tasks(
                &state,
                self.client.clone(),
                &model::StoredCredentials::default(),
                &model::UaConfig::default(),
                &list,
                &settings,
                token,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        state
    }
}

fn course(id: &str) -> Course {
    Course {
        jxbmc: id.into(),
        jxb_id: id.into(),
        kch_id: id.into(),
        kklxmc: "主修课程".into(),
        kcmc: format!("测试课程 {id}"),
        ..Course::default()
    }
}

fn select(id: &str) -> model::CourseTask {
    model::CourseTask {
        course: Some(course(id)),
        drops: vec![],
    }
}

fn assert_result(state: &AppState, id: &str, status: &str, action: Action) {
    let state = state.lock();
    let event = state
        .history
        .iter()
        .rev()
        .find(|e| e.course_id == id)
        .unwrap();
    assert_eq!(event.status, status);
    assert_eq!(
        std::mem::discriminant(event.action.as_ref().unwrap()),
        std::mem::discriminant(&action)
    );
    assert_eq!(event.course_name, format!("测试课程 {id}"));
    if status == "failed" {
        assert!(event.message.contains("本次按失败处理"));
    }
}

#[tokio::test]
async fn once_unconfirmed_select_skips_only_that_course() {
    for reply in [
        (StatusCode::OK, r#"{"flag":"-1","msg":"人数已满"}"#),
        (StatusCode::OK, "{}"),
        (StatusCode::OK, "<html>登录</html>"),
        (StatusCode::OK, ""),
        (StatusCode::INTERNAL_SERVER_ERROR, "unavailable"),
    ] {
        let school = MockSchool::start(reply, false, None).await;
        let token = CancellationToken::new();
        let state = school
            .run(vec![select("bad"), select("good")], Mode::Once, &token)
            .await;
        assert_eq!(
            school.submissions(),
            vec![
                ("select".into(), "bad".into()),
                ("select".into(), "good".into())
            ]
        );
        assert_result(&state, "bad", "failed", Action::Select);
        assert_result(&state, "good", "success", Action::Select);
        assert!(!token.is_cancelled());
    }
}

#[tokio::test]
async fn watch_consumes_failed_course_and_keeps_querying_others() {
    let school = MockSchool::start((StatusCode::OK, "{}"), true, None).await;
    let state = school
        .run(
            vec![select("bad"), select("good")],
            Mode::Watch,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(
        school.submissions(),
        vec![
            ("select".into(), "bad".into()),
            ("select".into(), "good".into())
        ]
    );
    let calls = school.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|(op, id)| op == "query" && id == "bad")
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|(op, id)| op == "query" && id == "good")
            .count(),
        2
    );
    assert_result(&state, "bad", "failed", Action::Select);
    assert_result(&state, "good", "success", Action::Select);
}

#[tokio::test]
async fn unconfirmed_drop_skips_dependents_but_continues_next_task() {
    let school = MockSchool::start((StatusCode::OK, r#""0""#), false, None).await;
    let mut paired = select("paired");
    paired.drops = vec![
        course("drop-good"),
        course("drop-bad"),
        course("drop-later"),
    ];
    let state = school
        .run(
            vec![paired, select("good")],
            Mode::Once,
            &CancellationToken::new(),
        )
        .await;
    assert_eq!(
        school.submissions(),
        vec![
            ("cancel".into(), "drop-good".into()),
            ("cancel".into(), "drop-bad".into()),
            ("select".into(), "good".into())
        ]
    );
    assert_result(&state, "drop-good", "success", Action::Cancel);
    assert_result(&state, "drop-bad", "failed", Action::Cancel);
    assert_result(&state, "paired", "error", Action::Select);
    assert_result(&state, "good", "success", Action::Select);
    assert!(!school
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|(_, id)| id == "paired" || id == "drop-later"));
}

#[tokio::test]
async fn stop_during_unconfirmed_submission_still_records_result_and_stops() {
    let token = CancellationToken::new();
    let school = MockSchool::start((StatusCode::OK, "{}"), false, Some(token.clone())).await;
    let state = school
        .run(vec![select("bad"), select("good")], Mode::Once, &token)
        .await;
    assert_eq!(school.submissions(), vec![("select".into(), "bad".into())]);
    assert_result(&state, "bad", "failed", Action::Select);
    assert!(token.is_cancelled());
}
