use crate::model::{self, Settings};
use serde::Serialize;
use std::{
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(crate) fn data_path(file: &str) -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("HDU_DATA_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|_| "无法创建数据目录")?;
        return Ok(dir.join(file));
    }
    let exe = std::env::current_exe().map_err(|_| "无法确定程序位置")?;
    let base = exe.parent().ok_or("无法确定程序目录")?;
    let dir = base.join("data");
    std::fs::create_dir_all(&dir).map_err(|_| {
        "无法创建数据目录（程序目录下的 data 文件夹），请把程序放到可写的文件夹中运行"
    })?;
    Ok(dir.join(file))
}

/// One-time migration: copy data from the old per-user app data directory
/// into the portable `data` folder when the portable copy does not exist yet.
pub(crate) fn migrate_portable_data() {
    let Ok(portable_file) = data_path("settings.json") else {
        return;
    };
    let Some(portable) = portable_file.parent().map(|p| p.to_path_buf()) else {
        return;
    };
    let Some(legacy) = directories::ProjectDirs::from("cn", "hducourse", "studio")
        .map(|p| p.data_dir().to_path_buf())
    else {
        return;
    };
    if !legacy.exists() {
        return;
    }
    for file in [
        "settings.json",
        "courses.json",
        "credentials.json",
        "ua.json",
    ] {
        let src = legacy.join(file);
        let dst = portable.join(file);
        if !dst.exists() && src.exists() {
            if let Ok(bytes) = std::fs::read(&src) {
                let _ = std::fs::write(&dst, bytes);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Storage {
    root: PathBuf,
    writes: Arc<Mutex<()>>,
}

impl Storage {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            writes: Arc::default(),
        }
    }

    pub(crate) fn path(&self, file: &str) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&self.root).map_err(|_| "无法创建账号数据目录")?;
        Ok(self.root.join(file))
    }

    pub(crate) fn save_json<T: Serialize>(&self, file: &str, value: &T) -> Result<(), String> {
        let _guard = self.writes.lock().unwrap();
        let path = self.path(file)?;
        let temporary = self
            .root
            .join(format!(".write-{:016x}.tmp", rand::random::<u64>()));
        let bytes = serde_json::to_vec_pretty(value).map_err(|_| "无法编码配置")?;
        let mut created = false;
        let result = (|| -> std::io::Result<()> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = options.open(&temporary)?;
            created = true;
            output.write_all(&bytes)?;
            output.sync_all()?;
            drop(output);
            std::fs::rename(&temporary, path)
        })();
        if result.is_err() && created {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|_| "无法保存账号数据".into())
    }

    fn load<T: serde::de::DeserializeOwned + Default>(&self, file: &str) -> Result<T, String> {
        let path = self.path(file)?;
        match std::fs::read(path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|_| format!("账号文件 {file} 已损坏"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(_) => Err(format!("无法读取账号文件 {file}")),
        }
    }

    pub(crate) fn load_settings_file(&self) -> Result<Settings, String> {
        self.load("settings.json")
    }
    pub(crate) fn load_stored_credentials(&self) -> Result<model::StoredCredentials, String> {
        self.load("credentials.json")
    }
    pub(crate) fn load_ua_config(&self) -> Result<model::UaConfig, String> {
        let mut value: model::UaConfig = self.load("ua.json")?;
        value.normalize();
        Ok(value)
    }
}
