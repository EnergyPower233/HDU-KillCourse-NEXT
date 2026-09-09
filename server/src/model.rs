use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Course {
    pub jxbmc: String,
    pub kch_id: String,
    pub jxb_id: String,
    pub kcmc: String,
    pub kklxmc: String,
    pub sksj: String,
    pub jxbzc: String,
    pub jzgxx: String,
    pub jxdd: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Select,
    Cancel,
}

/// A task: optionally select `course` after first dropping every course in
/// `drops` (chosen manually by the user). `course` None + drops = pure drop.
#[derive(Clone, Debug, Serialize)]
pub struct CourseTask {
    pub course: Option<Course>,
    pub drops: Vec<Course>,
}

impl<'de> Deserialize<'de> for CourseTask {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            course: Option<Course>,
            drops: Option<Vec<Course>>,
            action: Option<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        if raw.drops.is_some() || raw.action.is_none() {
            // New shape (or already-normalized legacy select task).
            Ok(CourseTask {
                course: raw.course,
                drops: raw.drops.unwrap_or_default(),
            })
        } else if raw.action.as_deref() == Some("cancel") {
            // Legacy {course, action:"cancel"} → pure drop of that course.
            Ok(CourseTask {
                course: None,
                drops: raw.course.into_iter().collect(),
            })
        } else {
            Ok(CourseTask {
                course: raw.course,
                drops: vec![],
            })
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Once,
    Watch,
}

/// A named, editable collection of course tasks with its own execution mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskList {
    pub name: String,
    pub mode: Mode,
    pub tasks: Vec<CourseTask>,
}

impl Default for TaskList {
    fn default() -> Self {
        Self {
            name: "默认清单".into(),
            mode: Mode::Watch,
            tasks: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub year: u32,
    pub term: u8,
    /// Query interval in milliseconds (canonical unit; the UI lets the user
    /// pick ms / s / min / h). Only used by watch-mode lists.
    pub interval_ms: u64,
    /// Maximum random offset in milliseconds around the watch interval; 0 disables jitter.
    pub jitter_ms: u64,
    /// MT19937 seed for the jitter (default 1919810).
    pub jitter_seed: u64,
    /// Start time as `YYYY-MM-DD HH:MM:SS.mmm` in fixed UTC+8, or empty for
    /// immediate start. Millisecond precision.
    pub start_at: String,
    /// Seconds before `start_at` to re-run the ordered login attempts. 0
    /// disables it; only used when a start time is set.
    pub relogin_before_secs: u64,
    /// Saved task lists; always at least one entry.
    pub lists: Vec<TaskList>,
    /// Index into `lists` of the list currently being edited.
    pub active_list: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            year: 2026,
            term: 1,
            interval_ms: 60_000,
            jitter_ms: 0,
            jitter_seed: 1_919_810,
            start_at: String::new(),
            relogin_before_secs: 0,
            lists: vec![TaskList::default()],
            active_list: 0,
        }
    }
}

