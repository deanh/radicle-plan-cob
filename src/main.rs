//! rad-plan CLI tool for managing Plan COBs.
//!
//! Usage:
//!   rad-plan open <title> [--description <desc>]
//!   rad-plan list [--status <status>]
//!   rad-plan show <id>
//!   rad-plan task add <plan-id> <subject> [--description <desc>]
//!   rad-plan task link-commit <plan-id> <task-id> --commit <oid>
//!   rad-plan task list <plan-id>
//!   rad-plan link --issue <issue-id> <plan-id>
//!   rad-plan link --patch <patch-id> <plan-id>
//!   rad-plan comment <plan-id> <message>
//!   rad-plan export <plan-id> [--format md|json]

use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use clap::{Parser, Subcommand};

use radicle::cob::thread::CommentId;
use radicle::cob::ObjectId;
use radicle::profile::Profile;
use radicle::rad;
use radicle::storage::ReadStorage;

use radicle_plan_cob::{PlanId, PlanStatus, Plans, TaskId};

/// rad-plan: Manage implementation plans as Radicle COBs
#[derive(Parser)]
#[command(name = "rad-plan")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the repository (defaults to current directory)
    #[arg(short, long, global = true)]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new plan
    Open {
        /// Plan title
        title: String,

        /// Plan description
        #[arg(short, long)]
        description: Option<String>,

        /// Labels to apply
        #[arg(short, long)]
        labels: Vec<String>,
    },

    /// List all plans
    List {
        /// Filter by status (draft, approved, in-progress, completed, archived)
        #[arg(short, long)]
        status: Option<String>,

        /// Show all plans including archived
        #[arg(short, long)]
        all: bool,
    },

    /// Show plan details
    Show {
        /// Plan ID (short form or full ID)
        id: String,

        /// Show in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Set plan status
    Status {
        /// Plan ID
        id: String,

        /// New status (draft, approved, in-progress, completed, archived)
        status: String,
    },

    /// Manage plan tasks
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },

    /// Link a COB to the plan
    Link {
        /// Plan ID
        plan_id: String,

        /// Issue ID to link
        #[arg(long)]
        issue: Option<String>,

        /// Patch ID to link
        #[arg(long)]
        patch: Option<String>,
    },

    /// Unlink a COB from the plan
    Unlink {
        /// Plan ID
        plan_id: String,

        /// Issue ID to unlink
        #[arg(long)]
        issue: Option<String>,

        /// Patch ID to unlink
        #[arg(long)]
        patch: Option<String>,
    },

    /// Add a comment to the plan
    Comment {
        /// Plan ID
        plan_id: String,

        /// Comment message
        message: String,

        /// Reply to a specific comment ID
        #[arg(long)]
        reply_to: Option<String>,
    },

    /// Export plan to another format
    Export {
        /// Plan ID
        id: String,

        /// Output format (md, json)
        #[arg(short, long, default_value = "md")]
        format: String,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Edit plan title or description
    Edit {
        /// Plan ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,
    },
}

#[derive(Subcommand)]
enum TaskCommands {
    /// Add a task to a plan
    Add {
        /// Plan ID
        plan_id: String,

        /// Task subject
        subject: String,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,

        /// Time estimate
        #[arg(short, long)]
        estimate: Option<String>,

        /// Affected files
        #[arg(short, long)]
        files: Vec<String>,
    },

    /// List tasks in a plan
    List {
        /// Plan ID
        plan_id: String,
    },

    /// Link a task to a commit (marks the task as done)
    LinkCommit {
        /// Plan ID
        plan_id: String,

        /// Task ID
        task_id: String,

        /// Commit OID
        #[arg(long)]
        commit: String,
    },

    /// Edit a task
    Edit {
        /// Plan ID
        plan_id: String,

        /// Task ID
        task_id: String,

        /// New subject
        #[arg(short, long)]
        subject: Option<String>,

        /// New description
        #[arg(short, long)]
        description: Option<String>,

        /// New estimate
        #[arg(short, long)]
        estimate: Option<String>,

        /// Affected files (replaces existing list)
        #[arg(short, long)]
        files: Vec<String>,
    },

