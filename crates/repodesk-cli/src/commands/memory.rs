use std::io::Read;

use anyhow::{Result, anyhow};

use crate::cli::MemoryCommand;
use repodesk_core::memory::model::{MemoryProposal, proposal_kind};
use repodesk_core::memory::{self, store};

fn resolve_project(project: Option<String>) -> Result<String> {
    match project {
        Some(p) => Ok(p),
        None => Ok(repodesk_core::projects::read_active_project()?),
    }
}

/// Active task id, best-effort (empty when no active task).
fn active_task_id() -> String {
    repodesk_core::tasks::show_active_task()
        .map(|t| t.config.id)
        .unwrap_or_default()
}

pub fn handle_memory_command(command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Add {
            project,
            content,
            category,
            tags,
        } => {
            let project = resolve_project(project)?;
            let entry = store::add_memory(&project, &content, &category, &tags)?;
            println!("Memory entry added successfully (ID: {}).", entry.id);
        }
        MemoryCommand::List { project } => {
            let project = resolve_project(project)?;
            let entries = store::list_memory(&project)?;
            if entries.is_empty() {
                println!("No memory entries for project '{}'.", project);
            } else {
                println!("Memory entries for project '{}':", project);
                for entry in entries {
                    print_entry(&entry);
                }
            }
        }
        MemoryCommand::Search { project, query } => {
            let project = resolve_project(project)?;
            let entries = store::search_entries(&project, &query)?;
            if entries.is_empty() {
                println!("No entries match '{query}' in project '{project}'.");
            } else {
                println!("{} match(es) for '{query}':", entries.len());
                for entry in entries {
                    print_entry(&entry);
                }
            }
        }
        MemoryCommand::Consolidate { project } => {
            let project = resolve_project(project)?;
            let markdown = memory::consolidate_project_memory(&project)?;
            println!("Memory consolidated for project '{}'.", project);
            println!("Written to memory.md ({} bytes).", markdown.len());
        }
        MemoryCommand::Capture {
            project,
            agent,
            file,
            content,
        } => {
            let project = resolve_project(project)?;
            let text = read_capture_text(file, content)?;
            let task_id = active_task_id();
            let proposals = memory::capture_from_text(&project, &task_id, &agent, &text)?;
            if proposals.is_empty() {
                println!("No new memory candidates found in the {agent} response.");
            } else {
                println!(
                    "Captured {} candidate(s) from {agent} as pending proposals:",
                    proposals.len()
                );
                for p in &proposals {
                    print_proposal(p);
                }
                println!("\nReview with `repodesk memory review`, then accept/reject by id.");
            }
        }
        MemoryCommand::Scan { project } => {
            let project = resolve_project(project)?;
            let summary = memory::scan(&project)?;
            println!(
                "Scan complete: {} dedup, {} merge, {} conflict proposal(s) created.",
                summary.dedup, summary.merge, summary.conflict
            );
            for p in &summary.created {
                print_proposal(p);
            }
            if summary.total() == 0 {
                println!("No new issues found.");
            }
        }
        MemoryCommand::Review { project, all } => {
            let project = resolve_project(project)?;
            let status = if all { None } else { Some("pending") };
            let proposals = store::list_proposals(&project, status)?;
            if proposals.is_empty() {
                let scope = if all { "" } else { "pending " };
                println!("No {scope}proposals for '{project}'.");
            } else {
                println!("{} proposal(s):", proposals.len());
                for p in proposals {
                    print_proposal(&p);
                }
            }
        }
        MemoryCommand::Accept { id, keep } => {
            let proposal = memory::accept_proposal(id, keep)?;
            println!(
                "Accepted {} proposal {id}. Applied entry id: {}.",
                proposal.kind,
                proposal
                    .applied_entry_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "n/a".to_string())
            );
        }
        MemoryCommand::Reject { id } => {
            memory::reject_proposal(id)?;
            println!("Rejected proposal {id}. Brain unchanged.");
        }
        MemoryCommand::Pin { id } => {
            store::set_pinned(id, true)?;
            println!("Pinned entry {id}.");
        }
        MemoryCommand::Unpin { id } => {
            store::set_pinned(id, false)?;
            println!("Unpinned entry {id}.");
        }
        MemoryCommand::Archive { id } => {
            store::set_status(id, "archived")?;
            println!("Archived entry {id} (excluded from context).");
        }
        MemoryCommand::Delete { id } => {
            store::delete_entry(id)?;
            println!("Deleted entry {id}.");
        }
    }
    Ok(())
}

fn read_capture_text(file: Option<std::path::PathBuf>, content: Option<String>) -> Result<String> {
    if let Some(content) = content {
        return Ok(content);
    }
    if let Some(path) = file {
        return std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("could not read {}: {e}", path.display()));
    }
    // Fall back to stdin so a response can be piped in.
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    if buf.trim().is_empty() {
        return Err(anyhow!(
            "no text provided — pass --content, --file <path>, or pipe via stdin"
        ));
    }
    Ok(buf)
}

fn print_entry(entry: &repodesk_core::memory::MemoryEntry) {
    let tags = if entry.tags.is_empty() {
        String::new()
    } else {
        format!(" (tags: {})", entry.tags.join(", "))
    };
    let pin = if entry.pinned { " [pinned]" } else { "" };
    println!(
        "[{}] [{}/{}] (id {}) {}{}{}:\n  {}",
        entry.timestamp.to_rfc3339(),
        entry.category,
        entry.status,
        entry.id,
        entry.provenance(),
        pin,
        tags,
        entry.content.replace('\n', "\n  ")
    );
}

fn print_proposal(p: &MemoryProposal) {
    let label = match p.kind.as_str() {
        proposal_kind::CAPTURE => "capture",
        proposal_kind::DEDUP => "dedup",
        proposal_kind::MERGE => "merge",
        proposal_kind::CONFLICT => "conflict",
        other => other,
    };
    println!(
        "  #{} [{}] {} — {}",
        p.id, label, p.status, p.payload.rationale
    );
    if !p.payload.source_ids.is_empty() {
        println!(
            "      source entries: {}",
            p.payload
                .source_ids
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(proposed) = &p.payload.proposed {
        println!(
            "      proposed [{}]: {}",
            proposed.category, proposed.content
        );
    }
}