impl Settings {
    pub fn list(&self, index: usize) -> Option<&TaskList> {
        self.lists.get(index)
    }
    fn validate_list(&self, list: &TaskList, starting: bool) -> Result<(), String> {
        if starting && list.tasks.is_empty() {
            return Err(format!("清单「{}」还没有课程任务", list.name));
        }
        if list.tasks.len() > 100 {
            return Err("单个清单任务数量不能超过 100 门".into());
        }
        let prefix = format!("({}-{}-{})-", self.year, self.year + 1, self.term);
        let mut seen = std::collections::HashSet::new();
        let check =
            |course: &Course, seen: &mut std::collections::HashSet<String>| -> Result<(), String> {
                if !course.jxbmc.starts_with(&prefix) {
                    return Err(format!("教学班与学期不匹配：{}", course.jxbmc));
                }
                if course.jxb_id.is_empty() || course.kch_id.is_empty() {
                    return Err("课程缺少内部编号，请重新获取课程资料".into());
                }
                if !seen.insert(course.jxbmc.clone()) {
                    return Err(format!("同一清单内教学班重复：{}", course.jxbmc));
                }
                Ok(())
            };
        for task in &list.tasks {
            if task.course.is_none() && task.drops.is_empty() {
                return Err("存在既没有要选课程也没有要退课程的空任务".into());
            }
            if matches!(list.mode, Mode::Watch) && (task.course.is_none() || !task.drops.is_empty())
            {
                return Err("蹲课仅支持纯选课；带退课或仅退课的任务请使用单次模式".into());
            }
            if let Some(course) = &task.course {
                check(course, &mut seen)?;
            }
            for drop in &task.drops {
                check(drop, &mut seen)?;
            }
        }
        Ok(())
    }
    pub fn validate(&self, starting: bool, list_index: usize) -> Result<(), String> {
        if !(2000..=2100).contains(&self.year) || ![1, 2].contains(&self.term) {
            return Err("学年或学期不合法".into());
        }
        if !(100..=86_400_000).contains(&self.interval_ms) {
            return Err("查询间隔应为 100 毫秒～24 小时".into());
        }
        if self.jitter_ms > 3_600_000 {
            return Err("波动范围不能超过 3600 秒".into());
        }
        if self.relogin_before_secs > 86_400 {
            return Err("提前重新登录不能超过 86400 秒（24 小时）".into());
        }
        if self.lists.is_empty() {
            return Err("至少需要一个任务清单".into());
        }
        if list_index >= self.lists.len() {
            return Err("要执行的清单不存在".into());
        }
        for list in &self.lists {
            self.validate_list(list, false)?;
        }
        self.validate_list(self.list(list_index).unwrap(), starting)?;
        self.delay_ms()?;
        Ok(())
    }
    pub fn xqm(&self) -> &'static str {
        if self.term == 1 {
            "3"
        } else {
            "12"
        }
    }
    /// Milliseconds until the scheduled start (UTC+8), 0 if no start time set
    /// or the time already passed.
    pub fn delay_ms(&self) -> Result<u64, String> {
        if self.start_at.trim().is_empty() {
            return Ok(0);
        }
        use chrono::TimeZone;
        let formats = [
            "%Y-%m-%d %H:%M:%S%.3f",
            "%Y-%m-%d %H:%M:%S%.f",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%dT%H:%M:%S%.3f",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M",
        ];
        let time = formats
            .iter()
            .find_map(|f| chrono::NaiveDateTime::parse_from_str(self.start_at.trim(), f).ok())
            .ok_or("开始时间格式错误，应为 YYYY-MM-DD HH:MM:SS.mmm（UTC+8）")?;
        let target = chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .from_local_datetime(&time)
            .single()
            .unwrap();
        Ok((target.timestamp_millis() - chrono::Utc::now().timestamp_millis()).max(0) as u64)
    }
}

#[derive(Deserialize)]
pub struct Credentials {
    pub method: String,
    pub username: String,
    pub password: String,
    pub session_id: String,
    pub route: String,
}

/// The four login methods, in no particular order; `StoredCredentials.order`
/// carries the user's preferred attempt order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginMethod {
    Cas,
    Newjw,
    Qrcode,
    Cookie,
}

/// User-chosen credentials persisted in the local data directory
/// (`credentials.json`, plaintext like the original Go config.json).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredCredentials {
    pub cas_username: String,
    pub cas_password: String,
    pub newjw_username: String,
    pub newjw_password: String,
    pub session_id: String,
    pub route: String,
    pub order: Vec<LoginMethod>,
}

impl Default for StoredCredentials {
    fn default() -> Self {
        Self {
            cas_username: String::new(),
            cas_password: String::new(),
            newjw_username: String::new(),
            newjw_password: String::new(),
            session_id: String::new(),
            route: String::new(),
            order: vec![
                LoginMethod::Cas,
                LoginMethod::Newjw,
                LoginMethod::Qrcode,
                LoginMethod::Cookie,
            ],
        }
    }
}

