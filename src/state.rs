//! Plan state structures.

use std::collections::BTreeSet;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use radicle::cob::common::{Author, Label, Timestamp};
use radicle::cob::thread::{CommentId, Thread};
use radicle::cob::{ActorId, ObjectId};
use radicle::git::Oid;
use radicle::prelude::Did;

/// Task identifier (same as entry ID that created it).
pub type TaskId = Oid;

/// Plan status.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanStatus {
    /// Plan is in draft mode, still being designed.
    #[default]
    Draft,
    /// Plan has been approved and is ready for implementation.
    Approved,
    /// Plan implementation is in progress.
    InProgress,
    /// Plan has been fully implemented.
    Completed,
    /// Plan has been archived (no longer active).
    Archived,
}

impl std::fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Approved => write!(f, "approved"),
            Self::InProgress => write!(f, "in-progress"),
            Self::Completed => write!(f, "completed"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for PlanStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "approved" => Ok(Self::Approved),
            "in-progress" | "inprogress" | "in_progress" => Ok(Self::InProgress),
            "completed" | "done" => Ok(Self::Completed),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("unknown plan status: {s}")),
        }
    }
}

/// A task within a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique identifier (entry ID that created this task).
    pub id: TaskId,
    /// Task subject/title.
    pub subject: String,
    /// Optional detailed description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional time estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
    /// Tasks that must be completed before this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<TaskId>,
    /// Files affected by this task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    /// Linked Radicle issue (if task was converted to an issue).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_issue: Option<ObjectId>,
    /// Linked commit OID — when present, the task is considered done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_commit: Option<Oid>,
    /// Author who created the task.
    pub author: Did,
    /// When the task was created.
    pub created_at: Timestamp,
}

impl Task {
    /// Create a new task.
    pub fn new(
        id: TaskId,
        subject: String,
        description: Option<String>,
        estimate: Option<String>,
        affected_files: Vec<String>,
        author: ActorId,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            id,
            subject,
            description,
            estimate,
            blocked_by: Vec::new(),
            affected_files,
            linked_issue: None,
            linked_commit: None,
            author: author.into(),
            created_at: timestamp,
        }
    }

    /// Check if the task is blocked.
    pub fn is_blocked(&self) -> bool {
        !self.blocked_by.is_empty()
    }

    /// Check if the task is done (has a linked commit).
    pub fn is_done(&self) -> bool {
        self.linked_commit.is_some()
    }
}

/// Plan state. Accumulates [`Action`](crate::Action).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Plan title.
    pub(crate) title: String,
    /// Plan description.
    pub(crate) description: String,
    /// Current plan status.
    pub(crate) status: PlanStatus,
    /// Tasks in this plan.
    pub(crate) tasks: Vec<Task>,
    /// Related Radicle issues.
    pub(crate) related_issues: BTreeSet<ObjectId>,
    /// Related Radicle patches.
    pub(crate) related_patches: BTreeSet<ObjectId>,
    /// Critical files that the plan affects.
    pub(crate) critical_files: BTreeSet<String>,
    /// Associated labels.
    pub(crate) labels: BTreeSet<Label>,
    /// Actors assigned to this plan.
    pub(crate) assignees: BTreeSet<Did>,
    /// Discussion thread.
    pub(crate) thread: Thread,
    /// Plan author.
    pub(crate) author: Author,
    /// When the plan was created.
    pub(crate) created_at: Timestamp,
}

