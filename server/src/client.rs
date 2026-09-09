//! Protocol adapted from the original Go client. Never log request bodies or cookies.
use crate::model::{Action, Course, Credentials, Settings, UaConfig};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{cookie::Jar, Client};
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

const JW: &str = "https://newjw.hdu.edu.cn/jwglxt";
const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.7467.120 Safari/537.36";
type Form = HashMap<String, String>;

#[derive(Clone)]
pub struct SchoolClient {
    http: Client,
    pub grade: String,
    pub major: String,
    /// User-Agent resolved once when the session is created. It stays the
    /// same for every request, like a real browser.
    ua: String,
}

#[derive(Debug)]
pub enum Outcome {
    Success,
    Rejected(String),
    Unknown,
}

/// Response shape of the qrlogin API endpoints (loginid and scan share it).
#[derive(Deserialize)]
pub struct QrResp {
    pub code: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: String,
}

fn build_http(jar: Arc<Jar>) -> Result<Client, String> {
    Client::builder()
        .cookie_provider(jar)
        // Slow networks and large course pages can take a
        // while; the original Go client sets no timeout at all. 3 minutes is
        // still bounded so a dead request eventually stops instead of hanging.
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "无法初始化网络客户端".into())
}

// ---------------------------------------------------------------------------
// User-Agent resolution: fixed / preset / rotating list / MT19937 generated.
// The UA is set per request so rotation and generation work within one session.
// ---------------------------------------------------------------------------

/// Resolve a single User-Agent for a new session. Like a real browser, the
/// chosen UA is used for every request in that session; rotation/generation
/// only picks a different one on the next login/session.
fn resolve_ua(config: &UaConfig) -> String {
    match config.mode.as_str() {
        "browser" if !config.browser_ua.trim().is_empty() => config.browser_ua.clone(),
        "fixed" => build_fixed_ua(&config.fixed_os, &config.fixed_browser, &config.fixed_version),
        "rotate" => {
            let list: Vec<&str> = config
                .rotate_list
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if list.is_empty() {
                return DEFAULT_UA.to_string();
            }
            use rand::Rng;
            let i = rand::thread_rng().gen_range(0..list.len());
            list[i].to_string()
        }
        "generate" => generated_ua(config.generate_seed),
        _ => DEFAULT_UA.to_string(),
    }
}

/// Compose a realistic UA from OS + browser + version. The frontend enforces
/// Safari-on-macOS, but the composer is lenient and always uses macOS for
/// Safari regardless.
fn build_fixed_ua(os: &str, browser: &str, version: &str) -> String {
    if version.trim().is_empty() {
        return DEFAULT_UA.to_string();
    }
    let platform = |firefox: bool| -> String {
        let base = match os {
            "macos" => "Macintosh; Intel Mac OS X 10.15",
            "linux" => "X11; Linux x86_64",
            _ => "Windows NT 10.0; Win64; x64",
        };
        if firefox {
            format!("{base}; rv:{version}")
        } else if os == "macos" {
            "Macintosh; Intel Mac OS X 10_15_7".to_string()
        } else {
            base.to_string()
        }
    };
    match browser {
        "firefox" => format!(
            "Mozilla/5.0 ({}) Gecko/20100101 Firefox/{version}",
            platform(true)
        ),
        "safari" => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version} Safari/605.1.15"
        ),
        "edge" => format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version} Safari/537.36 Edg/{version}",
            platform(false)
        ),
        "opera" => format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.124 Safari/537.36 OPR/{version}",
            platform(false)
        ),
        _ => format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version} Safari/537.36",
            platform(false)
        ),
    }
}

/// Process-wide MT19937 keyed by seed, so consecutive sessions each draw the
/// next UA from a deterministic, reproducible sequence for the given seed.
fn generated_ua(seed: u64) -> String {
    static STATE: std::sync::OnceLock<Mutex<(u64, Mt19937)>> = std::sync::OnceLock::new();
    let mut guard = STATE
        .get_or_init(|| Mutex::new((seed, Mt19937::new(seed as u32))))
        .lock()
        .unwrap();
    if guard.0 != seed {
        *guard = (seed, Mt19937::new(seed as u32));
    }
    generate_ua(&mut guard.1)
}

