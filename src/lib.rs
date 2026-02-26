//! # radicle-plan-cob
//!
//! A Radicle Collaborative Object (COB) type for storing implementation plans.
//!
//! Plans are first-class COBs that can link bidirectionally to Issues and Patches,
//! enabling tracked progress of development work across the Radicle network.
//!
//! ## Type Name
//!
//! The COB type name is `me.hdh.plan` following the reverse domain notation pattern.

#![warn(clippy::unwrap_used)]
#![warn(missing_docs)]

pub mod actions;
pub mod state;

use std::collections::BTreeSet;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::LazyLock;

use serde::Serialize;
use thiserror::Error;

use radicle::cob;
use radicle::cob::common::{Authorization, Label, Timestamp, Title, Uri};
use radicle::cob::store::Cob;
use radicle::cob::thread::{Comment, CommentId, Thread};
use radicle::cob::{op, store, ActorId, Embed, EntryId, ObjectId, TypeName};
use radicle::cob::{thread, TitleError};
use radicle::crypto;
use radicle::identity::doc::DocError;
use radicle::node::device::Device;
use radicle::node::NodeId;
use radicle::prelude::{Did, Doc, ReadRepository, RepoId};
use radicle::storage::{HasRepoId, RepositoryError, SignRepository, WriteRepository};

pub use actions::Action;
pub use state::{Plan, PlanStatus, Task, TaskId};

/// Plan operation.
pub type Op = cob::Op<Action>;

/// Type name of a plan COB.
pub static TYPENAME: LazyLock<TypeName> =
    LazyLock::new(|| FromStr::from_str("me.hdh.plan").expect("type name is valid"));

/// Identifier for a plan.
pub type PlanId = ObjectId;

