use crate::model::Progress;
use serde::{Deserialize, Serialize};
use std::{fs::{self, File, OpenOptions}, io::Write, path::Path};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RunInfo {
    pub id: String,
    pub list_name: String,
    pub started_at: String,
}

pub struct RunLog {
    pub info: RunInfo,
    file: File,
}

impl RunLog {
    pub fn create(directory: &Path, list_name: &str) -> Result<Self, String> {
        fs::create_dir_all(directory).map_err(|e| format!("无法创建运行日志目录：{e}"))?;
        let now = chrono::Utc::now();
        let info = RunInfo {
            id: format!("{}-{:016x}", now.format("%Y%m%dT%H%M%S%3fZ"), rand::random::<u64>()),
            list_name: list_name.into(),
            started_at: now.to_rfc3339(),
        };
        let mut file = OpenOptions::new().write(true).create_new(true)
            .open(directory.join(format!("{}.jsonl", info.id)))
            .map_err(|e| format!("无法创建本次运行日志：{e}"))?;
        let mut header = serde_json::to_vec(&info).map_err(|e| e.to_string())?;
        header.push(b'\n');
        file.write_all(&header).and_then(|_| file.sync_data())
            .map_err(|e| format!("无法保存运行日志信息：{e}"))?;
        Ok(Self { info, file })
    }

    pub fn append(&mut self, event: &Progress) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(event).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        self.file.write_all(&bytes).and_then(|_| self.file.sync_data())
            .map_err(|e| format!("运行日志写入失败：{e}"))
    }
}

#[derive(Serialize)]
pub struct RunPage {
    pub run: RunInfo,
    pub events: Vec<Progress>,
    pub next_before: Option<usize>,
    pub warning: Option<String>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub fn list_runs(directory: &Path) -> Result<Vec<RunInfo>, String> {
    use std::io::BufRead;
    if !directory.exists() { return Ok(vec![]); }
    let mut runs = Vec::new();
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") { continue; }
        let mut header = String::new();
        std::io::BufReader::new(File::open(&path).map_err(|e| e.to_string())?)
            .read_line(&mut header).map_err(|e| e.to_string())?;
        let info: RunInfo = serde_json::from_str(&header).map_err(|_| "运行日志头损坏".to_string())?;
        if !valid_id(&info.id) || path.file_stem().and_then(|s| s.to_str()) != Some(&info.id) {
            return Err("运行日志编号与文件不符".into());
        }
        runs.push(info);
    }
    runs.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(runs)
}

pub fn read_run(directory: &Path, id: &str, before: Option<usize>) -> Result<RunPage, String> {
    use std::io::BufRead;
    if !valid_id(id) { return Err("运行编号不合法".into()); }
    let file = File::open(directory.join(format!("{id}.jsonl"))).map_err(|_| "找不到运行日志".to_string())?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let run: RunInfo = serde_json::from_str(&line).map_err(|_| "运行日志头损坏".to_string())?;
    if run.id != id { return Err("运行日志编号与文件不符".into()); }
    let mut events = std::collections::VecDeque::new();
    let mut count = 0usize;
    let mut warning = None;
    while before.is_none_or(|limit| count < limit) {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 { break; }
        if !line.ends_with('\n') {
            warning = Some("最后一条记录尚未完整写入，已显示此前的完整日志".into());
            break;
        }
        let event: Progress = serde_json::from_str(&line).map_err(|_| format!("第 {} 条日志损坏", count + 1))?;
        events.push_back(event);
        count += 1;
        if events.len() > 500 { events.pop_front(); }
    }
    let first = count - events.len();
    Ok(RunPage { run, events: events.into(), next_before: (first > 0).then_some(first), warning })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_run_keeps_its_own_events_after_reopening() {
        let directory = std::env::temp_dir().join(format!("hdu-log-test-{:016x}", rand::random::<u64>()));
        let mut first = RunLog::create(&directory, "清单一").unwrap();
        let first_id = first.info.id.clone();
        for n in 0..1005 {
            first.append(&Progress::new("course", "waiting", &n.to_string())).unwrap();
        }
        drop(first);
        let second = RunLog::create(&directory, "清单二").unwrap();
        assert_ne!(first_id, second.info.id);
        drop(second);
        let text = fs::read_to_string(directory.join(format!("{first_id}.jsonl"))).unwrap();
        assert_eq!(text.lines().count(), 1006);
        let last: Progress = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last.message, "1004");
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn history_pages_survive_restart_and_ignore_partial_tail() {
        let directory = std::env::temp_dir().join(format!("hdu-log-pages-{:016x}", rand::random::<u64>()));
        let mut log = RunLog::create(&directory, "历史清单").unwrap();
        let id = log.info.id.clone();
        for n in 0..503 { log.append(&Progress::new("id", "success", &n.to_string())).unwrap(); }
        drop(log);
        assert_eq!(list_runs(&directory).unwrap()[0].list_name, "历史清单");
        let latest = read_run(&directory, &id, None).unwrap();
        assert_eq!(latest.events.len(), 500);
        assert_eq!(latest.events[0].message, "3");
        let older = read_run(&directory, &id, latest.next_before).unwrap();
        assert_eq!(older.events.len(), 3);
        assert_eq!(older.events[0].message, "0");
        assert!(older.next_before.is_none());
        let mut file = OpenOptions::new().append(true).open(directory.join(format!("{id}.jsonl"))).unwrap();
        file.write_all(b"{partial").unwrap(); drop(file);
        let partial = read_run(&directory, &id, None).unwrap();
        assert_eq!(partial.events.len(), 500);
        assert!(partial.warning.is_some());
        assert!(read_run(&directory, "../credentials", None).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn unwritable_log_location_is_reported() {
        let path = std::env::temp_dir().join(format!("hdu-log-file-{:016x}", rand::random::<u64>()));
        fs::write(&path, "occupied").unwrap();
        assert!(RunLog::create(&path, "test").is_err());
        fs::remove_file(path).unwrap();
    }
}
