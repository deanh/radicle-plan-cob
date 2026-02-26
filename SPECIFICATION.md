# me.hdh.plan COB Specification

## Overview

The `me.hdh.plan` Collaborative Object (COB) type stores implementation plans in Radicle repositories. Plans are first-class COBs that can link bidirectionally to Issues and Patches, enabling tracked progress of development work across the Radicle network.

## Type Name

```
me.hdh.plan
```

Following the reverse domain notation pattern used by Radicle COBs (e.g., `xyz.radicle.issue`, `xyz.radicle.patch`).

## Data Model

### Plan State

```rust
struct Plan {
    title: String,
    description: String,
    status: PlanStatus,
    tasks: Vec<Task>,
    related_issues: BTreeSet<ObjectId>,
    related_patches: BTreeSet<ObjectId>,
    critical_files: BTreeSet<String>,
    labels: BTreeSet<Label>,
    assignees: BTreeSet<Did>,
    thread: Thread,  // For comments/discussion
    author: Author,
    created_at: Timestamp,
}
```

### Plan Status

```rust
enum PlanStatus {
    Draft,      // Being designed, not ready for implementation
    Approved,   // Ready for implementation
    InProgress, // Implementation underway
    Completed,  // All tasks done
    Archived,   // No longer active
}
```

### Task

```rust
struct Task {
    id: TaskId,                    // Entry ID that created this task
    subject: String,               // Task title
    description: Option<String>,   // Detailed description
    estimate: Option<String>,      // Time estimate (e.g., "2h", "1d")
    blocked_by: Vec<TaskId>,       // Task dependencies
    affected_files: Vec<String>,   // Files this task will modify
    linked_issue: Option<ObjectId>, // If converted to Radicle issue
    linked_commit: Option<Oid>,    // Commit that completes this task
    author: Did,
    created_at: Timestamp,
}
```

A task is considered **done** when `linked_commit` is `Some`. There is no mutable status field — completion is signaled by linking the commit that implements the task.

## Actions

Actions are the operations that can be applied to a Plan COB. Each action is serialized as JSON and stored in the change history.

### Plan Lifecycle Actions

| Action | Description | Authorization |
|--------|-------------|---------------|
| `open` | Create new plan | Any user |
| `edit.title` | Change plan title | Author or delegate |
| `edit.description` | Change plan description | Author or delegate |
| `status` | Change plan status | Author or delegate |

### Task Actions

| Action | Description | Authorization |
|--------|-------------|---------------|
| `task.add` | Add a new task | Author or delegate |
| `task.edit` | Edit task details | Author or delegate |
| `task.linkCommit` | Link task to a commit (marks done) | Author or delegate |
| `task.remove` | Remove a task | Author or delegate |
| `task.reorder` | Reorder tasks | Author or delegate |
| `task.blockedBy` | Set task dependencies | Author or delegate |
| `task.linkIssue` | Link task to Radicle issue | Author or delegate |
| `task.status` | _(deprecated, no-op)_ Legacy status change | Author or delegate |

### Linking Actions

| Action | Description | Authorization |
|--------|-------------|---------------|
| `link.issue` | Link plan to Radicle issue | Author or delegate |
| `unlink.issue` | Remove issue link | Author or delegate |
| `link.patch` | Link plan to Radicle patch | Author or delegate |
| `unlink.patch` | Remove patch link | Author or delegate |
| `criticalFile.add` | Mark file as critical | Author or delegate |
| `criticalFile.remove` | Unmark critical file | Author or delegate |

### Discussion Actions

| Action | Description | Authorization |
|--------|-------------|---------------|
| `comment` | Add a comment | Any user |
| `comment.edit` | Edit own comment | Comment author |
| `comment.redact` | Redact own comment | Comment author |

### Metadata Actions

| Action | Description | Authorization |
|--------|-------------|---------------|
| `label` | Set plan labels | Delegate only |
| `assign` | Set plan assignees | Delegate only |

## Action JSON Schemas

### Open Action

```json
{
  "type": "open",
  "title": "Implement user authentication",
  "description": "Design and implement JWT-based authentication system",
  "embeds": []
}
```

### Add Task Action

```json
{
  "type": "task.add",
  "subject": "Create auth middleware",
  "description": "Set up JWT validation middleware for Express",
  "estimate": "4h",
  "affectedFiles": ["src/middleware/auth.ts", "src/types/auth.ts"]
}
```

