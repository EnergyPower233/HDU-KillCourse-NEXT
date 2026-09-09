use super::{form, input_value, json, value, Form, SchoolClient, JW};
use crate::model::{Action, Course, Settings};
use scraper::{Html, Selector};
use serde_json::Value;

#[derive(Debug)]
pub enum Outcome {
    Success,
    Rejected(String),
    Unknown,
}

impl SchoolClient {
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
