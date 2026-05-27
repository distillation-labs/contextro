//! SQLite-backed semantic memory store with remember/recall/forget.

use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Common English words unlikely to discriminate between memories.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "is", "it", "in", "on", "at", "to", "for", "of", "or", "and", "how", "does",
    "what", "which", "why", "when", "where", "who", "do", "did", "be", "was", "are", "were",
    "been", "have", "has", "had", "can", "could", "this", "that", "these", "those", "with", "from",
    "by", "as", "but", "not", "work", "works", "use", "used", "using", "get", "set", "will",
    "would", "should", "we", "my", "our", "their", "its", "me", "him", "her", "us", "them",
];

/// Light suffix-stripping to improve recall across word forms ("indexing" → "index").
fn stem_word(word: &str) -> String {
    let w = word.to_lowercase();
    for suffix in &["tion", "ing", "ion", "ers", "ed", "er", "ly", "es", "s"] {
        if w.len() >= suffix.len() + 3 && w.ends_with(suffix) {
            return w[..w.len() - suffix.len()].to_string();
        }
    }
    w
}

fn query_terms(text: &str) -> Vec<String> {
    let raw_words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_lowercase())
        .collect();

    let mut seen = HashSet::new();
    let filtered: Vec<String> = raw_words
        .iter()
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .map(|w| stem_word(w))
        .filter(|s| s.len() >= 2)
        .filter(|s| seen.insert(s.clone()))
        .collect();

    if filtered.is_empty() {
        raw_words
    } else {
        filtered
    }
}

fn content_terms(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(stem_word)
        .filter(|s| s.len() >= 2)
        .collect()
}

use chrono::{Duration, Utc};
use contextro_core::models::{Memory, MemoryTtl, MemoryType};
use contextro_core::ContextroError;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

/// Memory store backed by SQLite.
pub struct MemoryStore {
    conn: Mutex<Connection>,
}