impl Plan {
    /// Create a new plan.
    pub fn new(
        title: String,
        description: String,
        thread: Thread,
        author: Author,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            title,
            description,
            status: PlanStatus::Draft,
            tasks: Vec::new(),
            related_issues: BTreeSet::new(),
            related_patches: BTreeSet::new(),
            critical_files: BTreeSet::new(),
            labels: BTreeSet::new(),
            assignees: BTreeSet::new(),
            thread,
            author,
            created_at: timestamp,
        }
    }

    /// Get the plan title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get the plan description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the plan status.
    pub fn status(&self) -> &PlanStatus {
        &self.status
    }

    /// Get the plan author.
    pub fn author(&self) -> &Author {
        &self.author
    }

    /// Get when the plan was created.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Get the root comment (plan description).
    pub fn root(&self) -> (&CommentId, &radicle::cob::thread::Comment) {
        self.thread
            .comments()
            .next()
            .expect("Plan::root: at least one comment is present")
    }

    /// Get all tasks.
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    /// Get a task by ID.
    pub fn task(&self, id: &TaskId) -> Option<&Task> {
        self.tasks.iter().find(|t| &t.id == id)
    }

    /// Get tasks that are not yet done and whose blockers are all done.
    pub fn unblocked_tasks(&self) -> impl Iterator<Item = &Task> {
        let done_ids: BTreeSet<_> = self
            .tasks
            .iter()
            .filter(|t| t.is_done())
            .map(|t| t.id)
            .collect();

        self.tasks.iter().filter(move |t| {
            !t.is_done()
                && t.blocked_by.iter().all(|b| done_ids.contains(b))
        })
    }

    /// Get related issues.
    pub fn related_issues(&self) -> impl Iterator<Item = &ObjectId> {
        self.related_issues.iter()
    }

    /// Get related patches.
    pub fn related_patches(&self) -> impl Iterator<Item = &ObjectId> {
        self.related_patches.iter()
    }

    /// Get critical files.
    pub fn critical_files(&self) -> impl Iterator<Item = &String> {
        self.critical_files.iter()
    }

    /// Get labels.
    pub fn labels(&self) -> impl Iterator<Item = &Label> {
        self.labels.iter()
    }

    /// Get assignees.
    pub fn assignees(&self) -> impl Iterator<Item = &Did> {
        self.assignees.iter()
    }

    /// Get the discussion thread.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    /// Get comments.
    pub fn comments(&self) -> impl Iterator<Item = (&CommentId, &radicle::cob::thread::Comment)> {
        self.thread.comments()
    }

    /// Calculate completion percentage.
    pub fn completion_percentage(&self) -> f64 {
        if self.tasks.is_empty() {
            return 0.0;
        }
        let done = self.tasks.iter().filter(|t| t.is_done()).count();
        (done as f64 / self.tasks.len() as f64) * 100.0
    }

    /// Check if all tasks are complete.
    pub fn all_tasks_complete(&self) -> bool {
        !self.tasks.is_empty() && self.tasks.iter().all(|t| t.is_done())
    }
}

impl Deref for Plan {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        &self.thread
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_plan_status_display() {
        assert_eq!(PlanStatus::Draft.to_string(), "draft");
        assert_eq!(PlanStatus::InProgress.to_string(), "in-progress");
        assert_eq!(PlanStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_plan_status_parse() {
        assert_eq!("draft".parse::<PlanStatus>().unwrap(), PlanStatus::Draft);
        assert_eq!("in-progress".parse::<PlanStatus>().unwrap(), PlanStatus::InProgress);
        assert_eq!("in_progress".parse::<PlanStatus>().unwrap(), PlanStatus::InProgress);
        assert!("invalid".parse::<PlanStatus>().is_err());
    }

    #[test]
    fn test_task_is_done() {
        use radicle::git::Oid;

        let task_id = TaskId::from(Oid::from_str("0000000000000000000000000000000000000000").unwrap());
        let author = Did::from_str("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap();

        let mut task = Task {
            id: task_id,
            subject: "Test".to_string(),
            description: None,
            estimate: None,
            blocked_by: vec![],
            affected_files: vec![],
            linked_issue: None,
            linked_commit: None,
            author,
            created_at: Timestamp::from_secs(0),
        };

        assert!(!task.is_done());

        // Linking a commit marks the task as done
        task.linked_commit = Some(Oid::from_str("abcdef0000000000000000000000000000000001").unwrap());
        assert!(task.is_done());
    }
}