/// Standard MT19937 (32-bit), seeded the usual way. Deterministic: the same
/// seed always produces the same sequence.
struct Mt19937 {
    mt: [u32; 624],
    index: usize,
}

impl Mt19937 {
    fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { mt, index: 624 }
    }
    fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            self.twist();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }
    fn twist(&mut self) {
        for i in 0..624 {
            let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7fff_ffff);
            self.mt[i] = self.mt[(i + 397) % 624] ^ (y >> 1);
            if y & 1 != 0 {
                self.mt[i] ^= 0x9908_b0df;
            }
        }
        self.index = 0;
    }
}

/// Jittered watch wait: base interval plus a symmetric random offset in
/// [-jitter, +jitter], drawn from an MT19937 keyed by the user-provided seed.
/// The total is clamped to >= 0. Used to make polling timing look hand-driven.
pub(crate) fn jittered_wait(base_ms: u64, jitter_ms: u64, seed: u64) -> u64 {
    if base_ms == 0 || jitter_ms == 0 {
        return base_ms;
    }
    static STATE: std::sync::OnceLock<Mutex<(u64, Mt19937)>> = std::sync::OnceLock::new();
    let mut guard = STATE
        .get_or_init(|| Mutex::new((seed, Mt19937::new(seed as u32))))
        .lock()
        .unwrap();
    if guard.0 != seed {
        *guard = (seed, Mt19937::new(seed as u32));
    }
    let span = jitter_ms.saturating_mul(2).saturating_add(1);
    let offset = (guard.1.next_u32() as u64 % span) as i64 - jitter_ms as i64;
    (base_ms as i64 + offset).max(0) as u64
}

/// Generate a realistic browser UA from the PRNG (browser + OS + version),
/// using version/build formats that match real browsers.
fn generate_ua(rng: &mut Mt19937) -> String {
    let chrome_major = 120 + (rng.next_u32() % 22); // 120..=141
    let build = 1000 + (rng.next_u32() % 9000); // 4-digit build
    let patch = rng.next_u32() % 1000;
    let firefox_major = 115 + (rng.next_u32() % 27); // 115..=141
    let safari_major = 16 + (rng.next_u32() % 3); // 16..=18
    let safari_minor = rng.next_u32() % 7; // 0..=6
    let family = rng.next_u32() % 8;
    let chrome_ver = |maj: u32| format!("Chrome/{maj}.0.{build}.{patch} Safari/537.36");
    match family {
        0 => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) {}",
            chrome_ver(chrome_major)
        ),
        1 => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_major}.0.{build}.{patch} Safari/537.36 Edg/{chrome_major}.0.{build}.{patch}"
        ),
        2 => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:{firefox_major}.0) Gecko/20100101 Firefox/{firefox_major}.0"
        ),
        3 => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{safari_major}.{safari_minor} Safari/605.1.15"
        ),
        4 => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) {}",
            chrome_ver(chrome_major)
        ),
        5 => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:{firefox_major}.0) Gecko/20100101 Firefox/{firefox_major}.0"
        ),
        _ => format!(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) {}",
            chrome_ver(chrome_major)
        ),
    }
}

