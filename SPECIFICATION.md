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
    status: TaskStatus,
    blocked_by: Vec<TaskId>,       // Task dependencies
    affected_files: Vec<String>,   // Files this task will modify
    linked_issue: Option<ObjectId>, // If converted to Radicle issue
    author: Did,
    created_at: Timestamp,
    updated_at: Timestamp,
}
```

### Task Status

```rust
enum TaskStatus {
    Pending,    // Not started
    InProgress, // Currently being worked on
    Completed,  // Done
    Skipped,    // Won't be done
}
```

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
| `task.status` | Change task status | Author or delegate |
| `task.remove` | Remove a task | Author or delegate |
| `task.reorder` | Reorder tasks | Author or delegate |
| `task.blockedBy` | Set task dependencies | Author or delegate |
| `task.linkIssue` | Link task to Radicle issue | Author or delegate |

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

### Set Task Status Action

```json
{
  "type": "task.status",
  "taskId": "abc123...",
  "status": "completed"
}
```

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

- **Status**: Last-writer-wins based on timestamp
- **Tasks**: Ordered by creation entry ID, reorder action overwrites
- **Sets (labels, assignees, issues, patches)**: Union of all additions, intersection of removals
- **Thread**: Standard Radicle thread CRDT semantics

## Authorization Model

Follows the same model as Radicle Issues and Patches:

1. **Repository delegates** can perform all actions
2. **Plan author** can perform most actions on their own plan
3. **Any user** can comment on plans
4. **Comment authors** can edit/redact their own comments

## CLI Usage

```bash
# Create a plan
rad-plan open "Implement user auth" --description "JWT-based auth system"

# List plans
rad-plan list
rad-plan list --status in-progress

# Show plan details
rad-plan show abc123
rad-plan show abc123 --json

# Add tasks
rad-plan task add abc123 "Create auth middleware" --estimate "4h"
rad-plan task add abc123 "Write tests" --files "tests/auth.test.ts"

# Edit tasks
rad-plan task edit abc123 <task-id> --subject "Updated title"
rad-plan task edit abc123 <task-id> --description "New details"
rad-plan task edit abc123 <task-id> --files "src/client.rs,src/config.rs"

# Update task status
rad-plan task complete abc123 <task-id>
rad-plan task start abc123 <task-id>

# Comments
rad-plan comment abc123 "Implementation note"
rad-plan comment abc123 "Reply" --reply-to <comment-id>

# Link to issues/patches
rad-plan link abc123 --issue def456
rad-plan link abc123 --patch ghi789

# Export
rad-plan export abc123 --format md
rad-plan export abc123 --format json
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
