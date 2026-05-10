//! Session event tracker for context continuity.

use std::collections::VecDeque;
use std::time::Instant;

use parking_lot::Mutex;

/// A single session event.
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub event_type: String,
    pub summary: String,
    pub timestamp: Instant,
}

/// Tracks session events for context continuity.
pub struct SessionTracker {
    events: Mutex<VecDeque<SessionEvent>>,
    max_events: usize,
}

impl SessionTracker {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            max_events,
        }
    }

    pub fn track(&self, event_type: &str, summary: &str) {
        let mut events = self.events.lock();
        if events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(SessionEvent {
            event_type: event_type.into(),
            summary: summary.into(),
            timestamp: Instant::now(),
        });
    }

    pub fn recent_events(&self, limit: usize) -> Vec<SessionEvent> {
        let events = self.events.lock();
        events.iter().rev().take(limit).cloned().collect()
    }
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new(100)
    }
}
