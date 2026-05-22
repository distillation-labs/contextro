//! Session event tracker for context continuity.

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use contextro_config::get_settings;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// A single session event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// Tracks session events for context continuity.
pub struct SessionTracker {
    events: Mutex<VecDeque<SessionEvent>>,
    overflow_appends_since_compaction: AtomicUsize,
    max_events: usize,
    file_path: PathBuf,
}

impl SessionTracker {
    pub fn new(max_events: usize) -> Self {
        let storage_dir = get_settings().read().storage_dir.clone();
        Self::with_path(
            max_events,
            PathBuf::from(storage_dir).join("session-events.json"),
        )
    }

    pub fn with_path<P: Into<PathBuf>>(max_events: usize, file_path: P) -> Self {
        let file_path = file_path.into();
        let mut events = load_events(&file_path);
        trim_events(&mut events, max_events);
        Self {
            events: Mutex::new(events),
            overflow_appends_since_compaction: AtomicUsize::new(0),
            max_events,
            file_path,
        }
    }

    pub fn track(&self, event_type: &str, summary: &str, arguments: Option<serde_json::Value>) {
        let event = SessionEvent {
            event_type: event_type.into(),
            summary: summary.into(),
            timestamp: Utc::now().to_rfc3339(),
            arguments,
        };
        let mut events = self.events.lock();
        let overflowed = events.len() + 1 > self.max_events;
        trim_events(&mut events, self.max_events.saturating_sub(1));
        events.push_back(event.clone());

        let should_compact = if overflowed {
            self.overflow_appends_since_compaction
                .fetch_add(1, Ordering::Relaxed)
                + 1
                >= self.max_events
        } else {
            self.overflow_appends_since_compaction
                .store(0, Ordering::Relaxed);
            false
        };

        if should_compact || self.append_locked(&event).is_err() {
            self.save_locked(&events);
        }
    }

    pub fn recent_events(&self, limit: usize) -> Vec<SessionEvent> {
        let events = self.events.lock();
        events.iter().rev().take(limit).cloned().collect()
    }

    pub fn recent_events_filtered(
        &self,
        limit: usize,
        event_type: Option<&str>,
    ) -> Vec<SessionEvent> {
        let event_type = event_type.map(|value| value.to_ascii_lowercase());
        let events = self.events.lock();
        events
            .iter()
            .rev()
            .filter(|event| {
                event_type
                    .as_ref()
                    .is_none_or(|expected| event.event_type.to_ascii_lowercase() == *expected)
            })
            .take(limit)
            .cloned()
            .collect()
    }

    fn save_locked(&self, events: &VecDeque<SessionEvent>) {
        self.overflow_appends_since_compaction
            .store(0, Ordering::Relaxed);
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = self.file_path.with_extension("json.tmp");
        if let Ok(mut file) = std::fs::File::create(&tmp_path) {
            let mut ok = true;
            for event in events {
                if serde_json::to_writer(&mut file, event).is_err() || writeln!(&mut file).is_err()
                {
                    ok = false;
                    break;
                }
            }
            if ok {
                let _ = std::fs::rename(&tmp_path, &self.file_path);
            }
        }
    }

    fn append_locked(&self, event: &SessionEvent) -> std::io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        serde_json::to_writer(&mut file, event).map_err(std::io::Error::other)?;
        writeln!(&mut file)
    }
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new(100)
    }
}

fn load_events(path: &Path) -> VecDeque<SessionEvent> {
    let Ok(bytes) = std::fs::read(path) else {
        return VecDeque::new();
    };

    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'[') {
        match serde_json::from_slice::<Vec<SessionEvent>>(&bytes) {
            Ok(events) => VecDeque::from(events),
            Err(_) => {
                backup_corrupt_file(path);
                VecDeque::new()
            }
        }
    } else {
        match parse_line_delimited_events(&bytes) {
            Some(events) => events,
            None => {
                backup_corrupt_file(path);
                VecDeque::new()
            }
        }
    }
}

fn parse_line_delimited_events(bytes: &[u8]) -> Option<VecDeque<SessionEvent>> {
    let mut events = VecDeque::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        let event = serde_json::from_slice::<SessionEvent>(line).ok()?;
        events.push_back(event);
    }
    Some(events)
}

fn trim_events(events: &mut VecDeque<SessionEvent>, max_events: usize) {
    while events.len() > max_events {
        events.pop_front();
    }
}

fn backup_corrupt_file(path: &Path) {
    let backup = path.with_extension(format!("corrupt-{}.json", Utc::now().timestamp()));
    let _ = std::fs::rename(path, backup);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-session-{unique}-{name}"))
    }

    #[test]
    fn test_track_and_reload_events() {
        let path = temp_file("events.json");
        let tracker = SessionTracker::with_path(10, &path);
        tracker.track(
            "index",
            "index(path=\"repo\")",
            Some(serde_json::json!({"path":"repo"})),
        );
        tracker.track(
            "search",
            "search(query=\"jwt\")",
            Some(serde_json::json!({"query":"jwt"})),
        );

        let reloaded = SessionTracker::with_path(10, &path);
        let events = reloaded.recent_events(10);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "search");
        assert_eq!(events[0].arguments.as_ref().unwrap()["query"], "jwt");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_recent_events_filtered_applies_type_and_limit() {
        let path = temp_file("events-filtered.json");
        let tracker = SessionTracker::with_path(10, &path);
        tracker.track("index", "index(path=\"repo\")", None);
        tracker.track("search", "search(query=\"jwt\")", None);
        tracker.track("search", "search(query=\"cache\")", None);

        let events = tracker.recent_events_filtered(1, Some("search"));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "search");
        assert!(events[0].summary.contains("cache"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_events_supports_legacy_json_array_format() {
        let path = temp_file("events-legacy.json");
        let legacy = vec![SessionEvent {
            event_type: "index".into(),
            summary: "index(path=\"repo\")".into(),
            timestamp: Utc::now().to_rfc3339(),
            arguments: Some(serde_json::json!({"path":"repo"})),
        }];
        std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

        let tracker = SessionTracker::with_path(10, &path);
        let events = tracker.recent_events(10);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "index");
        assert_eq!(events[0].arguments.as_ref().unwrap()["path"], "repo");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_track_compacts_periodically_after_overflow() {
        let path = temp_file("events-trimmed.json");
        let tracker = SessionTracker::with_path(2, &path);
        tracker.track("one", "one()", None);
        tracker.track("two", "two()", None);
        tracker.track("three", "three()", None);

        let raw_after_append = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw_after_append.lines().count(), 3);

        tracker.track("four", "four()", None);

        let raw_after_compaction = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw_after_compaction.lines().count(), 2);

        let reloaded = SessionTracker::with_path(10, &path);
        let events = reloaded.recent_events(10);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "four");
        assert_eq!(events[1].event_type, "three");

        let _ = std::fs::remove_file(path);
    }
}