impl MemoryStore {
    pub fn new(db_path: &str) -> Result<Self, ContextroError> {
        let dir = Path::new(db_path).parent();
        if let Some(d) = dir {
            std::fs::create_dir_all(d).ok();
        }
        let conn = Connection::open(db_path)
            .map_err(|e| ContextroError::Memory(format!("Failed to open DB: {}", e)))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                project TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                accessed_at TEXT NOT NULL,
                ttl TEXT NOT NULL DEFAULT 'permanent',
                source TEXT NOT NULL DEFAULT 'user'
            );
            CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
            CREATE INDEX IF NOT EXISTS idx_memories_project ON memories(project);",
        )
        .map_err(|e| ContextroError::Memory(format!("Schema init failed: {}", e)))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory store (for testing).
    pub fn in_memory() -> Result<Self, ContextroError> {
        Self::new(":memory:")
    }

    /// Store a memory. Returns the generated ID.
    pub fn remember(&self, memory: &Memory) -> Result<String, ContextroError> {
        let id = if memory.id.is_empty() {
            Self::generate_id(&memory.content)
        } else {
            memory.id.clone()
        };
        let tags_str = memory.tags.join(",");
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO memories (id, content, memory_type, project, tags, created_at, accessed_at, ttl, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                memory.content,
                memory.memory_type.to_string(),
                memory.project,
                tags_str,
                memory.created_at,
                memory.accessed_at,
                format!("{:?}", memory.ttl).to_lowercase(),
                memory.source,
            ],
        )
        .map_err(|e| ContextroError::Memory(format!("Insert failed: {}", e)))?;
        Ok(id)
    }

    /// Search memories. Query terms are stemmed, stop words removed, then matched
    /// with OR logic (any term can match). Results are re-ranked in Rust by how
    /// many stems appear in the content so the most relevant memory ranks first.
    pub fn recall(
        &self,
        query: &str,
        limit: usize,
        memory_type: Option<&str>,
        tags: Option<&str>,
        project: Option<&str>,
    ) -> Result<Vec<Memory>, ContextroError> {
        let conn = self.conn.lock();

        // Build meaningful stems: strip stop words, apply light suffix stripping
        let stems = query_terms(query);

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        let mut idx = 1usize;

        // OR logic: any stem can match in content or tags.
        let word_clause = if stems.is_empty() {
            String::new()
        } else {
            let clauses: Vec<String> = stems
                .iter()
                .map(|s| {
                    param_values.push(Box::new(format!("%{}%", s)));
                    let content_idx = idx;
                    idx += 1;
                    param_values.push(Box::new(format!("%{}%", s)));
                    let tags_idx = idx;
                    idx += 1;
                    format!(
                        "(LOWER(content) LIKE ?{} OR LOWER(tags) LIKE ?{})",
                        content_idx, tags_idx
                    )
                })
                .collect();
            format!("({})", clauses.join(" OR "))
        };

        let mut sql = String::from(
            "SELECT id, content, memory_type, project, tags, created_at, accessed_at, ttl, source FROM memories WHERE 1=1",
        );
        if !word_clause.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&word_clause);
        }

        if let Some(mt) = memory_type {
            sql.push_str(&format!(" AND memory_type = ?{}", idx));
            param_values.push(Box::new(mt.to_string()));
            idx += 1;
        }
        if let Some(t) = tags {
            sql.push_str(&format!(" AND tags LIKE ?{}", idx));
            param_values.push(Box::new(format!("%{}%", t)));
            idx += 1;
        }
        if let Some(p) = project {
            sql.push_str(&format!(" AND project = ?{}", idx));
            param_values.push(Box::new(p.to_string()));
            let _ = idx;
        }
        // Keep recency as the initial DB order for stable tie-breaking, but do not
        // truncate candidates before Rust re-ranking or older relevant memories can
        // be dropped behind newer lexical distractors.
        sql.push_str(" ORDER BY created_at DESC");

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ContextroError::Memory(format!("Query failed: {}", e)))?;
        let rows = stmt
            .query_map(params_ref.as_slice(), |row| {
                Ok(Memory {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    memory_type: parse_memory_type(&row.get::<_, String>(2)?),
                    project: row.get(3)?,
                    tags: row
                        .get::<_, String>(4)?
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect(),
                    created_at: row.get(5)?,
                    accessed_at: row.get(6)?,
                    ttl: parse_ttl(&row.get::<_, String>(7)?),
                    source: row.get(8)?,
                })
            })
            .map_err(|e| ContextroError::Memory(format!("Query map failed: {}", e)))?;

        let candidates: Vec<Memory> = rows.flatten().collect();

        let primary_stem = stems.first().map(String::as_str);

        let term_frequencies: HashMap<&str, usize> = stems
            .iter()
            .map(|stem| {
                let frequency = candidates
                    .iter()
                    .filter(|memory| {
                        let terms = content_terms(&memory.content)
                            .into_iter()
                            .chain(memory.tags.iter().flat_map(|tag| content_terms(tag)))
                            .collect::<HashSet<_>>();
                        terms.contains(stem.as_str())
                    })
                    .count();
                (stem.as_str(), frequency)
            })
            .collect();
        let max_frequency = term_frequencies.values().copied().max().unwrap_or(0);
        let has_discriminative_terms = term_frequencies.values().any(|&freq| freq < max_frequency);

        // Re-rank: matching tags carry the strongest structured signal, but broad
        // query terms that hit nearly every candidate should not outrank rarer,
        // more specific matches like bug-tagged memories.
        let mut ranked: Vec<(bool, bool, usize, usize, Memory)> = candidates
            .into_iter()
            .map(|m| {
                let content_match_terms = content_terms(&m.content);
                let tag_match_terms: HashSet<String> =
                    m.tags.iter().flat_map(|tag| content_terms(tag)).collect();

                let primary_tag_hit = primary_stem
                    .map(|stem| tag_match_terms.contains(stem))
                    .unwrap_or(false);
                let primary_content_hit = primary_stem
                    .map(|stem| content_match_terms.contains(stem))
                    .unwrap_or(false);

                let weighted_content_hits = stems
                    .iter()
                    .filter(|stem| content_match_terms.contains(stem.as_str()))
                    .map(|stem| {
                        max_frequency + 1
                            - term_frequencies
                                .get(stem.as_str())
                                .copied()
                                .unwrap_or(max_frequency)
                    })
                    .sum();
                let weighted_tag_hits = stems
                    .iter()
                    .filter(|stem| tag_match_terms.contains(stem.as_str()))
                    .map(|stem| {
                        max_frequency + 1
                            - term_frequencies
                                .get(stem.as_str())
                                .copied()
                                .unwrap_or(max_frequency)
                    })
                    .sum();

                let _ = has_discriminative_terms;

                (
                    primary_tag_hit,
                    primary_content_hit,
                    weighted_tag_hits,
                    weighted_content_hits,
                    m,
                )
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| b.3.cmp(&a.3))
                .then_with(|| b.4.created_at.cmp(&a.4.created_at))
        });

        Ok(ranked
            .into_iter()
            .take(limit)
            .map(|(_, _, _, _, m)| m)
            .collect())
    }

    /// List all unique tags across all memories, sorted alphabetically.
    pub fn list_tags(&self) -> Vec<String> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare("SELECT tags FROM memories WHERE tags != ''") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let mut tag_set = std::collections::HashSet::new();
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                for tag in row.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
                    tag_set.insert(tag.to_string());
                }
            }
        }
        let mut tags: Vec<String> = tag_set.into_iter().collect();
        tags.sort();
        tags
    }

    /// Delete memories by ID, tags, or memory_type.
    pub fn forget(
        &self,
        id: Option<&str>,
        tags: Option<&str>,
        memory_type: Option<&str>,
    ) -> Result<usize, ContextroError> {
        let conn = self.conn.lock();
        if let Some(id) = id {
            let n = conn
                .execute("DELETE FROM memories WHERE id = ?1", params![id])
                .map_err(|e| ContextroError::Memory(format!("Delete failed: {}", e)))?;
            return Ok(n);
        }
        if let Some(t) = tags {
            let like = format!("%{}%", t);
            let n = conn
                .execute("DELETE FROM memories WHERE tags LIKE ?1", params![like])
                .map_err(|e| ContextroError::Memory(format!("Delete failed: {}", e)))?;
            return Ok(n);
        }
        if let Some(mt) = memory_type {
            let n = conn
                .execute("DELETE FROM memories WHERE memory_type = ?1", params![mt])
                .map_err(|e| ContextroError::Memory(format!("Delete failed: {}", e)))?;
            return Ok(n);
        }
        Ok(0)
    }

    /// Expire memories past their TTL.
    pub fn expire_ttl(&self) -> Result<usize, ContextroError> {
        let now = Utc::now();
        let conn = self.conn.lock();
        let mut total = 0;
        for (ttl_name, duration) in [
            ("session", Duration::hours(4)),
            ("day", Duration::days(1)),
            ("week", Duration::weeks(1)),
            ("month", Duration::days(30)),
        ] {
            let cutoff = (now - duration).to_rfc3339();
            let n = conn
                .execute(
                    "DELETE FROM memories WHERE ttl = ?1 AND created_at < ?2",
                    params![ttl_name, cutoff],
                )
                .unwrap_or(0);
            total += n;
        }
        Ok(total)
    }

    /// Count total memories.
    pub fn count(&self) -> usize {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM memories", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap_or(0)
    }

    fn generate_id(content: &str) -> String {
        let hash = Sha256::digest(content.as_bytes());
        format!("mem_{}", &hex::encode(&hash[..4]))
    }
}

fn parse_memory_type(s: &str) -> MemoryType {
    match s {
        "conversation" => MemoryType::Conversation,
        "status" => MemoryType::Status,
        "decision" => MemoryType::Decision,
        "preference" => MemoryType::Preference,
        "doc" => MemoryType::Doc,
        _ => MemoryType::Note,
    }
}

fn parse_ttl(s: &str) -> MemoryTtl {
    match s {
        "session" => MemoryTtl::Session,
        "day" => MemoryTtl::Day,
        "week" => MemoryTtl::Week,
        "month" => MemoryTtl::Month,
        _ => MemoryTtl::Permanent,
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests;
