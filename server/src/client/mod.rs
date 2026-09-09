//! School HTTP transport. Authentication, catalog and enrollment protocols are separate modules.
mod auth;
mod courses;
mod enrollment;
mod random;
#[cfg(test)]
mod tests;
mod ua;
pub(crate) use auth::generate_csrf;
pub(crate) use enrollment::Outcome;
pub(crate) use random::jittered_wait;
use reqwest::{cookie::Jar, Client};
use scraper::{Html, Selector};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};

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

fn build_http(jar: Arc<Jar>) -> Result<Client, String> {
    Client::builder()
        .cookie_provider(jar)
        // Bound each request; catalog downloads also have an overall deadline.
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "无法初始化网络客户端".into())
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
    async fn get(&self, url: &str) -> Result<String, String> {
        self.get_with(url, &[]).await
    }

    async fn get_with(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, String> {
        let mut request = self
            .http
            .get(url)
            .header(reqwest::header::USER_AGENT, &self.ua);
        for (key, value) in headers {
            request = request.header(*key, *value);
        }
        let response = request.send().await.map_err(|_| "网络请求失败或超时")?;
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
}