    /// Remove a task
    Remove {
        /// Plan ID
        plan_id: String,

        /// Task ID
        task_id: String,
    },

    /// Link a task to a Radicle issue
    Link {
        /// Plan ID
        plan_id: String,

        /// Task ID
        task_id: String,

        /// Issue ID to link
        #[arg(long)]
        issue: String,
    },
}

fn main() -> ExitCode {
    env_logger::init();

    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Load profile and get repository
    let profile = Profile::load()?;

    let (_, rid) = if let Some(path) = cli.repo {
        rad::at(&path)?
    } else {
        rad::cwd()?
    };

    let repo = profile.storage.repository(rid)?;

    match cli.command {
        Commands::Open { title, description, labels: _ } => {
            let mut plans = Plans::open(&repo)?;
            let signer = profile.signer()?;
            let desc = description.unwrap_or_default();

            let (id, plan) = plans.create(title.clone(), desc, vec![], &signer)?;

            println!("Plan created: {}", id);
            println!("  Title: {}", plan.title());
            println!("  Status: {:?}", plan.status());
        }
        Commands::List { status, all } => {
            let plans = Plans::open(&repo)?;
            let counts = plans.counts()?;

            println!("Plans ({} total, {} active):", counts.total(), counts.active());
            println!();

            let status_filter = status.as_ref().map(|s| parse_plan_status(s));

            for result in plans.all()? {
                let (id, plan) = result?;

                // Filter by status if specified
                if let Some(filter) = &status_filter {
                    if plan.status() != filter {
                        continue;
                    }
                }

                // Skip archived unless --all
                if !all && matches!(plan.status(), PlanStatus::Archived) {
                    continue;
                }

                let status_icon = match plan.status() {
                    PlanStatus::Draft => "📝",
                    PlanStatus::Approved => "✅",
                    PlanStatus::InProgress => "🚧",
                    PlanStatus::Completed => "✓",
                    PlanStatus::Archived => "📦",
                };

                let task_count = plan.tasks().len();
                let done = plan.tasks().iter().filter(|t| t.is_done()).count();

                println!("{} {} {} [{}/{}]", status_icon, short_id(&id), plan.title(), done, task_count);
            }
        }
        Commands::Show { id, json } => {
            let plans = Plans::open(&repo)?;
            let plan_id = resolve_plan_id_from_store(&id, &plans)?;

            let Some(plan) = plans.get(&plan_id)? else {
                return Err(format!("Plan not found: {id}").into());
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("# {}", plan.title());
                println!();
                println!("ID: {}", plan_id);
                println!("Status: {:?}", plan.status());
                println!("Author: {}", plan.author());
                println!();

                if !plan.description().is_empty() {
                    println!("## Description");
                    println!();
                    println!("{}", plan.description());
                    println!();
                }

                println!("## Tasks ({} total)", plan.tasks().len());
                println!();

                for task in plan.tasks() {
                    let checkbox = if task.is_done() { "[x]" } else { "[ ]" };
                    let estimate = task.estimate.as_ref().map(|e| format!(" ({})", e)).unwrap_or_default();
                    let commit_info = task.linked_commit.as_ref().map(|c| {
                        let s = c.to_string();
                        format!(" -> {}", &s[..7.min(s.len())])
                    }).unwrap_or_default();
                    println!("{} {}{}{}", checkbox, task.subject, estimate, commit_info);

                    if let Some(desc) = &task.description {
                        if !desc.is_empty() {
                            println!("    {}", desc);
                        }
                    }
                }

                let issues: Vec<_> = plan.related_issues().collect();
                if !issues.is_empty() {
                    println!();
                    println!("## Linked Issues");
                    for issue_id in issues {
                        println!("  - {}", issue_id);
                    }
                }

                let patches: Vec<_> = plan.related_patches().collect();
                if !patches.is_empty() {
                    println!();
                    println!("## Linked Patches");
                    for patch_id in patches {
                        println!("  - {}", patch_id);
                    }
                }
            }
        }
        Commands::Status { id, status } => {
            let mut plans = Plans::open(&repo)?;
            let plan_id = resolve_plan_id_from_store(&id, &plans)?;
            let new_status = parse_plan_status(&status);
            let signer = profile.signer()?;

            let mut plan = plans.get_mut(&plan_id)?;
            plan.set_status(new_status, &signer)?;

            println!("Plan {} status set to: {:?}", short_id(&plan_id), new_status);
        }
        Commands::Task { command } => match command {
            TaskCommands::Add { plan_id, subject, description, estimate, files } => {
                let mut plans = Plans::open(&repo)?;
                let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
                let signer = profile.signer()?;

                let mut plan = plans.get_mut(&pid)?;
                plan.add_task(&subject, description, estimate, files, &signer)?;

                println!("Task added to plan {}: {}", short_id(&pid), subject);
            }
            TaskCommands::List { plan_id } => {
                let plans = Plans::open(&repo)?;
                let pid = resolve_plan_id_from_store(&plan_id, &plans)?;

                let Some(plan) = plans.get(&pid)? else {
                    return Err(format!("Plan not found: {plan_id}").into());
                };

                println!("Tasks for plan: {}", plan.title());
                println!();

                for task in plan.tasks() {
                    let checkbox = if task.is_done() { "[x]" } else { "[ ]" };
                    let commit_info = task.linked_commit.as_ref().map(|c| {
                        let s = c.to_string();
                        format!(" -> {}", &s[..7.min(s.len())])
                    }).unwrap_or_default();

                    println!("{} {} ({}){}", checkbox, task.subject, short_id(&task.id.into()), commit_info);
                }
            }
            TaskCommands::LinkCommit { plan_id, task_id, commit } => {
                let mut plans = Plans::open(&repo)?;
                let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
                let tid = resolve_task_id(&task_id)?;
                let oid = resolve_oid(&commit)?;
                let signer = profile.signer()?;

                let mut plan = plans.get_mut(&pid)?;
                plan.link_task_to_commit(tid, oid, &signer)?;

                println!("Task {} linked to commit {}", short_id(&tid.into()), short_id(&oid.into()));
            }
            TaskCommands::Edit { plan_id, task_id, subject, description, estimate, files } => {
                let mut plans = Plans::open(&repo)?;
                let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
                let tid = resolve_task_id(&task_id)?;
                let signer = profile.signer()?;

                let affected_files = if files.is_empty() { None } else { Some(files) };

                let mut plan = plans.get_mut(&pid)?;
                plan.edit_task(
                    tid,
                    subject,
                    description.map(Some),
                    estimate.map(Some),
                    affected_files,
                    &signer,
                )?;

                println!("Task {} updated", short_id(&tid.into()));
            }
            TaskCommands::Remove { plan_id, task_id } => {
                let mut plans = Plans::open(&repo)?;
                let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
                let tid = resolve_task_id(&task_id)?;
                let signer = profile.signer()?;

                let mut plan = plans.get_mut(&pid)?;
                plan.remove_task(tid, &signer)?;

                println!("Task {} removed", short_id(&tid.into()));
            }
            TaskCommands::Link { plan_id, task_id, issue } => {
                let mut plans = Plans::open(&repo)?;
                let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
                let tid = resolve_task_id(&task_id)?;
                let issue_id = resolve_cob_id(&issue)?;
                let signer = profile.signer()?;

                let mut plan = plans.get_mut(&pid)?;
                plan.link_task_to_issue(tid, issue_id, &signer)?;

                println!("Task {} linked to issue {}", short_id(&tid.into()), short_id(&issue_id));
            }
        },
        Commands::Link { plan_id, issue, patch } => {
            let mut plans = Plans::open(&repo)?;
            let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
            let signer = profile.signer()?;

            let mut plan = plans.get_mut(&pid)?;

            if let Some(i) = issue {
                let issue_id = resolve_cob_id(&i)?;
                plan.link_issue(issue_id, &signer)?;
                println!("Linked issue {} to plan {}", short_id(&issue_id), short_id(&pid));
            }
            if let Some(p) = patch {
                let patch_id = resolve_cob_id(&p)?;
                plan.link_patch(patch_id, &signer)?;
                println!("Linked patch {} to plan {}", short_id(&patch_id), short_id(&pid));
            }
        }
        Commands::Unlink { plan_id, issue, patch } => {
            let mut plans = Plans::open(&repo)?;
            let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
            let signer = profile.signer()?;

            let mut plan = plans.get_mut(&pid)?;

            if let Some(i) = issue {
                let issue_id = resolve_cob_id(&i)?;
                plan.unlink_issue(issue_id, &signer)?;
                println!("Unlinked issue {} from plan {}", short_id(&issue_id), short_id(&pid));
            }
            if let Some(p) = patch {
                let patch_id = resolve_cob_id(&p)?;
                plan.unlink_patch(patch_id, &signer)?;
                println!("Unlinked patch {} from plan {}", short_id(&patch_id), short_id(&pid));
            }
        }
        Commands::Comment { plan_id, message, reply_to } => {
            let mut plans = Plans::open(&repo)?;
            let pid = resolve_plan_id_from_store(&plan_id, &plans)?;
            let signer = profile.signer()?;

            let reply_to: Option<CommentId> = reply_to
                .map(|r| resolve_comment_id(&r))
                .transpose()?;

            let mut plan = plans.get_mut(&pid)?;
            plan.comment(&message, reply_to, vec![], &signer)?;

            println!("Comment added to plan {}", short_id(&pid));
        }
        Commands::Export { id, format, output } => {
            let plans = Plans::open(&repo)?;
            let plan_id = resolve_plan_id_from_store(&id, &plans)?;

            let Some(plan) = plans.get(&plan_id)? else {
                return Err(format!("Plan not found: {id}").into());
            };

            let content = match format.as_str() {
                "md" => export_markdown(&plan_id, &plan),
                "json" => serde_json::to_string_pretty(&plan)?,
                _ => return Err(format!("Unknown format: {format}").into()),
            };

            if let Some(path) = output {
                std::fs::write(&path, &content)?;
                println!("Exported to: {}", path.display());
            } else {
                println!("{content}");
            }
        }
        Commands::Edit { id, title, description } => {
            let mut plans = Plans::open(&repo)?;
            let pid = resolve_plan_id_from_store(&id, &plans)?;
            let signer = profile.signer()?;

            let mut plan = plans.get_mut(&pid)?;

            if let Some(t) = title {
                plan.edit_title(&t, &signer)?;
                println!("Plan title updated to: {}", t);
            }
            if let Some(d) = description {
                plan.edit_description(&d, vec![], &signer)?;
                println!("Plan description updated");
            }
        }
    }

    Ok(())
}

