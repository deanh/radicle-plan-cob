# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-02-27

### Added

- Short-form ID support for plans, tasks, issues, patches, and commits (7-character minimum prefix)
- `task link-commit` CLI subcommand to associate a commit with a task
- `task.linkCommit` COB action and `link_task_to_commit()` method
- `--files` flag on the `task edit` subcommand
- `--reply-to` passthrough in the comment handler

### Changed

- Tasks now use linked commits as their completion signal instead of mutable status — a task is "done" when it has a linked commit, eliminating stale-status problems
- `unblocked_tasks()` rewritten to use `linked_commit` presence
- Task display uses `[x]`/`[ ]` checkboxes based on `linked_commit`
- ID resolution replaced: `resolve_cob_id()`, `resolve_plan_id_from_store()`, `resolve_task_id()`, `resolve_oid()`, and `resolve_comment_id()` superseded by prefix-aware resolvers (`validate_hex_prefix()`, `resolve_cob_prefix()`, `resolve_task_prefix()`, `resolve_commit_sha()`, `resolve_comment_prefix()`)

### Removed

- `TaskStatus` enum
- `task start` and `task complete` CLI subcommands
- `pending_tasks()`, `in_progress_tasks()`, and `completed_tasks()` helpers

## [0.1.0] - 2026-02-24

### Added

- Initial release: `radicle-plan-cob` extracted from `rad-skill`
- `me.hdh.plan` collaborative object type for implementation plans
- Plan CRUD operations (create, edit, list, show, delete)
- Task management within plans (add, edit, remove, reorder)
- Task dependencies (blocks/blocked-by)
- Comments on plans with threading support
- `rad-plan` CLI binary
- `radicle_plan_cob` library crate

[0.2.0]: https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5/commits/v0.2.0
[0.1.0]: https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5/commits/v0.1.0
