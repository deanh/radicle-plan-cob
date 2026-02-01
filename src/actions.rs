//! Plan actions that can be applied to the COB state.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use radicle::cob::common::{Label, Uri};
use radicle::cob::store::CobAction;
use radicle::cob::thread::CommentId;
use radicle::cob::{Embed, ObjectId, Title};
use radicle::prelude::Did;

use crate::state::{PlanStatus, TaskId, TaskStatus};

/// Plan action. Represents all possible mutations to a plan's state.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Open a new plan (initial action).
    #[serde(rename = "open")]
    Open {
        /// Plan title.
        title: String,
        /// Plan description.
        description: String,
        /// Embedded content.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        embeds: Vec<Embed<Uri>>,
    },

    /// Edit the plan title.
    #[serde(rename = "edit.title")]
    EditTitle {
        /// New title.
        title: Title,
    },

    /// Edit the plan description.
    #[serde(rename = "edit.description")]
    EditDescription {
        /// New description.
        description: String,
        /// Embedded content.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        embeds: Vec<Embed<Uri>>,
    },

    /// Set the plan status.
    #[serde(rename = "status")]
    SetStatus {
        /// New status.
        status: PlanStatus,
    },

    /// Add a task to the plan.
    #[serde(rename = "task.add")]
    AddTask {
        /// Task subject/title.
        subject: String,
        /// Optional detailed description.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Optional time estimate (e.g., "2h", "1d").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        estimate: Option<String>,
        /// Files affected by this task.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        affected_files: Vec<String>,
    },

    /// Edit an existing task.
    #[serde(rename = "task.edit")]
    EditTask {
        /// Task ID to edit.
        task_id: TaskId,
        /// New subject (if changing).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<String>,
        /// New description (if changing).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<Option<String>>,
        /// New estimate (if changing).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        estimate: Option<Option<String>>,
        /// New affected files (if changing).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        affected_files: Option<Vec<String>>,
    },

    /// Set a task's status.
    #[serde(rename = "task.status")]
    SetTaskStatus {
        /// Task ID.
        task_id: TaskId,
        /// New status.
        status: TaskStatus,
    },

    /// Remove a task from the plan.
    #[serde(rename = "task.remove")]
    RemoveTask {
        /// Task ID to remove.
        task_id: TaskId,
    },

    /// Reorder tasks in the plan.
    #[serde(rename = "task.reorder")]
    ReorderTasks {
        /// New task order (task IDs).
        task_ids: Vec<TaskId>,
    },

    /// Set which tasks block a given task.
    #[serde(rename = "task.blockedBy")]
    SetTaskBlockedBy {
        /// Task ID.
        task_id: TaskId,
        /// IDs of blocking tasks.
        blocked_by: Vec<TaskId>,
    },

    /// Link a Radicle issue to the plan.
    #[serde(rename = "link.issue")]
    LinkIssue {
        /// Issue object ID.
        issue_id: ObjectId,
    },

    /// Unlink a Radicle issue from the plan.
    #[serde(rename = "unlink.issue")]
    UnlinkIssue {
        /// Issue object ID.
        issue_id: ObjectId,
    },

    /// Link a Radicle patch to the plan.
    #[serde(rename = "link.patch")]
    LinkPatch {
        /// Patch object ID.
        patch_id: ObjectId,
    },

    /// Unlink a Radicle patch from the plan.
    #[serde(rename = "unlink.patch")]
    UnlinkPatch {
        /// Patch object ID.
        patch_id: ObjectId,
    },

    /// Link a task to a specific issue.
    #[serde(rename = "task.linkIssue")]
    LinkTaskToIssue {
        /// Task ID.
        task_id: TaskId,
        /// Issue to link.
        issue_id: ObjectId,
    },

    /// Add a critical file path.
    #[serde(rename = "criticalFile.add")]
    AddCriticalFile {
        /// File path.
        path: String,
    },

    /// Remove a critical file path.
    #[serde(rename = "criticalFile.remove")]
    RemoveCriticalFile {
        /// File path.
        path: String,
    },

    /// Comment on the plan.
    #[serde(rename = "comment")]
    #[serde(rename_all = "camelCase")]
    Comment {
        /// Comment body.
        body: String,
        /// Comment this is a reply to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reply_to: Option<CommentId>,
        /// Embedded content.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        embeds: Vec<Embed<Uri>>,
    },

    /// Edit a comment.
    #[serde(rename = "comment.edit")]
    CommentEdit {
        /// Comment being edited.
        id: CommentId,
        /// New value for the comment body.
        body: String,
        /// New value for the embeds list.
        embeds: Vec<Embed<Uri>>,
    },

    /// Redact a comment.
    #[serde(rename = "comment.redact")]
    CommentRedact {
        /// Comment to redact.
        id: CommentId,
    },

    /// Modify plan labels.
    #[serde(rename = "label")]
    Label {
        /// New set of labels.
        labels: BTreeSet<Label>,
    },

    /// Assign users to the plan.
    #[serde(rename = "assign")]
    Assign {
        /// New set of assignees.
        assignees: BTreeSet<Did>,
    },
}

impl CobAction for Action {
    fn produces_identifier(&self) -> bool {
        matches!(self, Self::Comment { .. } | Self::AddTask { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_action_serialization() {
        let action = Action::Open {
            title: "Test Plan".to_string(),
            description: "A test plan description".to_string(),
            embeds: vec![],
        };

        let json = serde_json::to_string(&action).expect("serialization failed");
        assert!(json.contains("\"type\":\"open\""));

        let deserialized: Action = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_task_action_serialization() {
        use radicle::git::Oid;

        let task_id = TaskId::from(Oid::from_str("0000000000000000000000000000000000000000").unwrap());
        let action = Action::SetTaskStatus {
            task_id,
            status: TaskStatus::Completed,
        };

        let json = serde_json::to_string(&action).expect("serialization failed");
        assert!(json.contains("\"type\":\"task.status\""));

        let deserialized: Action = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(action, deserialized);
    }
}
