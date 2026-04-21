//! Project task list at `.ide/tasks.md`.

use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::TaskError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub entries: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub summary: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Done,
    Cancelled,
}

impl TaskList {
    #[must_use]
    pub fn empty() -> Self {
        Self { entries: Vec::new() }
    }

    /// Parse checkbox markdown; lines without a `` `id` `` get a synthetic id on next serialize.
    pub fn parse(markdown: &str) -> Result<Self, TaskError> {
        static LINE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
            Regex::new(r"(?m)^\s*-\s*\[\s*([xX ])\s*\]\s*(?:`([^`]+)`\s+)?(.+?)\s*$")
                .expect("regex")
        });
        let mut entries = Vec::new();
        for cap in LINE.captures_iter(markdown) {
            let done = !cap.get(1).map(|m| m.as_str().trim()).unwrap_or(" ").is_empty();
            let id = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| "pending".into());
            let rest = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("").to_string();
            let (summary, notes) = split_notes(&rest);
            entries.push(Task {
                id,
                summary,
                status: if done { TaskStatus::Done } else { TaskStatus::Open },
                notes,
            });
        }
        Ok(TaskList { entries })
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut s = String::from("# Project tasks\n\n");
        for e in &self.entries {
            let mark = match e.status {
                TaskStatus::Open | TaskStatus::InProgress => ' ',
                TaskStatus::Done => 'x',
                TaskStatus::Cancelled => ' ',
            };
            let mut line = format!("- [{mark}] `{}` {}", e.id, e.summary);
            if let Some(n) = &e.notes {
                line.push_str(" — ");
                line.push_str(n);
            }
            line.push('\n');
            s.push_str(&line);
        }
        s
    }

    pub fn reconcile_ids(&mut self) {
        for e in &mut self.entries {
            if e.id == "pending" {
                e.id = new_task_id(&e.summary);
            }
        }
    }
}

fn split_notes(rest: &str) -> (String, Option<String>) {
    if let Some((a, b)) = rest.split_once(" — ") {
        (a.trim().to_string(), Some(b.trim().to_string()))
    } else {
        (rest.to_string(), None)
    }
}

#[must_use]
pub fn new_task_id(seed: &str) -> String {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update(format!("{:?}", std::time::SystemTime::now()).as_bytes());
    let hex = format!("{:x}", h.finalize());
    hex.chars().take(8).collect()
}

/// Absolute path to `.ide/tasks.md`.
#[must_use]
pub fn tasks_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".ide").join("tasks.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_tasks() {
        let list = TaskList {
            entries: vec![Task {
                id: "a1b2c3d4".into(),
                summary: "Fix auth".into(),
                status: TaskStatus::Open,
                notes: None,
            }],
        };
        let md = list.to_markdown();
        let back = TaskList::parse(&md).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].id, "a1b2c3d4");
        assert_eq!(back.entries[0].summary, "Fix auth");
    }
}