/// Parse a COB ID from a string (requires full 40-char hex ID).
/// Used for external COB references (issues, patches).
fn resolve_cob_id(s: &str) -> Result<ObjectId, Box<dyn std::error::Error>> {
    ObjectId::from_str(s).map_err(|e| format!("Invalid ID '{s}': {e}").into())
}

/// Resolve a plan ID prefix against the plans store.
fn resolve_plan_id_from_store<R>(s: &str, plans: &Plans<R>) -> Result<PlanId, Box<dyn std::error::Error>>
where
    R: radicle::prelude::ReadRepository + radicle::cob::Store,
{
    // Try full ID first
    if let Ok(id) = ObjectId::from_str(s) {
        return Ok(id);
    }

    // Validate hex prefix
    let prefix = s.to_lowercase();
    if prefix.is_empty() || !prefix.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Invalid plan ID '{s}': not a valid hex string").into());
    }

    // Search all plans for prefix matches
    let mut matches: Vec<ObjectId> = Vec::new();
    for result in plans.all()? {
        let (id, _plan) = result?;
        if id.to_string().starts_with(&prefix) {
            matches.push(id);
        }
    }

    match matches.len() {
        0 => Err(format!("No plan found matching prefix '{s}'").into()),
        1 => Ok(matches[0]),
        n => {
            let ids: Vec<String> = matches.iter().map(|id| short_id(id)).collect();
            Err(format!(
                "Ambiguous plan ID prefix '{s}': {n} plans match ({})",
                ids.join(", ")
            ).into())
        }
    }
}

