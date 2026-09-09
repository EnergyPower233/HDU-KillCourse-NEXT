use super::{form, json, SchoolClient, JW};
use crate::model::{Course, Settings};
use std::collections::HashSet;

impl SchoolClient {
    pub async fn courses(
        &self,
        settings: &Settings,
        progress: impl FnMut(usize, usize, Option<usize>),
    ) -> Result<Vec<Course>, String> {
        self.courses_from(
            &format!("{JW}/rwlscx/rwlscx_cxRwlsIndex.html?doType=query&gnmkdm=N1548"),
            settings,
            progress,
        )
        .await
    }
    pub(super) async fn courses_from(
        &self,
        url: &str,
        settings: &Settings,
        mut progress: impl FnMut(usize, usize, Option<usize>),
    ) -> Result<Vec<Course>, String> {
        let mut all = Vec::new();
        let mut seen_pages = HashSet::new();
        let mut expected_total = None;
        for page in 1..=200 {
            let data = self
                .post(
                    url,
                    &form(&[
                        ("xnm", &settings.year.to_string()),
                        ("xqm", settings.xqm()),
                        ("xnmc", &format!("{}-{}", settings.year, settings.year + 1)),
                        ("xqmc", &settings.term.to_string()),
                        ("queryModel.showCount", "9999"),
                        ("queryModel.currentPage", &page.to_string()),
                        ("queryModel.sortOrder", "asc"),
                        ("_search", "false"),
                        ("jxbmc", ""),
                    ]),
                )
                .await?;
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
            let rows: Vec<Course> =
                serde_json::from_value(items).map_err(|_| "无法读取课程列表，登录可能已过期")?;
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
}