/// Error updating or creating plans.
#[derive(Error, Debug)]
pub enum Error {
    /// Error loading the identity document.
    #[error("identity doc failed to load: {0}")]
    Doc(#[from] DocError),
    /// Thread apply failed.
    #[error("thread apply failed: {0}")]
    Thread(#[from] thread::Error),
    /// Store error.
    #[error("store: {0}")]
    Store(#[from] store::Error),
    /// Invalid plan title.
    #[error("invalid plan title due to: {0}")]
    TitleError(#[from] TitleError),
    /// Action not authorized.
    #[error("{0} not authorized to apply {1:?}")]
    NotAuthorized(ActorId, Action),
    /// Action not allowed.
    #[error("action is not allowed: {0}")]
    NotAllowed(EntryId),
    /// Invalid title.
    #[error("invalid title: {0:?}")]
    InvalidTitle(String),
    /// The identity doc is missing.
    #[error("identity document missing")]
    MissingIdentity,
    /// General error initializing a plan.
    #[error("initialization failed: {0}")]
    Init(&'static str),
    /// Error decoding an operation.
    #[error("op decoding failed: {0}")]
    Op(#[from] op::OpEncodingError),
    /// Task not found.
    #[error("task not found: {0}")]
    TaskNotFound(TaskId),
    /// Invalid task index.
    #[error("invalid task index: {0}")]
    InvalidTaskIndex(usize),
}

impl cob::store::CobWithType for Plan {
    fn type_name() -> &'static TypeName {
        &TYPENAME
    }
}

impl store::Cob for Plan {
    type Action = Action;
    type Error = Error;

    fn from_root<R: ReadRepository>(op: Op, repo: &R) -> Result<Self, Self::Error> {
        let doc = op.identity_doc(repo)?.ok_or(Error::MissingIdentity)?;
        let mut actions = op.actions.into_iter();

        // The first action must be Open
        let Some(Action::Open { title, description, embeds }) = actions.next() else {
            return Err(Error::Init("the first action must be of type `Open`"));
        };

        let comment = Comment::new(
            op.author,
            description.clone(),
            None,
            None,
            embeds,
            op.timestamp,
        );
        let thread = Thread::new(op.id, comment);
        let mut plan = Plan::new(title, description, thread, op.author.into(), op.timestamp);

        for action in actions {
            match plan.authorization(&action, &op.author, &doc)? {
                Authorization::Allow => {
                    plan.apply_action(action, op.id, op.author, op.timestamp)?;
                }
                Authorization::Deny => {
                    return Err(Error::NotAuthorized(op.author, action));
                }
                Authorization::Unknown => {
                    continue;
                }
            }
        }
        Ok(plan)
    }

    fn op<'a, R: ReadRepository, I: IntoIterator<Item = &'a cob::Entry>>(
        &mut self,
        op: Op,
        concurrent: I,
        repo: &R,
    ) -> Result<(), Error> {
        let doc = op.identity_doc(repo)?.ok_or(Error::MissingIdentity)?;
        let _concurrent = concurrent.into_iter().collect::<Vec<_>>();

        for action in op.actions {
            log::trace!(target: "plan", "Applying {} {action:?}", op.id);

            match self.authorization(&action, &op.author, &doc)? {
                Authorization::Allow => {
                    if let Err(e) = self.apply_action(action.clone(), op.id, op.author, op.timestamp) {
                        log::error!(target: "plan", "Error applying {}: {e}", op.id);
                        return Err(e);
                    }
                }
                Authorization::Deny => {
                    return Err(Error::NotAuthorized(op.author, action));
                }
                Authorization::Unknown => {
                    continue;
                }
            }
        }
        Ok(())
    }
}

impl<R: ReadRepository> cob::Evaluate<R> for Plan {
    type Error = Error;

    fn init(entry: &cob::Entry, repo: &R) -> Result<Self, Self::Error> {
        let op = Op::try_from(entry)?;
        let object = Plan::from_root(op, repo)?;
        Ok(object)
    }

    fn apply<'a, I: Iterator<Item = (&'a EntryId, &'a cob::Entry)>>(
        &mut self,
        entry: &cob::Entry,
        concurrent: I,
        repo: &R,
    ) -> Result<(), Self::Error> {
        let op = Op::try_from(entry)?;
        self.op(op, concurrent.map(|(_, e)| e), repo)
    }
}

impl Plan {
    /// Apply a single action to the plan.
    fn apply_action(
        &mut self,
        action: Action,
        entry: EntryId,
        author: ActorId,
        timestamp: Timestamp,
    ) -> Result<(), Error> {
        match action {
            Action::Open { title, description, .. } => {
                self.title = title;
                self.description = description;
            }
            Action::EditTitle { title } => {
                self.title = title.to_string();
            }
            Action::EditDescription { description, .. } => {
                self.description = description;
            }
            Action::SetStatus { status } => {
                self.status = status;
            }
            Action::AddTask { subject, description, estimate, affected_files } => {
                let task = Task::new(
                    entry,
                    subject,
                    description,
                    estimate,
                    affected_files,
                    author,
                    timestamp,
                );
                self.tasks.push(task);
            }
            Action::EditTask { task_id, subject, description, estimate, affected_files } => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    if let Some(s) = subject {
                        task.subject = s;
                    }
                    if let Some(d) = description {
                        task.description = d;
                    }
                    if let Some(e) = estimate {
                        task.estimate = e;
                    }
                    if let Some(f) = affected_files {
                        task.affected_files = f;
                    }
                }
            }
            Action::SetTaskStatus { .. } => {
                // Legacy no-op: old COBs may contain task.status actions.
                log::debug!(target: "plan", "Ignoring legacy SetTaskStatus action");
            }
            Action::LinkTaskToCommit { task_id, commit } => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.linked_commit = Some(commit);
                }
            }
            Action::RemoveTask { task_id } => {
                self.tasks.retain(|t| t.id != task_id);
            }
            Action::ReorderTasks { task_ids } => {
                let mut reordered = Vec::new();
                for id in task_ids {
                    if let Some(task) = self.tasks.iter().find(|t| t.id == id).cloned() {
                        reordered.push(task);
                    }
                }
                // Keep any tasks not in the reorder list at the end
                for task in &self.tasks {
                    if !reordered.iter().any(|t| t.id == task.id) {
                        reordered.push(task.clone());
                    }
                }
                self.tasks = reordered;
            }
            Action::SetTaskBlockedBy { task_id, blocked_by } => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.blocked_by = blocked_by;
                }
            }
            Action::LinkIssue { issue_id } => {
                self.related_issues.insert(issue_id);
            }
            Action::UnlinkIssue { issue_id } => {
                self.related_issues.remove(&issue_id);
            }
            Action::LinkPatch { patch_id } => {
                self.related_patches.insert(patch_id);
            }
            Action::UnlinkPatch { patch_id } => {
                self.related_patches.remove(&patch_id);
            }
            Action::LinkTaskToIssue { task_id, issue_id } => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.linked_issue = Some(issue_id);
                }
            }
            Action::AddCriticalFile { path } => {
                self.critical_files.insert(path);
            }
            Action::RemoveCriticalFile { path } => {
                self.critical_files.remove(&path);
            }
            Action::Comment { body, reply_to, embeds } => {
                thread::comment(
                    &mut self.thread,
                    entry,
                    author,
                    timestamp,
                    body,
                    reply_to,
                    None,
                    embeds,
                )?;
            }
            Action::CommentEdit { id, body, embeds } => {
                thread::edit(&mut self.thread, entry, author, id, timestamp, body, embeds)?;
            }
            Action::CommentRedact { id } => {
                thread::redact(&mut self.thread, entry, id)?;
            }
            Action::Label { labels } => {
                self.labels = BTreeSet::from_iter(labels);
            }
            Action::Assign { assignees } => {
                self.assignees = BTreeSet::from_iter(assignees);
            }
        }
        Ok(())
    }

    /// Apply authorization rules on plan actions.
    pub fn authorization(
        &self,
        action: &Action,
        actor: &ActorId,
        doc: &Doc,
    ) -> Result<Authorization, Error> {
        if doc.is_delegate(&actor.into()) {
            // A delegate is authorized to do all actions.
            return Ok(Authorization::Allow);
        }
        let author: ActorId = *self.author.id().as_key();
        let outcome = match action {
            // Plan authors can edit their own plans.
            Action::Open { .. }
            | Action::EditTitle { .. }
            | Action::EditDescription { .. }
            | Action::SetStatus { .. }
            | Action::AddTask { .. }
            | Action::EditTask { .. }
            | Action::SetTaskStatus { .. }
            | Action::RemoveTask { .. }
            | Action::ReorderTasks { .. }
            | Action::SetTaskBlockedBy { .. }
            | Action::LinkIssue { .. }
            | Action::UnlinkIssue { .. }
            | Action::LinkPatch { .. }
            | Action::UnlinkPatch { .. }
            | Action::LinkTaskToIssue { .. }
            | Action::LinkTaskToCommit { .. }
            | Action::AddCriticalFile { .. }
            | Action::RemoveCriticalFile { .. } => Authorization::from(*actor == author),
            // Only delegates can assign or label.
            Action::Assign { assignees } => {
                if assignees == &self.assignees {
                    Authorization::Allow
                } else {
                    Authorization::Deny
                }
            }
            Action::Label { labels } => {
                if labels == &self.labels {
                    Authorization::Allow
                } else {
                    Authorization::Deny
                }
            }
            // All roles can comment.
            Action::Comment { .. } => Authorization::Allow,
            // Authors can edit/redact their own comments.
            Action::CommentEdit { id, .. } | Action::CommentRedact { id, .. } => {
                // Look up the comment to check authorship
                let mut found_author = None;
                for (cid, comment) in self.thread.comments() {
                    if cid == id {
                        found_author = Some(comment.author());
                        break;
                    }
                }
                if let Some(comment_author) = found_author {
                    Authorization::from(*actor == comment_author)
                } else {
                    Authorization::Unknown
                }
            }
        };
        Ok(outcome)
    }
}

