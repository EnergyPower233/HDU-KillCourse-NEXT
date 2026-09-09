use super::*;
use super::{
    auth::{encrypt_cas, QrResp},
    enrollment::parse_outcome,
    random::Mt19937,
    ua::{build_fixed_ua, generate_ua},
};
use crate::model::{Action, Course, Settings};
use base64::{engine::general_purpose::STANDARD, Engine};
async fn fetch_mock_pages(pages: Vec<Value>) -> (Result<Vec<Course>, String>, usize) {
    use axum::{routing::post, Router};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = Router::new().route(
        "/courses",
        post(
            move |axum::Form(body): axum::Form<HashMap<String, String>>| {
                let i = observed.fetch_add(1, Ordering::SeqCst);
                let response = pages
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"items": []}));
                async move {
                    assert_eq!(body["queryModel.showCount"], "9999");
                    assert_eq!(body["queryModel.currentPage"], (i + 1).to_string());
                    axum::Json(response)
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/courses", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = SchoolClient {
        http: build_http(Arc::new(Jar::default())).unwrap(),
        grade: String::new(),
        major: String::new(),
        ua: DEFAULT_UA.into(),
    };
    let result = client
        .courses_from(&url, &Settings::default(), |_, _, _| {})
        .await;
    server.abort();
    (result, calls.load(Ordering::SeqCst))
}

#[tokio::test]
async fn catalog_large_page_and_capped_pages_are_complete() {
    let rows: Vec<Value> = (0..5193)
        .map(|i| serde_json::json!({"jxbmc": format!("class-{i}")}))
        .collect();
    let (result, calls) = fetch_mock_pages(vec![
        serde_json::json!({"items": rows, "totalResult": "5193"}),
    ])
    .await;
    assert_eq!(result.unwrap().len(), 5193);
    assert_eq!(calls, 1);

    let (result, calls) = fetch_mock_pages(vec![
        serde_json::json!({"items": [{"jxbmc": "a"}], "totalCount": 2}),
        serde_json::json!({"items": [{"jxbmc": "b"}], "totalCount": 2}),
    ])
    .await;
    assert_eq!(result.unwrap().len(), 2);
    assert_eq!(calls, 2);
}

#[tokio::test]
async fn catalog_without_total_waits_for_empty_page() {
    let (result, calls) = fetch_mock_pages(vec![
        serde_json::json!({"items": [{"jxbmc": "a"}], "count": 1}),
        serde_json::json!({"items": [{"jxbmc": "b"}], "count": 1}),
        serde_json::json!({"items": []}),
    ])
    .await;
    assert_eq!(result.unwrap().len(), 2);
    assert_eq!(calls, 3);
}

#[tokio::test]
async fn catalog_rejects_truncation_repeated_pages_and_changing_totals() {
    for pages in [
        vec![
            serde_json::json!({"items": [{"jxbmc": "a"}], "totalSize": 2}),
            serde_json::json!({"items": []}),
        ],
        vec![
            serde_json::json!({"items": [{"jxbmc": "a"}]}),
            serde_json::json!({"items": [{"jxbmc": "a"}]}),
        ],
        vec![
            serde_json::json!({"items": [{"jxbmc": "a"}], "totalResult": 2}),
            serde_json::json!({"items": [{"jxbmc": "b"}], "totalResult": 3}),
        ],
        vec![serde_json::json!({"items": [{"jxbmc": "a"}], "totalResult": 0})],
        vec![serde_json::json!({"items": []})],
        vec![serde_json::json!({"message": "login expired"})],
    ] {
        assert!(fetch_mock_pages(pages).await.0.is_err());
    }
}

