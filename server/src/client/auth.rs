use super::ua::resolve_ua;
use super::{build_http, form, json, text_at, value, SchoolClient, JW};
use crate::model::{Credentials, Settings, UaConfig};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::cookie::Jar;
use serde::Deserialize;
use std::sync::Arc;

/// Response shape of the qrlogin API endpoints (loginid and scan share it).
#[derive(Deserialize)]
pub struct QrResp {
    pub code: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: String,
}

impl SchoolClient {
    pub async fn login(
        auth: Credentials,
        settings: &Settings,
        ua: &UaConfig,
    ) -> Result<Self, String> {
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
                let key = text_at(
                    &c.get("https://sso.hdu.edu.cn/login").await?,
                    "#login-croypto",
                    None,
                )?;
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

    pub fn anonymous(ua: &UaConfig) -> Result<Self, String> {
        Ok(Self {
            http: build_http(Arc::new(Jar::default()))?,
            grade: String::new(),
            major: String::new(),
            ua: resolve_ua(ua),
        })
    }

    pub(crate) async fn cas_execution(&self) -> Result<String, String> {
        text_at(
            &self.get("https://sso.hdu.edu.cn/login").await?,
            "#login-page-flowkey",
            None,
        )
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

    pub async fn qr_scan(
        &self,
        id: &str,
        csrf_key: &str,
        csrf_value: &str,
    ) -> Result<QrResp, String> {
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
}

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

pub(super) fn encrypt_cas(key: &str, password: &str) -> Result<String, String> {
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