impl StoredCredentials {
    /// The request-shaped credentials for a method, or None when they are not
    /// saved (QR login is interactive and never pre-filled). Mirrors the
    /// frontend helper; used by tests and future server-side login flows.
    #[allow(dead_code)]
    pub fn as_credentials(&self, method: LoginMethod) -> Option<Credentials> {
        let make = |method: &str, username: String, password: String| Credentials {
            method: method.into(),
            username,
            password,
            session_id: String::new(),
            route: String::new(),
        };
        match method {
            LoginMethod::Cas if !self.cas_username.is_empty() && !self.cas_password.is_empty() => {
                Some(make(
                    "cas",
                    self.cas_username.clone(),
                    self.cas_password.clone(),
                ))
            }
            LoginMethod::Newjw
                if !self.newjw_username.is_empty() && !self.newjw_password.is_empty() =>
            {
                Some(make(
                    "newjw",
                    self.newjw_username.clone(),
                    self.newjw_password.clone(),
                ))
            }
            LoginMethod::Cookie if !self.session_id.is_empty() && !self.route.is_empty() => {
                Some(Credentials {
                    method: "cookie".into(),
                    username: String::new(),
                    password: String::new(),
                    session_id: self.session_id.clone(),
                    route: self.route.clone(),
                })
            }
            _ => None,
        }
    }

    /// Merge a successfully used credential set back into storage.
    #[allow(dead_code)]
    pub fn update_from(&mut self, auth: &Credentials) {
        match auth.method.as_str() {
            "cas" => {
                self.cas_username = auth.username.clone();
                self.cas_password = auth.password.clone();
            }
            "newjw" => {
                self.newjw_username = auth.username.clone();
                self.newjw_password = auth.password.clone();
            }
            "cookie" => {
                self.session_id = auth.session_id.clone();
                self.route = auth.route.clone();
            }
            _ => {}
        }
    }

    /// Deduplicate the stored order and append any missing methods, so the
    /// list always contains the four methods exactly once.
    pub fn normalized_order(&self) -> Vec<LoginMethod> {
        let all = [
            LoginMethod::Cas,
            LoginMethod::Newjw,
            LoginMethod::Qrcode,
            LoginMethod::Cookie,
        ];
        let mut out: Vec<LoginMethod> = Vec::new();
        for m in &self.order {
            if all.contains(m) && !out.contains(m) {
                out.push(*m);
            }
        }
        for m in all {
            if !out.contains(&m) {
                out.push(m);
            }
        }
        out
    }
}

/// User-Agent configuration sent with school HTTP requests.
/// Modes: `browser` (the user's actual browser UA, default), `fixed` (built
/// from OS + browser + version), `rotate` (a list) and `generate` (MT19937
/// with a user seed).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UaConfig {
    pub mode: String,
    /// Captured from the frontend via `navigator.userAgent` when mode is
    /// `browser`.
    pub browser_ua: String,
    pub fixed_os: String,
    pub fixed_browser: String,
    pub fixed_version: String,
    pub rotate_list: Vec<String>,
    pub generate_seed: u64,
}

impl Default for UaConfig {
    fn default() -> Self {
        Self {
            mode: "browser".into(),
            browser_ua: String::new(),
            fixed_os: "windows".into(),
            fixed_browser: "chrome".into(),
            fixed_version: "143.0.7467.120".into(),
            rotate_list: vec![],
            generate_seed: 114514,
        }
    }
}

