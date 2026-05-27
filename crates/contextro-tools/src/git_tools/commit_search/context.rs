use std::collections::HashSet;

use crate::git_tools::{token_overlap_score_lower, tokenize};

use super::{CachedCommitRecord, CommitDiffContext};

pub(super) fn commit_diff_context_score(
    repo: &git2::Repository,
    record: &CachedCommitRecord,
    query_lower: &str,
    query_tokens: &[String],
) -> f64 {
    if query_tokens.len() < 2 {
        return 0.0;
    }

    let subject = record.message.lines().next().unwrap_or("");
    if !commit_subject_needs_diff_context(subject) {
        return 0.0;
    }

    let context = record
        .diff_context
        .get_or_init(|| collect_commit_diff_context(repo, record.oid).unwrap_or_default());
    token_overlap_score_lower(
        query_lower,
        query_tokens,
        &context.text_lower,
        &context.tokens,
    )
}

fn collect_commit_diff_context(
    repo: &git2::Repository,
    oid: git2::Oid,
) -> Option<CommitDiffContext> {
    let commit = repo.find_commit(oid).ok()?;

    let Ok(tree) = commit.tree() else {
        return None;
    };
    let parent_tree = commit.parent(0).ok().and_then(|parent| parent.tree().ok());
    let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) else {
        return None;
    };

    let collector = std::cell::RefCell::new(CommitDiffContextCollector::default());
    let mut file_cb = |_delta: git2::DiffDelta<'_>, _progress: f32| true;
    let mut line_cb = |_delta: git2::DiffDelta<'_>,
                       _hunk: Option<git2::DiffHunk<'_>>,
                       line: git2::DiffLine<'_>| {
        if !matches!(line.origin(), '+' | '-') {
            return true;
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            collector.borrow_mut().push_text(content);
        }
        true
    };

    let _ = diff.foreach(&mut file_cb, None, None, Some(&mut line_cb));

    let collector = collector.into_inner();
    if collector.tokens.is_empty() {
        None
    } else {
        Some(CommitDiffContext {
            text_lower: collector.tokens.join(" "),
            tokens: collector.tokens,
        })
    }
}

fn commit_subject_needs_diff_context(subject: &str) -> bool {
    let lowered = subject.trim().to_ascii_lowercase();
    lowered.starts_with("update ")
        || lowered.starts_with("docs update ")
        || lowered.starts_with("documentation update ")
}

#[derive(Default)]
struct CommitDiffContextCollector {
    seen: HashSet<String>,
    tokens: Vec<String>,
}

impl CommitDiffContextCollector {
    const TOKEN_LIMIT: usize = 48;

    fn push_text(&mut self, text: &str) {
        if self.tokens.len() >= Self::TOKEN_LIMIT {
            return;
        }

        for token in tokenize(text) {
            if is_commit_diff_context_stopword(&token) || !self.seen.insert(token.clone()) {
                continue;
            }
            self.tokens.push(token);
            if self.tokens.len() >= Self::TOKEN_LIMIT {
                break;
            }
        }
    }
}

fn is_commit_diff_context_stopword(token: &str) -> bool {
    matches!(
        token,
        "else"
            | "enum"
            | "false"
            | "from"
            | "impl"
            | "into"
            | "none"
            | "self"
            | "some"
            | "struct"
            | "true"
            | "use"
    )
}