/// Mirrors the original Go `GenerateRandomString(32)` / `GenerateCsrfValue`
/// pair used by the qrlogin API headers.
pub fn generate_csrf() -> (String, String) {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let key: String = (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    let t = STANDARD.encode(key.as_bytes());
    let half = t.len() / 2;
    let o = format!("{}{}{}", &t[..half], t, &t[half..]);
    let value = format!("{:x}", md5::compute(o.as_bytes()));
    (key, value)
}

fn text_at(html: &str, selector: &str, attr: Option<&str>) -> Result<String, String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(selector).map_err(|_| "页面解析规则错误")?;
    let el = doc
        .select(&sel)
        .next()
        .ok_or_else(|| format!("页面缺少必要字段：{selector}"))?;
    let s = if let Some(attr) = attr {
        el.value().attr(attr).unwrap_or("").to_string()
    } else {
        el.text().collect::<String>()
    };
    if s.trim().is_empty() {
        Err("页面字段为空，可能需要重新登录".into())
    } else {
        Ok(s.trim().into())
    }
}
fn json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|_| "服务器回复格式异常，请检查登录状态".into())
}
fn value(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}
fn input_value(html: &str, key: &str) -> Result<String, String> {
    let doc = Html::parse_document(html);
    let selector =
        Selector::parse(&format!("input[name='{key}']")).map_err(|_| "页面解析规则错误")?;
    doc.select(&selector)
        .next()
        .and_then(|n| n.value().attr("value"))
        .map(str::to_string)
        .ok_or_else(|| format!("页面缺少字段：{key}，请检查选课阶段和登录状态"))
}
fn form(pairs: &[(&str, &str)]) -> Form {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

impl SchoolClient {
    pub async fn login(auth: Credentials, settings: &Settings, ua: &UaConfig) -> Result<Self, String> {
        let jar = Arc::new(Jar::default());
        if auth.method == "cookie" {
            if [&auth.session_id, &auth.route]
                .iter()
                .any(|v| v.trim().is_empty() || v.contains([';', '\r', '\n']))
            {
                return Err("请输入有效的 JSESSIONID 和 route".into());
            }
            let url = JW.parse().unwrap();
            jar.add_cookie_str(
                &format!("JSESSIONID={}; Path=/", auth.session_id.trim()),
                &url,
            );
            jar.add_cookie_str(&format!("route={}; Path=/", auth.route.trim()), &url);
        } else if auth.username.trim().is_empty() || auth.password.is_empty() {
            return Err("请输入账号和密码".into());
        }
        let mut c = Self {
            http: build_http(jar)?,
            grade: String::new(),
            major: String::new(),
            ua: resolve_ua(ua),
        };
        match auth.method.as_str() {
            "cookie" => {}
            "newjw" => {
                let login = c.get(&format!("{JW}/xtgl/login_slogin.html")).await?;
                let csrf = text_at(&login, "input[name=csrftoken]", Some("value"))?;
                let key = json(&c.get(&format!("{JW}/xtgl/login_getPublicKey.html")).await?)?;
                let n = STANDARD
                    .decode(value(&key, "modulus"))
                    .map_err(|_| "公钥格式错误")?;
                let e = STANDARD
                    .decode(value(&key, "exponent"))
                    .map_err(|_| "公钥指数格式错误")?;
                let pk = rsa::RsaPublicKey::new(
                    rsa::BigUint::from_bytes_be(&n),
                    rsa::BigUint::from_bytes_be(&e),
                )
                .map_err(|_| "无效的登录公钥")?;
                let password = STANDARD.encode(
                    pk.encrypt(
                        &mut rand::thread_rng(),
                        rsa::Pkcs1v15Encrypt,
                        auth.password.as_bytes(),
                    )
                    .map_err(|_| "登录密码加密失败")?,
                );
                c.post(
                    &format!("{JW}/xtgl/login_slogin.html"),
                    &form(&[
                        ("csrftoken", &csrf),
                        ("yhm", &auth.username),
                        ("mm", &password),
                    ]),
                )
                .await?;
            }
            "cas" => {
                let execution = c.cas_execution().await?;
                let key = text_at(&c.get("https://sso.hdu.edu.cn/login").await?, "#login-croypto", None)?;
                let password = encrypt_cas(&key, &auth.password)?;
                c.post(
                    "https://sso.hdu.edu.cn/login",
                    &form(&[
                        ("username", &auth.username),
                        ("password", &password),
                        ("type", "UsernamePassword"),
                        ("_eventId", "submit"),
                        ("execution", &execution),
                        ("croypto", &key),
                        ("captcha_code", ""),
                        ("geolocation", ""),
                    ]),
                )
                .await?;
                c.get(
                    "https://sso.hdu.edu.cn/login?service=http://newjw.hdu.edu.cn/sso/driot4login",
                )
                .await?;
            }
            _ => return Err("不支持的登录方式".into()),
        }
        c.validate_session(settings).await?;
        Ok(c)
    }

    /// A client with a fresh cookie jar, before any login happens. Used by the
    /// DingTalk QR flow, where all requests (loginid / qrgen / scan / CAS
    /// submit) must share one session.
    pub fn anonymous(ua: &UaConfig) -> Result<Self, String> {
        Ok(Self {
            http: build_http(Arc::new(Jar::default()))?,
            grade: String::new(),
            major: String::new(),
            ua: resolve_ua(ua),
        })
    }

    pub(crate) async fn cas_execution(&self) -> Result<String, String> {
        text_at(&self.get("https://sso.hdu.edu.cn/login").await?, "#login-page-flowkey", None)
    }

    async fn validate_session(&mut self, settings: &Settings) -> Result<(), String> {
        let info = json(
            &self
                .get(&format!(
                    "{JW}/kbcx/xskbcx_cxXsgrkb.html?gnmkdm=N2151&xnm={}&xqm={}",
                    settings.year,
                    settings.xqm()
                ))
                .await?,
        )?;
        self.grade = value(&info["xsxx"], "NJDM_ID");
        self.major = value(&info["xsxx"], "ZYH_ID");
        if self.grade.is_empty() || self.major.is_empty() {
            return Err("未能确认登录成功，请检查账号、Cookie 或验证码要求".into());
        }
        Ok(())
    }

    /// Run the stored login methods in the user's preferred order and return
    /// the first successful session plus the method used. QR login is skipped
    /// (it needs interactive scanning, which a background task cannot do).
    pub async fn login_with_order(
        stored: &crate::model::StoredCredentials,
        settings: &Settings,
        ua: &UaConfig,
    ) -> Result<(Self, crate::model::LoginMethod), String> {
        let mut attempted = false;
        let mut last_err = String::from("没有已保存的登录凭证");
        for method in stored.normalized_order() {
            if matches!(method, crate::model::LoginMethod::Qrcode) {
                continue;
            }
            let Some(auth) = stored.as_credentials(method) else {
                continue;
            };
            attempted = true;
            match Self::login(auth, settings, ua).await {
                Ok(client) => return Ok((client, method)),
                Err(e) => last_err = e,
            }
        }
        Err(if attempted {
            format!("所有登录方式均失败：{last_err}")
        } else {
            last_err
        })
    }

    // -- DingTalk QR login (protocol from the original Go CasQrLogin) -------

    pub async fn qr_login_id(&self, csrf_key: &str, csrf_value: &str) -> Result<String, String> {
        let v: QrResp = serde_json::from_str(
            &self
                .get_with(
                    "https://sso.hdu.edu.cn/api/protected/qrlogin/loginid",
                    &[("Csrf-Key", csrf_key), ("Csrf-Value", csrf_value)],
                )
                .await?,
        )
        .map_err(|_| "获取登录二维码失败：服务器回复格式异常".to_string())?;
        if v.data.trim().is_empty() {
            return Err(if v.message.trim().is_empty() {
                "获取登录二维码失败".into()
            } else {
                format!("获取登录二维码失败：{}", v.message)
            });
        }
        Ok(v.data)
    }

    pub async fn qr_code(&self, id: &str) -> Result<Vec<u8>, String> {
        self.get_bytes(&format!(
            "https://sso.hdu.edu.cn/api/public/qrlogin/qrgen/{}/dingDingQr",
            id
        ))
        .await
    }

    pub async fn qr_scan(&self, id: &str, csrf_key: &str, csrf_value: &str) -> Result<QrResp, String> {
        serde_json::from_str(
            &self
                .get_with(
                    &format!("https://sso.hdu.edu.cn/api/protected/qrlogin/scan/{id}"),
                    &[("Csrf-Key", csrf_key), ("Csrf-Value", csrf_value)],
                )
                .await?,
        )
        .map_err(|_| "查询扫码状态失败：服务器回复格式异常".to_string())
    }

    pub async fn qr_login_complete(
        &mut self,
        execution: &str,
        username: &str,
        settings: &Settings,
    ) -> Result<(), String> {
        let reply = self
            .post(
                "https://sso.hdu.edu.cn/login",
                &form(&[
                    ("username", username),
                    ("password", ""),
                    ("type", "dingDingQr"),
                    ("_eventId", "submit"),
                    ("execution", execution),
                    ("croypto", ""),
                    ("captcha_code", ""),
                    ("geolocation", ""),
                ]),
            )
            .await?;
        if reply.contains("用户名密码登录") {
            return Err("钉钉扫码登录失败，请重新获取二维码".into());
        }
        let reply = self
            .get("https://sso.hdu.edu.cn/login?service=http://newjw.hdu.edu.cn/sso/driot4login")
            .await?;
        if !reply.contains("杭州电子科技大学本科教学管理服务平台") {
            return Err("未知错误：无法通过扫码会话进入教务系统，请重试".into());
        }
        self.validate_session(settings).await
    }

    async fn get(&self, url: &str) -> Result<String, String> {
        self.get_with(url, &[]).await
    }
    async fn get_with(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
        let mut request = self.http.get(url).header(reqwest::header::USER_AGENT, &self.ua);
        for (key, value) in headers {
            request = request.header(*key, *value);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "网络请求失败或超时")?;
        response
            .error_for_status()
            .map_err(|_| "服务器暂时不可用")?
            .text()
            .await
            .map_err(|_| "读取服务器回复失败".into())
    }
    async fn get_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self
            .http
            .get(url)
            .header(reqwest::header::USER_AGENT, &self.ua)
            .send()
            .await
            .map_err(|_| "网络请求失败或超时")?;
        response
            .error_for_status()
            .map_err(|_| "服务器暂时不可用")?
            .bytes()
            .await
            .map_err(|_| "读取服务器回复失败".into())
            .map(|b| b.to_vec())
    }
    async fn post(&self, url: &str, body: &Form) -> Result<String, String> {
        let response = self
            .http
            .post(url)
            .header(reqwest::header::USER_AGENT, &self.ua)
            .form(body)
            .send()
            .await
            .map_err(|_| "网络请求失败或超时")?;
        response
            .error_for_status()
            .map_err(|_| "服务器暂时不可用")?
            .text()
            .await
            .map_err(|_| "读取服务器回复失败".into())
    }
    pub async fn courses(
        &self,
        settings: &Settings,
        progress: impl FnMut(usize, usize, Option<usize>),
    ) -> Result<Vec<Course>, String> {
        self.courses_from(
            &format!("{JW}/rwlscx/rwlscx_cxRwlsIndex.html?doType=query&gnmkdm=N1548"),
            settings,
            progress,
        ).await
    }

    async fn courses_from(
        &self,
        url: &str,
        settings: &Settings,
        mut progress: impl FnMut(usize, usize, Option<usize>),
    ) -> Result<Vec<Course>, String> {
        let mut all = Vec::new();
        let mut seen_pages = HashSet::new();
        let mut expected_total = None;
        for page in 1..=200 {
            let data = self.post(url, &form(&[
                ("xnm", &settings.year.to_string()),
                ("xqm", settings.xqm()),
                ("xnmc", &format!("{}-{}", settings.year, settings.year + 1)),
                ("xqmc", &settings.term.to_string()),
                ("queryModel.showCount", "9999"),
                ("queryModel.currentPage", &page.to_string()),
                ("queryModel.sortOrder", "asc"),
                ("_search", "false"),
                ("jxbmc", ""),
            ])).await?;
            if data.contains("无功能权限") {
                return Err("任务落实查询尚未开放".into());
            }
            let mut v = json(&data)?;
            // `count` may describe this page, not the entire catalog.
            let total = ["totalResult", "totalCount", "totalSize"]
                .iter()
                .filter_map(|k| v[*k].as_u64().or_else(|| v[*k].as_str()?.parse().ok()))
                .next()
                .and_then(|n| usize::try_from(n).ok());
            if let Some(total) = total {
                if expected_total.is_some_and(|old| old != total) {
                    return Err("下载期间课程总数发生变化，请重新获取，未保存不完整资料".into());
                }
                expected_total = Some(total);
            }
            let items = v["items"].take();
            let fingerprint = md5::compute(items.to_string());
            let rows: Vec<Course> = serde_json::from_value(items)
                .map_err(|_| "无法读取课程列表，登录可能已过期")?;
            let count = rows.len();
            if count > 0 && !seen_pages.insert(fingerprint) {
                return Err("学校返回了重复课程页，未保存不完整资料".into());
            }
            all.extend(rows);
            if all.len() > 100_000 || expected_total.is_some_and(|n| all.len() > n) {
                return Err("课程数量超出上限或与学校总数不符，未保存不完整资料".into());
            }
            progress(page, all.len(), expected_total);
            // A server may cap the requested page size. A short page alone
            // therefore cannot establish completeness; use its total or EOF.
            if count == 0 || expected_total == Some(all.len()) {
                if expected_total.is_some_and(|n| all.len() != n) {
                    return Err("学校提前返回空页，课程数量不足，未保存不完整资料".into());
                }
                return if all.is_empty() {
                    Err("该学期没有查到课程".into())
                } else {
                    crate::model::normalize_courses(all)
                };
            }
        }
        Err("课程数量超过分页上限，未保存不完整资料".into())
    }
    pub async fn body_config(&self) -> Result<Form, String> {
        let html = self
            .get(&format!(
                "{JW}/xsxk/zzxkyzb_cxZzxkYzbIndex.html?gnmkdm=N253512&layout=default"
            ))
            .await?;
        if html.contains("当前不属于选课阶段") {
            return Err("当前不属于选课阶段".into());
        }
        let mut result = Form::new();
        for key in [
            "ccdm", "bh_id", "jg_id_1", "xsbj", "xz", "mzm", "xslbdm", "xbm", "zyfx_id", "xqh_id",
        ] {
            result.insert(key.replace("jg_id_1", "jg_id"), input_value(&html, key)?);
        }
        let doc = Html::parse_document(&html);
        let re = regex::Regex::new(r#"queryCourse\(this,'(\d+)','([^']+)'"#).unwrap();
        for tab in doc.select(&Selector::parse("a[onclick]").unwrap()) {
            if let Some(caps) = re.captures(tab.value().attr("onclick").unwrap_or("")) {
                result.insert(format!("control_{}", &caps[1]), caps[2].into());
            }
        }
        Ok(result)
    }
    pub async fn available(&self, s: &Settings, course: &Course) -> Result<bool, String> {
        let body = form(&[
            ("xkxnm", &s.year.to_string()),
            ("xkxqm", s.xqm()),
            ("kklxdm", category(course)?),
            ("jspage", "10"),
            ("kspage", "1"),
            ("yl_list[0]", "1"),
            ("filter_list[0]", &course.jxbmc),
            ("njdm_id_xs", &self.grade),
            ("zyh_id_xs", &self.major),
        ]);
        let v = json(
            &self
                .post(
                    &format!("{JW}/xsxk/zzxkyzb_cxZzxkYzbPartDisplay.html?gnmkdm=N253512"),
                    &body,
                )
                .await?,
        )?;
        let rows = v["tmpList"]
            .as_array()
            .ok_or("余量查询失败，登录可能已过期")?;
        Ok(rows.iter().any(|v| value(v, "jxbmc") == course.jxbmc))
    }
    pub async fn prepare(
        &self,
        s: &Settings,
        course: &Course,
        config: &Form,
    ) -> Result<Form, String> {
        let cat = category(course)?;
        let control = config
            .get(&format!("control_{cat}"))
            .ok_or("该课程类型尚未开放或缺少选课参数")?;
        let mut body: Form = config
            .iter()
            .filter(|(k, _)| !k.starts_with("control_"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        body.extend(form(&[
            ("bklx_id", "0"),
            ("njdm_id", &self.grade),
            ("xkxnm", &s.year.to_string()),
            ("xkxqm", s.xqm()),
            ("kklxdm", cat),
            ("kch_id", &course.kch_id),
            ("xkkz_id", control),
            ("njdm_id_xs", &self.grade),
            ("zyh_id_xs", &self.major),
        ]));
        let v = json(
            &self
                .post(
                    &format!("{JW}/xsxk/zzxkyzbjk_cxJxbWithKchZzxkYzb.html?gnmkdm=N253512"),
                    &body,
                )
                .await?,
        )?;
        let id = v
            .as_array()
            .and_then(|a| a.iter().find(|v| value(v, "jxb_id") == course.jxb_id))
            .map(|v| value(v, "do_jxb_id"))
            .filter(|s| !s.is_empty())
            .ok_or("没有找到可操作的教学班")?;
        Ok(form(&[
            ("jxb_ids", &id),
            ("kch_id", &course.kch_id),
            ("qz", "0"),
            ("xkkz_id", control),
            ("njdm_id", if cat == "01" { &self.grade } else { "" }),
            ("zyh_id", if cat == "01" { &self.major } else { "" }),
            ("njdm_id_xs", &self.grade),
            ("zyh_id_xs", &self.major),
            ("xkxnm", &s.year.to_string()),
            ("xkxqm", s.xqm()),
        ]))
    }
    pub async fn submit(&self, action: &Action, body: &Form) -> Outcome {
        let endpoint = match action {
            Action::Select => "zzxkyzbjk_xkBcZyZzxkYzb",
            Action::Cancel => "zzxkyzb_tuikBcZzxkYzb",
        };
        match self
            .post(&format!("{JW}/xsxk/{endpoint}.html?gnmkdm=N253512"), body)
            .await
        {
            Ok(reply) => parse_outcome(action, &reply),
            Err(_) => Outcome::Unknown,
        }
    }
}

fn category(c: &Course) -> Result<&'static str, String> {
    match c.kklxmc.as_str() {
        "主修课程" => Ok("01"),
        "通识选修课" => Ok("10"),
        "体育分项" => Ok("05"),
        "特殊课程" => Ok("09"),
        _ => Err(format!("暂不支持课程类型：{}", c.kklxmc)),
    }
}
pub fn parse_outcome(action: &Action, reply: &str) -> Outcome {
    let Ok(v) = serde_json::from_str::<Value>(reply) else {
        return Outcome::Unknown;
    };
    match action {
        Action::Select => match v["flag"].as_str() {
            Some("1") => Outcome::Success,
            Some("0") => Outcome::Rejected(
                v["msg"]
                    .as_str()
                    .unwrap_or("学校拒绝本次选课")
                    .chars()
                    .take(500)
                    .collect(),
            ),
            _ => Outcome::Unknown,
        },
        Action::Cancel => {
            if v.as_str() == Some("1") {
                Outcome::Success
            } else {
                Outcome::Unknown
            }
        }
    }
}
fn encrypt_cas(key: &str, password: &str) -> Result<String, String> {
    use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
    let key = STANDARD.decode(key).map_err(|_| "CAS 密钥格式错误")?;
    let mut bytes = password.as_bytes().to_vec();
    let pad = 16 - bytes.len() % 16;
    bytes.extend(std::iter::repeat_n(pad as u8, pad));
    macro_rules! encrypt {
        ($cipher:ty) => {{
            let cipher = <$cipher>::new_from_slice(&key).map_err(|_| "CAS 密钥长度错误")?;
            for chunk in bytes.chunks_mut(16) {
                cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
            }
        }};
    }
    match key.len() {
        16 => encrypt!(aes::Aes128),
        24 => encrypt!(aes::Aes192),
        32 => encrypt!(aes::Aes256),
        _ => return Err("CAS 密钥长度错误".into()),
    }
    Ok(STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fetch_mock_pages(pages: Vec<Value>) -> (Result<Vec<Course>, String>, usize) {
        use axum::{routing::post, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let app = Router::new().route("/courses", post(move |axum::Form(body): axum::Form<HashMap<String, String>>| {
            let i = observed.fetch_add(1, Ordering::SeqCst);
            let response = pages.get(i).cloned().unwrap_or_else(|| serde_json::json!({"items": []}));
            async move {
                assert_eq!(body["queryModel.showCount"], "9999");
                assert_eq!(body["queryModel.currentPage"], (i + 1).to_string());
                axum::Json(response)
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/courses", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = SchoolClient {
            http: build_http(Arc::new(Jar::default())).unwrap(),
            grade: String::new(), major: String::new(), ua: DEFAULT_UA.into(),
        };
        let result = client.courses_from(&url, &Settings::default(), |_, _, _| {}).await;
        server.abort();
        (result, calls.load(Ordering::SeqCst))
    }

    #[tokio::test]
    async fn catalog_large_page_and_capped_pages_are_complete() {
        let rows: Vec<Value> = (0..5193).map(|i| serde_json::json!({"jxbmc": format!("class-{i}")})).collect();
        let (result, calls) = fetch_mock_pages(vec![serde_json::json!({"items": rows, "totalResult": "5193"})]).await;
        assert_eq!(result.unwrap().len(), 5193);
        assert_eq!(calls, 1);

        let (result, calls) = fetch_mock_pages(vec![
            serde_json::json!({"items": [{"jxbmc": "a"}], "totalCount": 2}),
            serde_json::json!({"items": [{"jxbmc": "b"}], "totalCount": 2}),
        ]).await;
        assert_eq!(result.unwrap().len(), 2);
        assert_eq!(calls, 2);
    }

    #[tokio::test]
    async fn catalog_without_total_waits_for_empty_page() {
        let (result, calls) = fetch_mock_pages(vec![
            serde_json::json!({"items": [{"jxbmc": "a"}], "count": 1}),
            serde_json::json!({"items": [{"jxbmc": "b"}], "count": 1}),
            serde_json::json!({"items": []}),
        ]).await;
        assert_eq!(result.unwrap().len(), 2);
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn catalog_rejects_truncation_repeated_pages_and_changing_totals() {
        for pages in [
            vec![serde_json::json!({"items": [{"jxbmc": "a"}], "totalSize": 2}), serde_json::json!({"items": []})],
            vec![serde_json::json!({"items": [{"jxbmc": "a"}]}), serde_json::json!({"items": [{"jxbmc": "a"}]})],
            vec![serde_json::json!({"items": [{"jxbmc": "a"}], "totalResult": 2}), serde_json::json!({"items": [{"jxbmc": "b"}], "totalResult": 3})],
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
        let compressed: Vec<u8> = vec![31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 171, 86, 202, 44, 73, 205, 45, 86, 178, 138, 142, 213, 81, 42, 201, 47, 73, 204, 9, 74, 45, 46, 205, 41, 81, 178, 50, 168, 5, 0, 133, 177, 218, 180, 28, 0, 0, 0];
        let app = Router::new().route("/gzip", post(move |headers: axum::http::HeaderMap| async move {
            assert!(headers["accept-encoding"].to_str().unwrap().contains("gzip"));
            ([("content-encoding", "gzip")], compressed)
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/gzip", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = SchoolClient {
            http: build_http(Arc::new(Jar::default())).unwrap(),
            grade: String::new(), major: String::new(), ua: DEFAULT_UA.into(),
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
                assert!((900..=1100).contains(&w), "wait {w} out of range for seed {seed}");
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
}
