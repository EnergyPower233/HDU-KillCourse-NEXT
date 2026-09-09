use super::{random::Mt19937, DEFAULT_UA};
use crate::model::UaConfig;
use std::sync::Mutex;

pub(super) fn resolve_ua(config: &UaConfig) -> String {
    match config.mode.as_str() {
        "browser" if !config.browser_ua.trim().is_empty() => config.browser_ua.clone(),
        "fixed" => build_fixed_ua(
            &config.fixed_os,
            &config.fixed_browser,
            &config.fixed_version,
        ),
        "rotate" => {
            let list: Vec<&str> = config
                .rotate_list
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if list.is_empty() {
                return DEFAULT_UA.to_string();
            }
            use rand::Rng;
            let i = rand::thread_rng().gen_range(0..list.len());
            list[i].to_string()
        }
        "generate" => generated_ua(config.generate_seed),
        _ => DEFAULT_UA.to_string(),
    }
}

pub(super) fn build_fixed_ua(os: &str, browser: &str, version: &str) -> String {
    if version.trim().is_empty() {
        return DEFAULT_UA.to_string();
    }
    let platform = |firefox: bool| -> String {
        let base = match os {
            "macos" => "Macintosh; Intel Mac OS X 10.15",
            "linux" => "X11; Linux x86_64",
            _ => "Windows NT 10.0; Win64; x64",
        };
        if firefox {
            format!("{base}; rv:{version}")
        } else if os == "macos" {
            "Macintosh; Intel Mac OS X 10_15_7".to_string()
        } else {
            base.to_string()
        }
    };
    match browser {
        "firefox" => format!(
            "Mozilla/5.0 ({}) Gecko/20100101 Firefox/{version}",
            platform(true)
        ),
        "safari" => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version} Safari/605.1.15"
        ),
        "edge" => format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version} Safari/537.36 Edg/{version}",
            platform(false)
        ),
        "opera" => format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.124 Safari/537.36 OPR/{version}",
            platform(false)
        ),
        _ => format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version} Safari/537.36",
            platform(false)
        ),
    }
}

pub(super) fn generated_ua(seed: u64) -> String {
    static STATE: std::sync::OnceLock<Mutex<(u64, Mt19937)>> = std::sync::OnceLock::new();
    let mut guard = STATE
        .get_or_init(|| Mutex::new((seed, Mt19937::new(seed as u32))))
        .lock()
        .unwrap();
    if guard.0 != seed {
        *guard = (seed, Mt19937::new(seed as u32));
    }
    generate_ua(&mut guard.1)
}

pub(super) fn generate_ua(rng: &mut Mt19937) -> String {
    let chrome_major = 120 + (rng.next_u32() % 22); // 120..=141
    let build = 1000 + (rng.next_u32() % 9000); // 4-digit build
    let patch = rng.next_u32() % 1000;
    let firefox_major = 115 + (rng.next_u32() % 27); // 115..=141
    let safari_major = 16 + (rng.next_u32() % 3); // 16..=18
    let safari_minor = rng.next_u32() % 7; // 0..=6
    let family = rng.next_u32() % 8;
    let chrome_ver = |maj: u32| format!("Chrome/{maj}.0.{build}.{patch} Safari/537.36");
    match family {
        0 => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) {}",
            chrome_ver(chrome_major)
        ),
        1 => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_major}.0.{build}.{patch} Safari/537.36 Edg/{chrome_major}.0.{build}.{patch}"
        ),
        2 => format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:{firefox_major}.0) Gecko/20100101 Firefox/{firefox_major}.0"
        ),
        3 => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{safari_major}.{safari_minor} Safari/605.1.15"
        ),
        4 => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) {}",
            chrome_ver(chrome_major)
        ),
        5 => format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:{firefox_major}.0) Gecko/20100101 Firefox/{firefox_major}.0"
        ),
        _ => format!(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) {}",
            chrome_ver(chrome_major)
        ),
    }
}