impl UaConfig {
    pub fn normalize(&mut self) {
        if !matches!(
            self.mode.as_str(),
            "browser" | "fixed" | "rotate" | "generate"
        ) {
            self.mode = "browser".into();
        }
        self.browser_ua = self.browser_ua.trim().to_string();
        if self.fixed_os.trim().is_empty() {
            self.fixed_os = "windows".into();
        }
        if self.fixed_browser.trim().is_empty() {
            self.fixed_browser = "chrome".into();
        }
        if self.fixed_version.trim().is_empty() {
            self.fixed_version = "143.0.7467.120".into();
        }
        self.rotate_list = self
            .rotate_list
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Progress {
    pub course_id: String,
    #[serde(default)]
    pub course_name: String,
    #[serde(default)]
    pub schedule: String,
    #[serde(default)]
    pub action: Option<Action>,
    pub status: String,
    pub message: String,
    pub time: String,
}
impl Progress {
    pub fn for_course(course: &Course, action: Action, status: &str, message: &str) -> Self {
        let mut event = Self::new(&course.jxbmc, status, message);
        event.course_name = course.kcmc.clone();
        event.schedule = course.sksj.clone();
        event.action = Some(action);
        event
    }
    pub fn new(id: &str, status: &str, message: &str) -> Self {
        Self {
            course_id: id.into(),
            course_name: String::new(),
            schedule: String::new(),
            action: None,
            status: status.into(),
            message: message.into(),
            time: chrono::Local::now().to_rfc3339(),
        }
    }
}

pub fn parse_courses(text: &str) -> Result<Vec<Course>, String> {
    #[derive(Deserialize)]
    struct List {
        items: Vec<Course>,
    }
    let courses = serde_json::from_str::<List>(text)
        .map(|v| v.items)
        .or_else(|_| serde_json::from_str::<Vec<Course>>(text))
        .map_err(|_| "不是有效的 course.json 课程列表".to_string())?;
    if courses.is_empty() {
        return Err("课程列表为空".into());
    }
    if courses
        .iter()
        .any(|c| c.jxbmc.is_empty() || c.jxb_id.is_empty() || c.kch_id.is_empty())
    {
        return Err("课程资料缺少教学班名称或内部编号".into());
    }
    normalize_courses(courses)
}

pub fn normalize_courses(courses: Vec<Course>) -> Result<Vec<Course>, String> {
    let mut result: Vec<Course> = Vec::new();
    let mut indexes = std::collections::HashMap::<String, usize>::new();
    for course in courses {
        if let Some(index) = indexes.get(&course.jxbmc) {
            let old = &mut result[*index];
            if old.jxb_id != course.jxb_id || old.kch_id != course.kch_id {
                return Err(format!("同一教学班存在不同内部编号：{}", course.jxbmc));
            }
            fn merge(old: &mut String, new: &str) {
                let mut values: Vec<&str> = old.split(';').filter(|s| !s.is_empty()).collect();
                for part in new.split(';').filter(|s| !s.is_empty()) {
                    if !values.contains(&part) {
                        values.push(part);
                    }
                }
                *old = values.join(";");
            }
            merge(&mut old.sksj, &course.sksj);
            merge(&mut old.jxbzc, &course.jxbzc);
            merge(&mut old.jzgxx, &course.jzgxx);
            merge(&mut old.jxdd, &course.jxdd);
        } else {
            indexes.insert(course.jxbmc.clone(), result.len());
            result.push(course);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn course_events_keep_operation_and_identity_without_course_cache() {
        let course = Course {
            jxbmc: "class-01".into(),
            kcmc: "数据结构".into(),
            sksj: "星期三3-4节".into(),
            ..Default::default()
        };
        for action in [Action::Select, Action::Cancel] {
            let event = Progress::for_course(&course, action, "success", "学校已返回成功");
            let saved = serde_json::to_string(&event).unwrap();
            let restored: Progress = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.course_name, "数据结构");
            assert_eq!(restored.course_id, "class-01");
            assert_eq!(restored.schedule, "星期三3-4节");
            assert!(restored.action.is_some());
            assert!(chrono::DateTime::parse_from_rfc3339(&restored.time).is_ok());
        }
        let legacy: Progress = serde_json::from_str(
            r#"{"course_id":"old","status":"success","message":"ok","time":"10:00:00"}"#,
        )
        .unwrap();
        assert!(legacy.action.is_none());
        assert!(legacy.course_name.is_empty());
    }
    #[test]
    fn merges_duplicate_teaching_rows_but_not_ambiguous_ids() {
        let course = Course {
            jxbmc: "A".into(),
            jxb_id: "a".into(),
            kch_id: "A".into(),
            sksj: "星期一".into(),
            ..Default::default()
        };
        let mut other = course.clone();
        other.sksj = "星期三".into();
        let merged = normalize_courses(vec![course.clone(), other.clone()]).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sksj, "星期一;星期三");
        other.jxb_id = "b".into();
        assert!(normalize_courses(vec![course, other]).is_err());
    }
    #[test]
    fn rejects_empty_and_invalid_schedule() {
        let mut s = Settings::default();
        assert!(s.validate(true, 0).is_err()); // active list empty when starting
        s.start_at = "tomorrow".into();
        assert!(s.validate(false, 0).is_err());
        s.start_at.clear();
        s.interval_ms = 0;
        assert!(s.validate(false, 0).is_err());
        s.interval_ms = 60_000;
        s.relogin_before_secs = 200_000;
        assert!(s.validate(false, 0).is_err());
        s.relogin_before_secs = 86_400;
        assert!(s.validate(false, 0).is_ok());
        s.jitter_ms = 4_000_000;
        assert!(s.validate(false, 0).is_err());
        s.jitter_ms = 3_600_000;
        assert!(s.validate(false, 0).is_ok());
    }
    #[test]
    fn parses_millisecond_start_time() {
        let mut s = Settings::default();
        s.start_at = "2026-09-09 12:00:00.000".into();
        assert!(s.delay_ms().is_ok());
        // Millisecond precision is preserved in the timestamp arithmetic.
        s.start_at = "2026-09-09 12:00:00.500".into();
        assert!(s.delay_ms().is_ok());
        // Legacy minute-precision format still parses.
        s.start_at = "2026-09-09T12:00".into();
        assert!(s.delay_ms().is_ok());
    }
    #[test]
    fn imports_go_cache_and_rejects_missing_ids() {
        assert_eq!(
            parse_courses(r#"{"items":[{"jxbmc":"test","jxb_id":"internal","kch_id":"course"}]}"#)
                .unwrap()
                .len(),
            1
        );
        assert!(parse_courses(r#"{"items":[{"jxbmc":"test"}]}"#).is_err());
    }
    #[test]
    fn refuses_watch_drops_and_wrong_term() {
        let mut s = Settings::default();
        let course = Course {
            jxbmc: "(2026-2027-1)-A-01".into(),
            jxb_id: "a".into(),
            kch_id: "a".into(),
            ..Default::default()
        };
        // A watch task with drops is invalid.
        s.lists[0].tasks.push(CourseTask {
            course: Some(course.clone()),
            drops: vec![Course {
                jxbmc: "(2026-2027-1)-A-02".into(),
                ..course.clone()
            }],
        });
        assert!(s.validate(true, 0).is_err());
        // A pure-drop task in watch mode is invalid too.
        s.lists[0].tasks = vec![CourseTask {
            course: None,
            drops: vec![course.clone()],
        }];
        assert!(s.validate(true, 0).is_err());
        // Once mode allows paired tasks.
        s.lists[0].mode = Mode::Once;
        s.lists[0].tasks = vec![CourseTask {
            course: Some(course.clone()),
            drops: vec![Course {
                jxbmc: "(2026-2027-1)-A-02".into(),
                ..course.clone()
            }],
        }];
        assert!(s.validate(true, 0).is_ok());
        s.term = 2;
        assert!(s.validate(true, 0).is_err());
    }
    #[test]
    fn rejects_duplicates_and_empty_tasks() {
        let mut s = Settings::default();
        s.lists[0].mode = Mode::Once;
        let course = Course {
            jxbmc: "(2026-2027-1)-A-01".into(),
            jxb_id: "a".into(),
            kch_id: "a".into(),
            ..Default::default()
        };
        // Empty task.
        s.lists[0].tasks = vec![CourseTask {
            course: None,
            drops: vec![],
        }];
        assert!(s.validate(false, 0).is_err());
        // Same course as select and as drop → duplicate.
        s.lists[0].tasks = vec![
            CourseTask {
                course: Some(course.clone()),
                drops: vec![],
            },
            CourseTask {
                course: None,
                drops: vec![course.clone()],
            },
        ];
        assert!(s.validate(false, 0).is_err());
        // Two drops of the same course → duplicate.
        s.lists[0].tasks = vec![CourseTask {
            course: None,
            drops: vec![course.clone(), course.clone()],
        }];
        assert!(s.validate(false, 0).is_err());
        // Distinct courses pass.
        let other = Course {
            jxbmc: "(2026-2027-1)-A-02".into(),
            ..course.clone()
        };
        s.lists[0].tasks = vec![CourseTask {
            course: Some(other),
            drops: vec![course],
        }];
        assert!(s.validate(false, 0).is_ok());
    }
    #[test]
    fn legacy_cancel_tasks_migrate_to_pure_drops() {
        let s: Settings = serde_json::from_str(
            r#"{"year":2026,"term":1,"interval_ms":60000,"jitter_ms":0,"jitter_seed":1919810,
               "start_at":"","relogin_before_secs":0,"active_list":0,
               "lists":[{"name":"默认清单","mode":"once","tasks":[
                 {"course":{"jxbmc":"(2026-2027-1)-A-01","jxb_id":"a","kch_id":"a"},"action":"cancel"},
                 {"course":{"jxbmc":"(2026-2027-1)-A-02","jxb_id":"b","kch_id":"b"},"action":"select"}
               ]}]}"#,
        )
        .unwrap();
        assert_eq!(s.lists[0].tasks[0].course, None);
        assert_eq!(s.lists[0].tasks[0].drops.len(), 1);
        assert_eq!(s.lists[0].tasks[0].drops[0].jxbmc, "(2026-2027-1)-A-01");
        assert_eq!(
            s.lists[0].tasks[1].course.as_ref().unwrap().jxbmc,
            "(2026-2027-1)-A-02"
        );
    }
    #[test]
    fn multiple_lists_validate_independently() {
        let mut s = Settings::default();
        let course = Course {
            jxbmc: "(2026-2027-1)-A-01".into(),
            jxb_id: "a".into(),
            kch_id: "a".into(),
            ..Default::default()
        };
        s.lists[0].tasks.push(CourseTask {
            course: Some(course.clone()),
            drops: vec![],
        });
        s.lists.push(TaskList {
            name: "抢课清单".into(),
            mode: Mode::Once,
            tasks: vec![],
        });
        assert!(s.validate(true, 0).is_ok());
        assert!(s.validate(true, 1).is_err()); // second list empty when starting
        assert!(s.validate(true, 2).is_err()); // out of range
    }
    #[test]
    fn stored_credentials_map_and_normalize_order() {
        let mut c = StoredCredentials::default();
        assert_eq!(
            c.normalized_order(),
            vec![
                LoginMethod::Cas,
                LoginMethod::Newjw,
                LoginMethod::Qrcode,
                LoginMethod::Cookie
            ]
        );
        // Duplicates removed, missing methods appended.
        c.order = vec![LoginMethod::Cookie, LoginMethod::Cas, LoginMethod::Cas];
        assert_eq!(
            c.normalized_order(),
            vec![
                LoginMethod::Cookie,
                LoginMethod::Cas,
                LoginMethod::Newjw,
                LoginMethod::Qrcode
            ]
        );
        // Only fully-saved pairs map to request credentials.
        c.cas_username = "u".into();
        c.cas_password = "p".into();
        let creds = c.as_credentials(LoginMethod::Cas).unwrap();
        assert_eq!(creds.username, "u");
        assert_eq!(creds.password, "p");
        assert!(c.as_credentials(LoginMethod::Newjw).is_none());
        assert!(c.as_credentials(LoginMethod::Qrcode).is_none());
        // update_from stores a used method back.
        let mut c2 = StoredCredentials::default();
        c2.update_from(&Credentials {
            method: "cookie".into(),
            username: String::new(),
            password: String::new(),
            session_id: "s".into(),
            route: "r".into(),
        });
        assert_eq!(c2.session_id, "s");
        assert_eq!(c2.route, "r");
    }
}
