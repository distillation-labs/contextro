//! Async update check — fetches the latest GitHub release and prints a notice
//! to stderr if a newer version is available. Never blocks startup.
//!
//! Suppressed by setting `CTX_NO_UPDATE_CHECK=1`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const API_URL: &str = "https://api.github.com/repos/distillation-labs/contextro/releases/latest";
/// Cache file lives next to the index store; checked at most once per day.
const CACHE_TTL_SECS: u64 = 86_400;

pub fn spawn() {
    if std::env::var("CTX_NO_UPDATE_CHECK").is_ok() {
        return;
    }
    tokio::spawn(async {
        if let Some(latest) = fetch_latest().await {
            if is_newer(&latest, CURRENT) {
                eprintln!(
                    "\n  contextro {} → {} available.\n  Update: npm install -g contextro  |  curl -fsSL https://install.contextro.dev | sh\n",
                    CURRENT, latest
                );
            }
        }
    });
}

/// Returns the latest version string, using a daily on-disk cache to avoid
/// hammering the API on every startup.
async fn fetch_latest() -> Option<String> {
    let cache_path = cache_path();

    // Return cached value if fresh enough.
    if let Some(cached) = read_cache(&cache_path) {
        return Some(cached);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(format!("contextro/{}", CURRENT))
        .build()
        .ok()?;

    let resp = client.get(API_URL).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    let version = tag.trim_start_matches('v').to_string();

    write_cache(&cache_path, &version);
    Some(version)
}

fn cache_path() -> std::path::PathBuf {
    let base = std::env::var("CTX_STORAGE_DIR").unwrap_or_else(|_| {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".contextro")
            .to_string_lossy()
            .into_owned()
    });
    std::path::Path::new(&base).join(".update_check")
}

fn read_cache(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    let ts: u64 = lines.next()?.parse().ok()?;
    let version = lines.next()?.to_string();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    if now.saturating_sub(ts) < CACHE_TTL_SECS {
        Some(version)
    } else {
        None
    }
}

fn write_cache(path: &std::path::Path, version: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(path, format!("{}\n{}", now, version));
}

/// Compares two semver strings. Returns true if `latest` > `current`.
/// Falls back to string comparison if parsing fails.
fn is_newer(latest: &str, current: &str) -> bool {
    fn parse(v: &str) -> Option<(u64, u64, u64)> {
        let mut parts = v.trim_start_matches('v').splitn(3, '.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next().and_then(|p| p.parse().ok()).unwrap_or(0),
        ))
    }
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => latest != current,
    }
}
