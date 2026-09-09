use crate::model::{Course, CourseTask, Mode, Settings, TaskList};
use serde::Deserialize;

pub const MAX_TASK_FILE_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskFile {
    version: u32,
    year: u32,
    term: u8,
    lists: Vec<ImportedList>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedList {
    name: String,
    mode: Mode,
    tasks: Vec<ImportedTask>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedTask {
    course: Option<ImportedCourse>,
    drops: Vec<ImportedCourse>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedCourse {
    jxbmc: String,
    kch_id: String,
    jxb_id: String,
    #[serde(default)]
    kcmc: String,
    #[serde(default)]
    kklxmc: String,
    #[serde(default)]
    sksj: String,
    #[serde(default)]
    jxbzc: String,
    #[serde(default)]
    jzgxx: String,
    #[serde(default)]
    jxdd: String,
}

impl ImportedCourse {
    fn into_course(self) -> Result<Course, String> {
        if [&self.jxbmc, &self.kch_id, &self.jxb_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err("课程缺少教学班编号、课程内部编号或教学班内部编号".into());
        }
        Ok(Course {
            jxbmc: self.jxbmc,
            kch_id: self.kch_id,
            jxb_id: self.jxb_id,
            kcmc: self.kcmc,
            kklxmc: self.kklxmc,
            sksj: self.sksj,
            jxbzc: self.jxbzc,
            jzgxx: self.jzgxx,
            jxdd: self.jxdd,
        })
    }
}

/// Validate the entire file before returning any lists. Import never starts tasks.
pub fn parse_task_file(text: &str, settings: &Settings) -> Result<Vec<TaskList>, String> {
    if text.len() > MAX_TASK_FILE_BYTES {
        return Err("任务清单文件不能超过 1 MB".into());
    }
    let file: TaskFile = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("任务清单 JSON 格式错误：{error}"))?;
    if file.version != 1 {
        return Err("不支持的任务清单版本，目前仅支持 version: 1".into());
    }
    if file.year != settings.year || file.term != settings.term {
        return Err("任务清单学期与当前设置不一致，请先核对偏好设置".into());
    }
    if file.lists.is_empty() || file.lists.len() > 100 {
        return Err("文件应包含 1～100 个清单".into());
    }
    let mut lists = Vec::new();
    for list in file.lists {
        if list.name.trim().is_empty() || list.name.chars().count() > 100 {
            return Err("清单名称不能为空，且不能超过 100 个字符".into());
        }
        let mut tasks = Vec::new();
        for task in list.tasks {
            tasks.push(CourseTask {
                course: task.course.map(ImportedCourse::into_course).transpose()?,
                drops: task
                    .drops
                    .into_iter()
                    .map(ImportedCourse::into_course)
                    .collect::<Result<_, _>>()?,
            });
        }
        lists.push(TaskList {
            name: list.name,
            mode: list.mode,
            tasks,
        });
    }
    let candidate = Settings {
        lists,
        active_list: 0,
        ..settings.clone()
    };
    candidate.validate(false, 0)?;
    Ok(candidate.lists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn fixture() -> Value {
        json!({"version": 1, "year": 2026, "term": 1, "lists": [{
            "name": "导入清单", "mode": "once", "tasks": [{
                "course": {"jxbmc": "(2026-2027-1)-a", "jxb_id": "a", "kch_id": "k"},
                "drops": [{"jxbmc": "(2026-2027-1)-b", "jxb_id": "b", "kch_id": "k"}]
            }]
        }]})
    }

    #[test]
    fn imports_paired_and_drop_only_tasks_without_changing_settings() {
        let settings = Settings::default();
        let original = serde_json::to_value(&settings).unwrap();
        let mut file = fixture();
        let lists = parse_task_file(&file.to_string(), &settings).unwrap();
        assert_eq!(lists[0].tasks[0].drops.len(), 1);
        assert_eq!(lists[0].tasks[0].course.as_ref().unwrap().jxb_id, "a");
        file["lists"][0]["tasks"][0]["course"] = Value::Null;
        assert!(
            parse_task_file(&file.to_string(), &settings).unwrap()[0].tasks[0]
                .course
                .is_none()
        );
        assert_eq!(serde_json::to_value(&settings).unwrap(), original);
    }

    #[test]
    fn rejects_wrong_term_unknown_fields_bad_ids_and_watch_drops() {
        for (pointer, value) in [
            ("/version", json!(2)),
            ("/year", json!(2025)),
            ("/lists/0/mode", json!("watch")),
            ("/lists/0/name", json!(" ")),
            ("/lists/0/tasks/0/course/jxb_id", json!(" ")),
            ("/lists/0/tasks/0/course/jxbmc", json!("(2025-2026-1)-a")),
        ] {
            let mut file = fixture();
            *file.pointer_mut(pointer).unwrap() = value;
            assert!(
                parse_task_file(&file.to_string(), &Settings::default()).is_err(),
                "{pointer}"
            );
        }
        let mut file = fixture();
        file["credentials"] = json!({});
        assert!(parse_task_file(&file.to_string(), &Settings::default()).is_err());
        assert!(parse_task_file("{}", &Settings::default()).is_err());
        assert!(
            parse_task_file(&" ".repeat(MAX_TASK_FILE_BYTES + 1), &Settings::default()).is_err()
        );
    }

    #[test]
    fn rejects_duplicate_courses_and_accepts_multiple_watch_lists() {
        let mut file = fixture();
        file["lists"][0]["tasks"][0]["drops"][0] = file["lists"][0]["tasks"][0]["course"].clone();
        assert!(parse_task_file(&file.to_string(), &Settings::default()).is_err());
        file["lists"][0]["tasks"][0]["drops"] = json!([]);
        file["lists"][0]["mode"] = json!("watch");
        let another = file["lists"][0].clone();
        file["lists"].as_array_mut().unwrap().push(another);
        assert_eq!(
            parse_task_file(&file.to_string(), &Settings::default())
                .unwrap()
                .len(),
            2
        );
    }
}