/// Plans store for a repository.
pub struct Plans<'a, R> {
    raw: store::Store<'a, Plan, R>,
}

impl<'a, R> Deref for Plans<'a, R> {
    type Target = store::Store<'a, Plan, R>;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<R> HasRepoId for Plans<'_, R>
where
    R: ReadRepository,
{
    fn rid(&self) -> RepoId {
        self.raw.as_ref().id()
    }
}

/// Detailed information on plan states.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCounts {
    /// Number of draft plans.
    pub draft: usize,
    /// Number of approved plans.
    pub approved: usize,
    /// Number of in-progress plans.
    pub in_progress: usize,
    /// Number of completed plans.
    pub completed: usize,
    /// Number of archived plans.
    pub archived: usize,
}

impl PlanCounts {
    /// Total count.
    pub fn total(&self) -> usize {
        self.draft + self.approved + self.in_progress + self.completed + self.archived
    }

    /// Active count (not archived).
    pub fn active(&self) -> usize {
        self.draft + self.approved + self.in_progress + self.completed
    }
}

impl<'a, R> Plans<'a, R>
where
    R: ReadRepository + cob::Store<Namespace = NodeId>,
{
    /// Open a plans store.
    pub fn open(repository: &'a R) -> Result<Self, RepositoryError> {
        let identity = repository.identity_head()?;
        let raw = store::Store::open(repository)?.identity(identity);
        Ok(Self { raw })
    }
}

