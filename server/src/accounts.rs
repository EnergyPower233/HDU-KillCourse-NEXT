#[cfg(test)]
mod tests;

use crate::{state::AppState, storage::Storage};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub(crate) const DEFAULT_ACCOUNT: &str = "default";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Serialize, Deserialize)]
struct Index {
    version: u32,
    accounts: Vec<Profile>,
}

#[derive(Serialize)]
pub(crate) struct AccountSummary {
    #[serde(flatten)]
    profile: Profile,
    running: bool,
    logged_in: bool,
    authenticating: bool,
    log_error: Option<String>,
}

struct Entry {
    profile: Profile,
    state: AppState,
}

#[derive(Clone)]
pub(crate) struct Accounts {
    entries: Arc<Mutex<Vec<Entry>>>,
    store: Storage,
    shutdown: watch::Sender<bool>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn checked_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("账号名称需为 1～60 个字符".into());
    }
    Ok(name.into())
}

impl Accounts {
    pub(crate) fn load(root: PathBuf, shutdown: watch::Sender<bool>) -> Result<Self, String> {
        let store = Storage::new(root.clone());
        let path = store.path("accounts.json")?;
        let index: Index = if path.exists() {
            serde_json::from_slice(&std::fs::read(path).map_err(|_| "无法读取账号列表")?)
                .map_err(|_| "账号列表已损坏")?
        } else {
            let index = Index {
                version: 1,
                accounts: vec![Profile {
                    id: DEFAULT_ACCOUNT.into(),
                    name: "默认账号".into(),
                }],
            };
            store.save_json("accounts.json", &index)?;
            index
        };
        let mut ids = HashSet::new();
        if index.version != 1
            || index.accounts.is_empty()
            || index.accounts.len() > 64
            || index.accounts.iter().any(|p| {
                !valid_id(&p.id) || checked_name(&p.name).is_err() || !ids.insert(p.id.clone())
            })
            || !ids.contains(DEFAULT_ACCOUNT)
        {
            return Err("账号列表的版本或内容无效".into());
        }
        let entries = index
            .accounts
            .into_iter()
            .map(|profile| {
                let account_store = if profile.id == DEFAULT_ACCOUNT {
                    store.clone()
                } else {
                    Storage::new(root.join("accounts").join(&profile.id))
                };
                Entry {
                    profile,
                    state: AppState::for_store(account_store, shutdown.clone()),
                }
            })
            .collect();
        Ok(Self {
            entries: Arc::new(Mutex::new(entries)),
            store,
            shutdown,
        })
    }

    #[cfg(test)]
    pub(crate) fn single(state: AppState) -> Self {
        Self {
            store: state.store.clone(),
            shutdown: state.shutdown.clone(),
            entries: Arc::new(Mutex::new(vec![Entry {
                profile: Profile {
                    id: DEFAULT_ACCOUNT.into(),
                    name: "默认账号".into(),
                },
                state,
            }])),
        }
    }

    pub(crate) fn get(&self, id: &str) -> Result<AppState, String> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.profile.id == id)
            .map(|e| e.state.clone())
            .ok_or_else(|| "账号不存在，请刷新账号列表".into())
    }

    pub(crate) fn states(&self) -> Vec<(Profile, AppState)> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|e| (e.profile.clone(), e.state.clone()))
            .collect()
    }

    pub(crate) fn list(&self) -> Vec<AccountSummary> {
        self.states()
            .into_iter()
            .map(|(profile, state)| {
                let inner = state.lock();
                AccountSummary {
                    profile,
                    running: inner.cancel.is_some(),
                    logged_in: inner.client.is_some(),
                    authenticating: inner.authenticating,
                    log_error: inner.log_error.clone(),
                }
            })
            .collect()
    }

    pub(crate) fn create(&self, name: &str, copy_from: Option<&str>) -> Result<Profile, String> {
        let name = checked_name(name)?;
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= 64 {
            return Err("最多保存 64 个账号".into());
        }
        if entries.iter().any(|e| e.profile.name == name) {
            return Err("账号名称已存在".into());
        }
        let settings = copy_from
            .map(|id| {
                entries
                    .iter()
                    .find(|e| e.profile.id == id)
                    .ok_or("复制来源账号不存在")?
                    .state
                    .store
                    .load_settings_file()
            })
            .transpose()?;
        let id = loop {
            let id = format!("account-{:016x}", rand::random::<u64>());
            if !self.store.path(&format!("accounts/{id}"))?.exists() {
                break id;
            }
        };
        let profile = Profile { id, name };
        let store = Storage::new(self.store.path(&format!("accounts/{}", profile.id))?);
        if let Some(settings) = settings {
            store.save_json("settings.json", &settings)?;
        }
        let mut profiles: Vec<_> = entries.iter().map(|e| e.profile.clone()).collect();
        profiles.push(profile.clone());
        self.store.save_json(
            "accounts.json",
            &Index {
                version: 1,
                accounts: profiles,
            },
        )?;
        entries.push(Entry {
            profile: profile.clone(),
            state: AppState::for_store(store, self.shutdown.clone()),
        });
        Ok(profile)
    }

    pub(crate) fn rename(&self, id: &str, name: &str) -> Result<(), String> {
        let name = checked_name(name)?;
        let mut entries = self.entries.lock().unwrap();
        if entries
            .iter()
            .any(|e| e.profile.id != id && e.profile.name == name)
        {
            return Err("账号名称已存在".into());
        }
        let position = entries
            .iter()
            .position(|e| e.profile.id == id)
            .ok_or("账号不存在")?;
        let mut profiles: Vec<_> = entries.iter().map(|e| e.profile.clone()).collect();
        profiles[position].name = name.clone();
        self.store.save_json(
            "accounts.json",
            &Index {
                version: 1,
                accounts: profiles,
            },
        )?;
        entries[position].profile.name = name;
        Ok(())
    }
}
