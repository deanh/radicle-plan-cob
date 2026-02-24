# radicle-plan-cob

A Radicle Collaborative Object (COB) type for storing implementation plans.

Plans are first-class COBs that can link bidirectionally to Issues and Patches, enabling tracked progress of development work across the Radicle network.

## Installation

```bash
cargo install --path .
```

This installs the `rad-plan` binary to your Cargo bin directory.

## Usage

### Create a plan

```bash
rad-plan open "Implement user authentication" --description "JWT-based auth system"
```

### List plans

```bash
rad-plan list
rad-plan list --status in-progress
rad-plan list --all  # Include archived
```

### Show plan details

```bash
rad-plan show <plan-id>
rad-plan show <plan-id> --json
```

### Manage tasks

```bash
# Add a task
rad-plan task add <plan-id> "Create auth middleware" --estimate "4h"
rad-plan task add <plan-id> "Write tests" --files "tests/auth.test.ts"

# Edit a task
rad-plan task edit <plan-id> <task-id> --subject "Updated title"
rad-plan task edit <plan-id> <task-id> --files "src/client.rs,src/config.rs"

# Update task status
rad-plan task start <plan-id> <task-id>
rad-plan task complete <plan-id> <task-id>

# List tasks
rad-plan task list <plan-id>
rad-plan task list <plan-id> --status pending
```

### Comments

```bash
rad-plan comment <plan-id> "Implementation note"
rad-plan comment <plan-id> "Reply to your point" --reply-to <comment-id>
```

### Link to other COBs

```bash
rad-plan link <plan-id> --issue <issue-id>
rad-plan link <plan-id> --patch <patch-id>
```

### Export

```bash
rad-plan export <plan-id> --format md
rad-plan export <plan-id> --format json --output plan.json
```

## COB Type

The COB type name is `me.hdh.plan` following Radicle's reverse domain notation pattern.

See [SPECIFICATION.md](SPECIFICATION.md) for full documentation of the data model, actions, and CRDT semantics.

## Local Development

For local development with a heartwood checkout, create `.cargo/config.toml`:

```toml
[patch."https://seed.radicle.xyz/z3gqcJUoA1n9HaHKufZs5FCSGazv5.git"]
radicle = { path = "../heartwood/crates/radicle" }
radicle-cob = { path = "../heartwood/crates/radicle-cob" }
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