impl<'a, R> Plans<'a, R>
where
    R: WriteRepository + cob::Store<Namespace = NodeId>,
{
    /// Create a new plan.
    pub fn create<G>(
        &mut self,
        title: String,
        description: String,
        embeds: Vec<Embed<Uri>>,
        signer: &Device<G>,
    ) -> Result<(ObjectId, Plan), Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        use nonempty::NonEmpty;

        let action = Action::Open {
            title,
            description,
            embeds: embeds.clone(),
        };
        let actions = NonEmpty::new(action);

        self.raw.create("Create plan", actions, embeds, signer).map_err(Error::from)
    }
}

impl<R> Plans<'_, R>
where
    R: ReadRepository + cob::Store,
{
    /// Get a plan.
    pub fn get(&self, id: &ObjectId) -> Result<Option<Plan>, store::Error> {
        self.raw.get(id)
    }

    /// Plans count by state.
    pub fn counts(&self) -> Result<PlanCounts, Error> {
        let all = self.all()?;
        let counts = all
            .filter_map(|s| s.ok())
            .fold(PlanCounts::default(), |mut counts, (_, p)| {
                match p.status() {
                    PlanStatus::Draft => counts.draft += 1,
                    PlanStatus::Approved => counts.approved += 1,
                    PlanStatus::InProgress => counts.in_progress += 1,
                    PlanStatus::Completed => counts.completed += 1,
                    PlanStatus::Archived => counts.archived += 1,
                }
                counts
            });
        Ok(counts)
    }
}

impl<'a, R> Plans<'a, R>
where
    R: WriteRepository + SignRepository + cob::Store<Namespace = NodeId>,
{
    /// Get a plan for mutation.
    pub fn get_mut<'g>(&'g mut self, id: &ObjectId) -> Result<PlanMut<'a, 'g, R>, store::Error> {
        let plan = self
            .raw
            .get(id)?
            .ok_or_else(move || store::Error::NotFound(TYPENAME.clone(), *id))?;

        Ok(PlanMut {
            id: *id,
            plan,
            store: self,
        })
    }
}

/// A mutable plan handle for performing updates.
pub struct PlanMut<'a, 'g, R> {
    id: ObjectId,
    plan: Plan,
    store: &'g mut Plans<'a, R>,
}

impl<R> std::fmt::Debug for PlanMut<'_, '_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PlanMut")
            .field("id", &self.id)
            .field("plan", &self.plan)
            .finish()
    }
}

impl<R> std::ops::Deref for PlanMut<'_, '_, R> {
    type Target = Plan;

    fn deref(&self) -> &Self::Target {
        &self.plan
    }
}

