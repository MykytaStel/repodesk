//! User-authored **playbooks**: named shortcuts that open the surface owning a
//! piece of work (Work / Changes / Orchestrate …). They used to be three
//! hardcoded cards in the desktop with "authoring planned" pinned next to them.
//!
//! Playbooks now live in `playbooks.toml` (the same [`ConfigStore`] pattern as
//! `costs.toml`), seeded with sensible defaults on first run and fully editable:
//! create, update, delete, and import. A playbook never starts an agent by
//! itself — it only routes you to the surface that owns the work, so it stays
//! within RepoDesk's "no hidden runs" guarantee.

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::utils::ConfigStore;

/// One playbook shortcut.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Playbook {
    /// Stable slug, unique within the config.
    pub id: String,
    pub title: String,
    pub summary: String,
    /// The tab id this opens (e.g. `work`, `changes`, `orchestrate`).
    pub target: String,
    /// Human label for where it lands (e.g. "Work / Execute").
    pub destination: String,
    /// What opening it does — kept honest about whether a run starts.
    pub action: String,
    /// The visible result the user should expect.
    pub artifact: String,
    /// Whether following this shortcut will (eventually) start an agent.
    #[serde(default)]
    pub starts_agent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybooksConfig {
    #[serde(default)]
    pub playbooks: Vec<Playbook>,
}

impl Default for PlaybooksConfig {
    fn default() -> Self {
        Self {
            playbooks: vec![
                Playbook {
                    id: "db-migrations".to_string(),
                    title: "Generate DB Migrations".to_string(),
                    summary: "Start the guarded Work flow, then run the agent with review and verification.".to_string(),
                    target: "work".to_string(),
                    destination: "Work / Execute".to_string(),
                    action: "Sets you in the six-phase task flow; the agent starts only when you approve and press Run agent.".to_string(),
                    artifact: "After the run: Review shows changed files, diff, proof, and memory proposals.".to_string(),
                    starts_agent: false,
                },
                Playbook {
                    id: "security-hotspot".to_string(),
                    title: "Security Hotspot Review".to_string(),
                    summary: "Inspect changed files, run RepoPilot, and review blocking findings inline.".to_string(),
                    target: "changes".to_string(),
                    destination: "Changes".to_string(),
                    action: "Opens the current git diff and RepoPilot review surface; it does not run a coding agent.".to_string(),
                    artifact: "You inspect file diffs, findings, and staged/unstaged status.".to_string(),
                    starts_agent: false,
                },
                Playbook {
                    id: "react-scaffold".to_string(),
                    title: "React Component Scaffold".to_string(),
                    summary: "Delegate the scaffold to the orchestrator, then accept or reject its isolated diff.".to_string(),
                    target: "orchestrate".to_string(),
                    destination: "Orchestrate".to_string(),
                    action: "Opens the planner where you preview steps, choose approvals, then run a real or dry run.".to_string(),
                    artifact: "After a real run: run receipt, isolated worktree, diff, checks proof, and proposals.".to_string(),
                    starts_agent: false,
                },
            ],
        }
    }
}

impl ConfigStore for PlaybooksConfig {
    const FILE_NAME: &'static str = "playbooks.toml";
}

fn slugify(value: &str) -> String {
    let lowered = value.trim().to_ascii_lowercase();
    let mapped: String = lowered
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let collapsed = mapped
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "playbook".to_string()
    } else {
        collapsed
    }
}

/// All configured playbooks (seeds the defaults on first run).
pub fn list_playbooks() -> RepoDeskResult<Vec<Playbook>> {
    Ok(PlaybooksConfig::load_config()?.playbooks)
}

/// Create or update a playbook (matched by `id`; a blank id is derived from the
/// title). Returns the full list after the change.
pub fn save_playbook(mut playbook: Playbook) -> RepoDeskResult<Vec<Playbook>> {
    if playbook.title.trim().is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "a playbook needs a title".to_string(),
        });
    }
    if playbook.target.trim().is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "a playbook needs a target surface".to_string(),
        });
    }
    if playbook.id.trim().is_empty() {
        playbook.id = slugify(&playbook.title);
    }

    let mut config = PlaybooksConfig::load_config()?;
    if let Some(existing) = config.playbooks.iter_mut().find(|p| p.id == playbook.id) {
        *existing = playbook;
    } else {
        config.playbooks.push(playbook);
    }
    config.save_config()?;
    Ok(config.playbooks)
}

/// Delete a playbook by id. Returns the full list after the change.
pub fn delete_playbook(id: &str) -> RepoDeskResult<Vec<Playbook>> {
    let mut config = PlaybooksConfig::load_config()?;
    let before = config.playbooks.len();
    config.playbooks.retain(|p| p.id != id);
    if config.playbooks.len() == before {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("no playbook with id '{id}'"),
        });
    }
    config.save_config()?;
    Ok(config.playbooks)
}

/// Import playbooks from a TOML or JSON document (a `{ playbooks = [...] }`
/// table/object, or a bare array), merging by id (imported entries win). Returns
/// the full list after the merge.
pub fn import_playbooks(document: &str) -> RepoDeskResult<Vec<Playbook>> {
    let imported = parse_playbooks(document)?;
    if imported.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "no playbooks found in the imported document".to_string(),
        });
    }
    let mut config = PlaybooksConfig::load_config()?;
    for mut pb in imported {
        if pb.id.trim().is_empty() {
            pb.id = slugify(&pb.title);
        }
        if let Some(existing) = config.playbooks.iter_mut().find(|p| p.id == pb.id) {
            *existing = pb;
        } else {
            config.playbooks.push(pb);
        }
    }
    config.save_config()?;
    Ok(config.playbooks)
}

fn parse_playbooks(document: &str) -> RepoDeskResult<Vec<Playbook>> {
    let trimmed = document.trim();
    if trimmed.is_empty() {
        return Err(RepoDeskError::RoutingFailed {
            detail: "nothing to import".to_string(),
        });
    }
    // Try a full config table first, then a bare array, in both TOML and JSON.
    if let Ok(config) = toml::from_str::<PlaybooksConfig>(trimmed) {
        return Ok(config.playbooks);
    }
    if let Ok(config) = serde_json::from_str::<PlaybooksConfig>(trimmed) {
        return Ok(config.playbooks);
    }
    if let Ok(list) = serde_json::from_str::<Vec<Playbook>>(trimmed) {
        return Ok(list);
    }
    Err(RepoDeskError::RoutingFailed {
        detail: "could not parse playbooks (expected TOML or JSON with a 'playbooks' list)"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes() {
        assert_eq!(slugify("Generate DB Migrations!"), "generate-db-migrations");
        assert_eq!(slugify("  "), "playbook");
    }

    #[test]
    fn parse_accepts_json_array() {
        let doc = r#"[{"id":"x","title":"X","summary":"s","target":"work","destination":"d","action":"a","artifact":"r","starts_agent":false}]"#;
        let parsed = parse_playbooks(doc).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "x");
    }

    #[test]
    fn parse_accepts_toml_table() {
        let doc = r#"
[[playbooks]]
id = "y"
title = "Y"
summary = "s"
target = "changes"
destination = "Changes"
action = "a"
artifact = "r"
"#;
        let parsed = parse_playbooks(doc).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].target, "changes");
    }

    #[test]
    fn default_seeds_three() {
        assert_eq!(PlaybooksConfig::default().playbooks.len(), 3);
    }
}