/// Parse a task ID from a string.
fn resolve_task_id(s: &str) -> Result<TaskId, Box<dyn std::error::Error>> {
    use radicle::git::Oid;
    let oid = Oid::from_str(s).map_err(|e| format!("Invalid task ID '{s}': {e}"))?;
    Ok(TaskId::from(oid))
}

/// Parse a git OID from a string.
fn resolve_oid(s: &str) -> Result<radicle::git::Oid, Box<dyn std::error::Error>> {
    use radicle::git::Oid;
    Oid::from_str(s).map_err(|e| format!("Invalid OID '{s}': {e}").into())
}

/// Parse a comment ID from a string.
fn resolve_comment_id(s: &str) -> Result<CommentId, Box<dyn std::error::Error>> {
    use radicle::git::Oid;
    let oid = Oid::from_str(s).map_err(|e| format!("Invalid comment ID '{s}': {e}"))?;
    Ok(CommentId::from(oid))
}

/// Get a short form of an object ID.
fn short_id(id: &ObjectId) -> String {
    let s = id.to_string();
    s[..7.min(s.len())].to_string()
}

/// Parse a plan status string.
fn parse_plan_status(s: &str) -> PlanStatus {
    match s.to_lowercase().as_str() {
        "draft" => PlanStatus::Draft,
        "approved" => PlanStatus::Approved,
        "in-progress" | "inprogress" | "in_progress" => PlanStatus::InProgress,
        "completed" | "complete" | "done" => PlanStatus::Completed,
        "archived" | "archive" => PlanStatus::Archived,
        _ => PlanStatus::Draft,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_comment_id_valid() {
        let id = resolve_comment_id("abcdef0000000000000000000000000000000001").unwrap();
        assert_eq!(id.to_string(), "abcdef0000000000000000000000000000000001");
    }

    #[test]
    fn test_resolve_comment_id_invalid() {
        assert!(resolve_comment_id("not-a-valid-oid").is_err());
        assert!(resolve_comment_id("").is_err());
        assert!(resolve_comment_id("abcdef").is_err()); // too short
    }

    #[test]
    fn test_resolve_task_id_valid() {
        let id = resolve_task_id("0000000000000000000000000000000000000000").unwrap();
        assert_eq!(id.to_string(), "0000000000000000000000000000000000000000");
    }

    #[test]
    fn test_resolve_cob_id_valid() {
        let id = resolve_cob_id("abcdef0000000000000000000000000000000001").unwrap();
        assert_eq!(id.to_string(), "abcdef0000000000000000000000000000000001");
    }

    #[test]
    fn test_resolve_cob_id_rejects_prefix() {
        assert!(resolve_cob_id("abcdef").is_err());
    }
}

/// Export a plan as markdown.
fn export_markdown(id: &PlanId, plan: &radicle_plan_cob::Plan) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", plan.title()));
    out.push_str(&format!("**ID:** {}\n", id));
    out.push_str(&format!("**Status:** {:?}\n", plan.status()));
    out.push_str(&format!("**Author:** {}\n\n", plan.author()));

    if !plan.description().is_empty() {
        out.push_str("## Description\n\n");
        out.push_str(plan.description());
        out.push_str("\n\n");
    }

    out.push_str(&format!("## Tasks ({})\n\n", plan.tasks().len()));

    for task in plan.tasks() {
        let checkbox = if task.is_done() { "[x]" } else { "[ ]" };
        let estimate = task.estimate.as_ref().map(|e| format!(" _({})", e)).unwrap_or_default();
        out.push_str(&format!("- {} {}{}\n", checkbox, task.subject, estimate));

        if let Some(desc) = &task.description {
            if !desc.is_empty() {
                out.push_str(&format!("  - {}\n", desc));
            }
        }
    }

    let issues: Vec<_> = plan.related_issues().collect();
    if !issues.is_empty() {
        out.push_str("\n## Linked Issues\n\n");
        for issue_id in issues {
            out.push_str(&format!("- {}\n", issue_id));
        }
    }

    let patches: Vec<_> = plan.related_patches().collect();
    if !patches.is_empty() {
        out.push_str("\n## Linked Patches\n\n");
        for patch_id in patches {
            out.push_str(&format!("- {}\n", patch_id));
        }
    }

    out
}