impl<'a, 'g, R> PlanMut<'a, 'g, R>
where
    R: WriteRepository + SignRepository + cob::Store<Namespace = NodeId>,
{
    /// Get the plan ID.
    pub fn id(&self) -> &ObjectId {
        &self.id
    }

    /// Run a transaction on the plan.
    fn transaction<G, F>(
        &mut self,
        message: &str,
        signer: &Device<G>,
        operations: F,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
        F: FnOnce(&mut store::Transaction<Plan, R>) -> Result<(), store::Error>,
    {
        let mut tx = store::Transaction::default();
        operations(&mut tx)?;

        let (plan, commit) = tx.commit(message, self.id, &mut self.store.raw, signer)?;
        self.plan = plan;

        Ok(commit)
    }

    /// Set the plan status.
    pub fn set_status<G>(&mut self, status: PlanStatus, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Set status", signer, |tx| {
            tx.push(Action::SetStatus { status })
        })
    }

    /// Edit the plan title.
    pub fn edit_title<G>(&mut self, title: impl ToString, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        let title = Title::try_from(title.to_string())?;
        self.transaction("Edit title", signer, |tx| {
            tx.push(Action::EditTitle { title })
        })
    }

    /// Edit the plan description.
    pub fn edit_description<G>(
        &mut self,
        description: impl ToString,
        embeds: Vec<Embed<Uri>>,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        let description = description.to_string();
        self.transaction("Edit description", signer, |tx| {
            tx.push(Action::EditDescription { description, embeds })
        })
    }

    /// Add a task to the plan.
    pub fn add_task<G>(
        &mut self,
        subject: impl ToString,
        description: Option<String>,
        estimate: Option<String>,
        affected_files: Vec<String>,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        let subject = subject.to_string();
        self.transaction("Add task", signer, |tx| {
            tx.push(Action::AddTask {
                subject,
                description,
                estimate,
                affected_files,
            })
        })
    }

    /// Link a task to a commit, marking it as done.
    pub fn link_task_to_commit<G>(
        &mut self,
        task_id: TaskId,
        commit: radicle::git::Oid,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Link task to commit", signer, |tx| {
            tx.push(Action::LinkTaskToCommit { task_id, commit })
        })
    }

    /// Edit a task.
    pub fn edit_task<G>(
        &mut self,
        task_id: TaskId,
        subject: Option<String>,
        description: Option<Option<String>>,
        estimate: Option<Option<String>>,
        affected_files: Option<Vec<String>>,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Edit task", signer, |tx| {
            tx.push(Action::EditTask {
                task_id,
                subject,
                description,
                estimate,
                affected_files,
            })
        })
    }

    /// Remove a task from the plan.
    pub fn remove_task<G>(&mut self, task_id: TaskId, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Remove task", signer, |tx| {
            tx.push(Action::RemoveTask { task_id })
        })
    }

    /// Link an issue to the plan.
    pub fn link_issue<G>(&mut self, issue_id: ObjectId, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Link issue", signer, |tx| {
            tx.push(Action::LinkIssue { issue_id })
        })
    }

    /// Unlink an issue from the plan.
    pub fn unlink_issue<G>(&mut self, issue_id: ObjectId, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Unlink issue", signer, |tx| {
            tx.push(Action::UnlinkIssue { issue_id })
        })
    }

    /// Link a patch to the plan.
    pub fn link_patch<G>(&mut self, patch_id: ObjectId, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Link patch", signer, |tx| {
            tx.push(Action::LinkPatch { patch_id })
        })
    }

    /// Unlink a patch from the plan.
    pub fn unlink_patch<G>(&mut self, patch_id: ObjectId, signer: &Device<G>) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Unlink patch", signer, |tx| {
            tx.push(Action::UnlinkPatch { patch_id })
        })
    }

    /// Link a task to an issue.
    pub fn link_task_to_issue<G>(
        &mut self,
        task_id: TaskId,
        issue_id: ObjectId,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        self.transaction("Link task to issue", signer, |tx| {
            tx.push(Action::LinkTaskToIssue { task_id, issue_id })
        })
    }

    /// Add a comment to the plan.
    pub fn comment<G, S>(
        &mut self,
        body: S,
        reply_to: Option<CommentId>,
        embeds: Vec<Embed<Uri>>,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
        S: ToString,
    {
        let body = body.to_string();
        self.transaction("Comment", signer, |tx| {
            tx.push(Action::Comment { body, reply_to, embeds })
        })
    }

    /// Label the plan.
    pub fn label<G>(
        &mut self,
        labels: impl IntoIterator<Item = Label>,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        let labels: BTreeSet<Label> = labels.into_iter().collect();
        self.transaction("Label", signer, |tx| {
            tx.push(Action::Label { labels })
        })
    }

    /// Assign DIDs to the plan.
    pub fn assign<G>(
        &mut self,
        assignees: impl IntoIterator<Item = Did>,
        signer: &Device<G>,
    ) -> Result<EntryId, Error>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        let assignees: BTreeSet<Did> = assignees.into_iter().collect();
        self.transaction("Assign", signer, |tx| {
            tx.push(Action::Assign { assignees })
        })
    }
}
