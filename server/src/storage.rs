use crate::model::{self, Settings};
use serde::Serialize;
use std::path::PathBuf;

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

pub(crate) fn save_json<T: Serialize>(file: &str, value: &T) -> Result<(), String> {
    let path = data_path(file)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "无法编码配置")?;
    std::fs::write(path, bytes).map_err(|_| "无法保存本地数据".into())
}

pub(crate) fn load_settings_file() -> Result<Settings, String> {
    let path = data_path("settings.json")?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取配置")?)
        .map_err(|_| "保存的配置已损坏".into())
}

pub(crate) fn load_stored_credentials() -> Result<model::StoredCredentials, String> {
    let path = data_path("credentials.json")?;
    if !path.exists() {
        return Ok(model::StoredCredentials::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取登录凭证")?)
        .map_err(|_| "保存的登录凭证已损坏，请在偏好设置中清除后重新填写".into())
}

pub(crate) fn load_ua_config() -> Result<model::UaConfig, String> {
    let path = data_path("ua.json")?;
    if !path.exists() {
        return Ok(model::UaConfig::default());
    }
    let mut config: model::UaConfig =
        serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取 User-Agent 配置")?)
            .map_err(|_| "User-Agent 配置已损坏，请在设置中重置".to_string())?;
    config.normalize();
    Ok(config)
}
