//! Render the active brain to a human-readable `memory.md` artifact.
//!
//! This replaces the original naive group-by-category dump. It now includes
//! only `active` entries, ranks them (pinned/recency/salience), and records
//! provenance + tags. An optional Ollama summarization pass is layered on in a
//! later phase; today it is fully deterministic.

use std::collections::BTreeMap;

use crate::errors::RepoDeskResult;
use crate::paths::RepoDeskPaths;

use super::model::MemoryEntry;
use super::retrieval::{TaskSignals, rank_for_task};
use super::store;

/// Build `memory.md` for a project from active entries and return the markdown.
pub fn consolidate_project_memory(project_name: &str) -> RepoDeskResult<String> {
    let entries = store::list_active(project_name)?;
    let markdown = render(project_name, &entries);

    let paths = RepoDeskPaths::resolve()?;
    let project_dir = paths.project_dir(project_name);
    std::fs::create_dir_all(&project_dir)?;
    std::fs::write(project_dir.join("memory.md"), &markdown)?;

    Ok(markdown)
}

fn render(project_name: &str, entries: &[MemoryEntry]) -> String {
    let mut markdown = format!(
        "# {project_name} Memory\n\n\
         Project-specific decisions, constraints, risks, and patterns curated by the RepoDesk \
         Memory Brain. Active entries only.\n"
    );

    if entries.is_empty() {
        markdown.push_str(
            "\n_No memory entries yet. Capture decisions and constraints so every agent shares \
             the same context._\n",
        );
        return markdown;
    }

    // Rank within each category so the most relevant/important float to the top.
    let ranked = rank_for_task(&TaskSignals::default(), entries);

    let mut categories: BTreeMap<String, Vec<MemoryEntry>> = BTreeMap::new();
    for scored in ranked {
        categories
            .entry(scored.entry.category.clone())
            .or_default()
            .push(scored.entry);
    }

    for (category, cat_entries) in categories {
        markdown.push_str(&format!("\n## {category}\n\n"));
        for entry in cat_entries {
            let pin = if entry.pinned { " [pinned]" } else { "" };
            let tags_str = if entry.tags.is_empty() {
                String::new()
            } else {
                format!(" (Tags: {})", entry.tags.join(", "))
            };
            markdown.push_str(&format!(
                "- **[{provenance}]**{pin}{tags_str}:\n  {content}\n\n",
                provenance = entry.provenance(),
                content = entry.content.replace('\n', "\n  ").trim_end(),
            ));
        }
    }

    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consolidate_groups_by_category_with_tags() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_home = std::env::temp_dir().join(format!("repodesk-consolidate-{now}"));
        std::fs::create_dir_all(&test_home).unwrap();
        unsafe {
            std::env::set_var("REPODESK_HOME", &test_home);
        }
        crate::init::init_home().unwrap();

        let project = "test_memory_project";
        store::add_memory(
            project,
            "First note about design patterns",
            "Design",
            &["architecture".into(), "backend".into()],
        )
        .unwrap();
        store::add_memory(
            project,
            "Second note about schema design",
            "Database",
            &["db".into()],
        )
        .unwrap();
        store::add_memory(project, "Third note without tags", "Design", &[]).unwrap();

        let md = consolidate_project_memory(project).unwrap();

        assert!(md.contains("# test_memory_project Memory"));
        assert!(md.contains("## Database"));
        assert!(md.contains("## Design"));
        assert!(md.contains("First note about design patterns"));
        assert!(md.contains("Second note about schema design"));
        assert!(md.contains("Third note without tags"));
        assert!(md.contains("(Tags: architecture, backend)"));
        assert!(md.contains("(Tags: db)"));

        let paths = RepoDeskPaths::resolve().unwrap();
        let expected_file = paths.project_dir(project).join("memory.md");
        assert!(expected_file.exists());
        assert_eq!(std::fs::read_to_string(expected_file).unwrap(), md);

        let _ = std::fs::remove_dir_all(&test_home);
    }
}
