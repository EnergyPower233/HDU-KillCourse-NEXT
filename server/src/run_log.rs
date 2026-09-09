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
    fn unwritable_log_location_is_reported() {
        let path = std::env::temp_dir().join(format!("hdu-log-file-{:016x}", rand::random::<u64>()));
        fs::write(&path, "occupied").unwrap();
        assert!(RunLog::create(&path, "test").is_err());
        fs::remove_file(path).unwrap();
    }
}
