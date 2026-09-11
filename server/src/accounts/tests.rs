use super::*;
use crate::{
    api::router,
    client::SchoolClient,
    model::{Course, CourseTask, Mode, Settings, StoredCredentials, TaskList},
};
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::time::Duration;
use tower::ServiceExt;

struct Fixture {
    root: PathBuf,
    accounts: Accounts,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("hdu-accounts-test-{:016x}", rand::random::<u64>()));
        let (shutdown, _) = watch::channel(false);
        Self {
            accounts: Accounts::load(root.clone(), shutdown).unwrap(),
            root,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("hdu-accounts-test-"));
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn call(
    accounts: &Accounts,
    account: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .uri(path)
        .header("x-hdu-account", account);
    let body = match body {
        Some(value) => {
            request = request
                .method("POST")
                .header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = router(accounts.clone())
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn settings(name: &str) -> Settings {
    Settings {
        year: 2026,
        term: 1,
        start_at: String::new(),
        relogin_before_secs: 0,
        lists: vec![TaskList {
            name: "独立任务".into(),
            mode: Mode::Once,
            tasks: vec![CourseTask {
                course: Some(Course {
                    jxbmc: "(2026-2027-1)-shared".into(),
                    jxb_id: "class".into(),
                    kch_id: "course".into(),
                    kklxmc: "主修课程".into(),
                    kcmc: name.into(),
                    ..Course::default()
                }),
                drops: vec![],
            }],
        }],
        ..Settings::default()
    }
}

async fn wait_finished(state: &AppState) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while state.lock().cancel.is_some() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn account_api_isolates_settings_credentials_and_unknown_ids() {
    let f = Fixture::new();
    let profile = f.accounts.create("账号 B", None).unwrap();
    let a = settings("A 的课程");
    let b = settings("B 的课程");
    for (id, settings, username) in [
        (DEFAULT_ACCOUNT, &a, "test-a"),
        (profile.id.as_str(), &b, "test-b"),
    ] {
        assert_eq!(
            call(
                &f.accounts,
                id,
                "/api/settings",
                Some(json!({"settings":settings}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let credentials = StoredCredentials {
            cas_username: username.into(),
            cas_password: "test-only".into(),
            ..StoredCredentials::default()
        };
        assert_eq!(
            call(
                &f.accounts,
                id,
                "/api/credentials",
                Some(json!({"credentials":credentials}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        call(&f.accounts, DEFAULT_ACCOUNT, "/api/credentials", None)
            .await
            .1["cas_username"],
        "test-a"
    );
    assert_eq!(
        call(&f.accounts, &profile.id, "/api/credentials", None)
            .await
            .1["cas_username"],
        "test-b"
    );
    for id in ["missing", "../default", ""] {
        assert_eq!(
            call(&f.accounts, id, "/api/settings", None).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    call(
        &f.accounts,
        &profile.id,
        "/api/credentials/clear",
        Some(json!({})),
    )
    .await;
    assert_eq!(
        call(&f.accounts, DEFAULT_ACCOUNT, "/api/credentials", None)
            .await
            .1["cas_username"],
        "test-a"
    );
    assert!(f.root.join("settings.json").exists());
    let (shutdown, _) = watch::channel(false);
    let reloaded = Accounts::load(f.root.clone(), shutdown).unwrap();
    assert_eq!(
        reloaded
            .get(DEFAULT_ACCOUNT)
            .unwrap()
            .store
            .load_settings_file()
            .unwrap()
            .lists[0]
            .tasks[0]
            .course
            .as_ref()
            .unwrap()
            .kcmc,
        "A 的课程"
    );
    assert_eq!(
        reloaded
            .get(&profile.id)
            .unwrap()
            .store
            .load_settings_file()
            .unwrap()
            .lists[0]
            .tasks[0]
            .course
            .as_ref()
            .unwrap()
            .kcmc,
        "B 的课程"
    );
}

#[test]
fn copying_settings_never_copies_credentials_or_shares_edits() {
    let f = Fixture::new();
    let a = f.accounts.get(DEFAULT_ACCOUNT).unwrap();
    a.store
        .save_json("settings.json", &settings("原课程"))
        .unwrap();
    a.store
        .save_json(
            "credentials.json",
            &StoredCredentials {
                cas_username: "private-a".into(),
                ..StoredCredentials::default()
            },
        )
        .unwrap();
    let profile = f.accounts.create("副本", Some(DEFAULT_ACCOUNT)).unwrap();
    let b = f.accounts.get(&profile.id).unwrap();
    assert_eq!(
        b.store.load_settings_file().unwrap().lists[0].tasks.len(),
        1
    );
    assert!(b
        .store
        .load_stored_credentials()
        .unwrap()
        .cas_username
        .is_empty());
    b.store
        .save_json("settings.json", &settings("修改后的课程"))
        .unwrap();
    assert_eq!(
        a.store.load_settings_file().unwrap().lists[0].tasks[0]
            .course
            .as_ref()
            .unwrap()
            .kcmc,
        "原课程"
    );
    assert!(f.accounts.create("副本", None).is_err());
    assert!(f.accounts.create("坏来源", Some("../default")).is_err());
    f.accounts.rename(&profile.id, "改名").unwrap();
    assert_eq!(f.accounts.list()[1].profile.name, "改名");
}

#[tokio::test]
async fn stopping_one_account_does_not_cancel_another_or_close_the_service() {
    let f = Fixture::new();
    let profile = f.accounts.create("B", None).unwrap();
    let mut config = settings("课程");
    config.start_at = (chrono::Utc::now() + chrono::Duration::minutes(1))
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    for id in [DEFAULT_ACCOUNT, profile.id.as_str()] {
        f.accounts.get(id).unwrap().lock().client =
            Some(SchoolClient::for_test("http://127.0.0.1:9".into()));
        assert_eq!(
            call(
                &f.accounts,
                id,
                "/api/tasks/start",
                Some(json!({"settings":config,"list_index":0}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        call(
            &f.accounts,
            DEFAULT_ACCOUNT,
            "/api/shutdown",
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    call(
        &f.accounts,
        DEFAULT_ACCOUNT,
        "/api/tasks/stop",
        Some(json!({})),
    )
    .await;
    wait_finished(&f.accounts.get(DEFAULT_ACCOUNT).unwrap()).await;
    assert!(f.accounts.get(&profile.id).unwrap().lock().cancel.is_some());
    call(&f.accounts, &profile.id, "/api/tasks/stop", Some(json!({}))).await;
    wait_finished(&f.accounts.get(&profile.id).unwrap()).await;
}

#[tokio::test]
async fn two_accounts_submit_concurrently_with_separate_cookies_and_logs() {
    let f = Fixture::new();
    let profile = f.accounts.create("B", None).unwrap();
    let gate = Arc::new(tokio::sync::Barrier::new(3));
    let entered = gate.clone();
    let cookies = Arc::new(Mutex::new(Vec::new()));
    let observed = cookies.clone();
    let mock = Router::new()
        .route(
            "/xsxk/zzxkyzb_cxZzxkYzbIndex.html",
            get(move || {
                let gate = entered.clone();
                async move {
                    gate.wait().await;
                    let mut html = [
                        "ccdm", "bh_id", "jg_id_1", "xsbj", "xz", "mzm", "xslbdm", "xbm",
                        "zyfx_id", "xqh_id",
                    ]
                    .iter()
                    .map(|key| format!("<input name='{key}' value=''>"))
                    .collect::<String>();
                    html.push_str("<a onclick=\"queryCourse(this,'01','control')\">主修</a>");
                    html
                }
            }),
        )
        .route(
            "/xsxk/zzxkyzbjk_cxJxbWithKchZzxkYzb.html",
            post(|| async { axum::Json(json!([{"jxb_id":"class","do_jxb_id":"operation"}])) }),
        )
        .route(
            "/xsxk/zzxkyzbjk_xkBcZyZzxkYzb.html",
            post(move |headers: HeaderMap| {
                let observed = observed.clone();
                async move {
                    observed
                        .lock()
                        .unwrap()
                        .push(headers["cookie"].to_str().unwrap().to_string());
                    axum::Json(json!({"flag":"1"}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });
    for (id, cookie, name) in [
        (DEFAULT_ACCOUNT, "profile=a", "A 的课程"),
        (profile.id.as_str(), "profile=b", "B 的课程"),
    ] {
        f.accounts.get(id).unwrap().lock().client =
            Some(SchoolClient::for_test_account(base.clone(), cookie));
        assert_eq!(
            call(
                &f.accounts,
                id,
                "/api/tasks/start",
                Some(json!({"settings":settings(name),"list_index":0}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    tokio::time::timeout(Duration::from_secs(3), gate.wait())
        .await
        .expect("both accounts must reach school requests concurrently");
    for (id, name) in [
        (DEFAULT_ACCOUNT, "A 的课程"),
        (profile.id.as_str(), "B 的课程"),
    ] {
        wait_finished(&f.accounts.get(id).unwrap()).await;
        let snapshot = call(&f.accounts, id, "/api/snapshot", None).await.1;
        assert!(snapshot["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["status"] == "success" && e["course_name"] == name));
        let run = snapshot["current_run"]["id"].as_str().unwrap();
        let log = call(&f.accounts, id, &format!("/api/runs/{run}"), None)
            .await
            .1;
        assert!(log["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["course_name"] == "" || e["course_name"] == name));
    }
    let mut seen = cookies.lock().unwrap().clone();
    seen.sort();
    assert_eq!(seen, vec!["profile=a", "profile=b"]);
    server.abort();
}