### Edit Task Action

```json
{
  "type": "task.edit",
  "taskId": "abc123...",
  "subject": "Updated task title",
  "affectedFiles": ["src/client.rs", "src/config.rs"]
}
```

All fields except `taskId` are optional. Only provided fields are updated; omitted fields are left unchanged.

### Link Task to Commit Action

```json
{
  "type": "task.linkCommit",
  "task_id": "abc123...",
  "commit": "def456..."
}
```

> **Deprecated:** The `task.status` action is still accepted for backward compatibility with existing COBs but is applied as a no-op.

### Link Issue Action

```json
{
  "type": "link.issue",
  "issueId": "def456..."
}
```

## Storage

Plans are stored under the Git refs namespace:

```
refs/cobs/me.hdh.plan/<PLAN-ID>
```

Each Plan ID is a content-addressed identifier derived from the initial change commit.

## CRDT Semantics

Like other Radicle COBs, Plans use operation-based CRDTs:

1. **Operations are commutative**: Can be applied in any order to reach the same final state
2. **Deterministic ordering**: Topological sort of the DAG ensures consensus
3. **Offline-first**: Full local functionality, sync on reconnection

### Conflict Resolution

- **Plan Status**: Last-writer-wins based on timestamp
- **Tasks**: Ordered by creation entry ID, reorder action overwrites
- **Linked commits**: Last-writer-wins (linking a second commit to the same task replaces the first)
- **Sets (labels, assignees, issues, patches)**: Union of all additions, intersection of removals
- **Thread**: Standard Radicle thread CRDT semantics

## Authorization Model

Follows the same model as Radicle Issues and Patches:

1. **Repository delegates** can perform all actions
2. **Plan author** can perform most actions on their own plan
3. **Any user** can comment on plans
4. **Comment authors** can edit/redact their own comments

## CLI Usage

All commands accept short-form IDs (minimum 7 hex characters) or full 40-character IDs. Short prefixes are resolved automatically; ambiguous prefixes produce a clear error.

```bash
# Create a plan
rad-plan open "Implement user auth" --description "JWT-based auth system"

# List plans
rad-plan list
rad-plan list --status in-progress

# Show plan details (short-form ID)
rad-plan show abc1234
rad-plan show abc1234 --json

# Add tasks
rad-plan task add abc1234 "Create auth middleware" --estimate "4h"
rad-plan task add abc1234 "Write tests" --files "tests/auth.test.ts"

# Edit tasks (short-form plan and task IDs)
rad-plan task edit abc1234 def5678 --subject "Updated title"
rad-plan task edit abc1234 def5678 --description "New details"
rad-plan task edit abc1234 def5678 --files "src/client.rs,src/config.rs"

# Link a task to a commit (short-form commit SHA)
rad-plan task link-commit abc1234 def5678 --commit 9a1b2c3

# Comments (short-form reply-to ID)
rad-plan comment abc1234 "Implementation note"
rad-plan comment abc1234 "Reply" --reply-to 1234567

# Link to issues/patches (short-form IDs)
rad-plan link abc1234 --issue 108a1dc
rad-plan link abc1234 --patch aabb123

# Export
rad-plan export abc1234 --format md
rad-plan export abc1234 --format json
```

## Integration with rad-skill

The Plan COB integrates with the rad-skill Claude Code plugin:

### /rad-import --save-plan

Creates a Plan COB when importing a Radicle issue, storing the implementation breakdown.

### /rad-plan sync

Synchronizes Claude Code task completion status to the Plan COB.

### /rad-plan list

Lists plans in the current repository.

## Migration Path

For potential upstream inclusion:

1. **Phase 1**: Prototype as `me.hdh.plan` in this repository
2. **Phase 2**: Gather community feedback via Radicle Zulip
3. **Phase 3**: If traction, propose RFC for `xyz.radicle.plan`
4. **Phase 4**: Port to heartwood patterns and submit PR

## References

- [Radicle Protocol Overview](https://hackmd.io/@radicle/rJ2UH54P6)
- [radicle-cob crate](https://docs.rs/radicle-cob/)
- [heartwood repository](https://github.com/radicle-dev/heartwood)
- [Radicle Issue COB implementation](https://github.com/radicle-dev/heartwood/blob/master/radicle/src/cob/issue.rs)