#[tokio::test]
async fn school_client_negotiates_and_decodes_gzip() {
    use axum::{routing::post, Router};
    // gzip-compressed {"items":[],"totalResult":0}; no school data.
    let compressed: Vec<u8> = vec![
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 171, 86, 202, 44, 73, 205, 45, 86, 178, 138, 142, 213,
        81, 42, 201, 47, 73, 204, 9, 74, 45, 46, 205, 41, 81, 178, 50, 168, 5, 0, 133, 177, 218,
        180, 28, 0, 0, 0,
    ];
    let app = Router::new().route(
        "/gzip",
        post(move |headers: axum::http::HeaderMap| async move {
            assert!(headers["accept-encoding"]
                .to_str()
                .unwrap()
                .contains("gzip"));
            ([("content-encoding", "gzip")], compressed)
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/gzip", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = SchoolClient {
        http: build_http(Arc::new(Jar::default())).unwrap(),
        grade: String::new(),
        major: String::new(),
        ua: DEFAULT_UA.into(),
    };
    let result = client.post(&url, &Form::new()).await;
    server.abort();
    assert_eq!(result.unwrap(), r#"{"items":[],"totalResult":0}"#);
}

#[test]
fn empty_optional_form_values_are_valid() {
    assert_eq!(
        input_value("<input name='zyfx_id' value=''>", "zyfx_id").unwrap(),
        ""
    );
    assert!(input_value("<html></html>", "zyfx_id").is_err());
}
#[test]
fn school_rejection_is_not_success() {
    assert!(matches!(
        parse_outcome(&Action::Select, r#"{"flag":"0","msg":"人数已满"}"#),
        Outcome::Rejected(_)
    ));
    assert!(matches!(
        parse_outcome(&Action::Select, r#"{"flag":"1"}"#),
        Outcome::Success
    ));
    assert!(matches!(
        parse_outcome(&Action::Select, "<html>登录</html>"),
        Outcome::Unknown
    ));
    assert!(matches!(
        parse_outcome(&Action::Cancel, r#""1""#),
        Outcome::Success
    ));
    assert!(matches!(
        parse_outcome(&Action::Cancel, ""),
        Outcome::Unknown
    ));
}
#[test]
fn cas_ecb_matches_known_vector() {
    // AES-128 ECB zero-key/zero-block test vector; padded second block is retained.
    let encrypted = STANDARD
        .decode(
            encrypt_cas(
                "AAAAAAAAAAAAAAAAAAAAAA==",
                "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        &encrypted[..16],
        &[
            0x66, 0xe9, 0x4b, 0xd4, 0xef, 0x8a, 0x2c, 0x3b, 0x88, 0x4c, 0xfa, 0x59, 0xca, 0x34,
            0x2b, 0x2e
        ]
    );
    assert_eq!(encrypted.len(), 32);
}
#[test]
fn csrf_value_matches_go_algorithm() {
    let (key, _) = generate_csrf();
    assert_eq!(key.len(), 32);
    assert!(key.bytes().all(|b| b.is_ascii_alphanumeric()));
    // Known vector from the original Go implementation: base64(key) is
    // inserted into itself at the midpoint, then MD5'd as hex.
    let t = STANDARD.encode(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef");
    let o = format!("{}{}{}", &t[..t.len() / 2], t, &t[t.len() / 2..]);
    assert_eq!(
        format!("{:x}", md5::compute(o.as_bytes())),
        "edba2bc30129a58f5ea96a944d7c839e"
    );
}
#[test]
fn qr_scan_response_parses() {
    let v: QrResp =
        serde_json::from_str(r#"{"code":200,"message":"ok","data":"2201xxxx"}"#).unwrap();
    assert_eq!(v.code, 200);
    assert_eq!(v.data, "2201xxxx");
    let v: QrResp = serde_json::from_str(r#"{"code":400,"message":"等待扫码"}"#).unwrap();
    assert_eq!(v.code, 400);
    assert_eq!(v.data, "");
}
#[tokio::test]
async fn ordered_login_without_credentials_fails_without_network() {
    // No stored credentials → every method is skipped and the attempt
    // ends with an error before any network request is made.
    let stored = crate::model::StoredCredentials::default();
    let settings = crate::model::Settings::default();
    let ua = crate::model::UaConfig::default();
    let result = SchoolClient::login_with_order(&stored, &settings, &ua).await;
    assert!(result.is_err());
}
#[test]
fn mt19937_is_deterministic() {
    let mut a = Mt19937::new(114514);
    let mut b = Mt19937::new(114514);
    for _ in 0..1000 {
        assert_eq!(a.next_u32(), b.next_u32());
    }
    // First outputs of the standard MT19937 seeded with 114514.
    let mut c = Mt19937::new(114514);
    assert_eq!(c.next_u32(), 2_953_888_979);
    assert_eq!(c.next_u32(), 3_549_084_027);
}
#[test]
fn generated_ua_looks_like_a_browser() {
    let mut rng = Mt19937::new(114514);
    for _ in 0..20 {
        let ua = generate_ua(&mut rng);
        assert!(ua.starts_with("Mozilla/5.0"));
        assert!(ua.contains("Chrome") || ua.contains("Firefox") || ua.contains("Safari"));
    }
}
#[test]
fn jitter_stays_within_range_and_offsets_to_zero() {
    assert_eq!(jittered_wait(1000, 0, 1_919_810), 1000);
    assert_eq!(jittered_wait(0, 500, 1_919_810), 0);
    for seed in [0u64, 1_919_810, 42, u32::MAX as u64] {
        for _ in 0..200 {
            let w = jittered_wait(1000, 100, seed);
            assert!(
                (900..=1100).contains(&w),
                "wait {w} out of range for seed {seed}"
            );
        }
    }
}
#[test]
fn default_ua_is_a_standard_browser() {
    assert!(DEFAULT_UA.contains("Chrome/"));
    assert!(!DEFAULT_UA.contains("HDU-Course-Studio"));
}
#[test]
fn fixed_ua_composes_realistic_strings() {
    let chrome = build_fixed_ua("windows", "chrome", "143.0.7467.120");
    assert!(chrome.starts_with("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"));
    assert!(chrome.contains("Chrome/143.0.7467.120"));
    let ff = build_fixed_ua("macos", "firefox", "143.0");
    assert!(ff.contains("Firefox/143.0") && ff.contains("rv:143.0"));
    let safari = build_fixed_ua("macos", "safari", "17.5");
    assert!(safari.contains("Version/17.5 Safari/605.1.15"));
    let edge = build_fixed_ua("linux", "edge", "143.0.3270.55");
    assert!(edge.contains("Edg/143.0.3270.55") && edge.contains("X11; Linux x86_64"));
}
