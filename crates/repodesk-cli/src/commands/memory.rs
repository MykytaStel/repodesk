use crate::cli::MemoryCommand;
use anyhow::Result;

pub fn handle_memory_command(command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Add {
            project,
            content,
            category,
            tags,
        } => {
            let project_name = match project {
                Some(p) => p,
                None => repodesk_core::projects::read_active_project()?,
            };
            let entry = repodesk_core::persistence::db::add_memory(&project_name, &content, &category, &tags)?;
            println!("Memory entry added successfully (ID: {}).", entry.id);
        }
        MemoryCommand::List { project } => {
            let project_name = match project {
                Some(p) => p,
                None => repodesk_core::projects::read_active_project()?,
            };
            let entries = repodesk_core::persistence::db::list_memory(&project_name)?;
            if entries.is_empty() {
                println!("No memory entries for project '{}'.", project_name);
            } else {
                println!("Memory entries for project '{}':", project_name);
                for entry in entries {
                    let tags_str = if entry.tags.is_empty() {
                        "".to_string()
                    } else {
                        format!(" (Tags: {})", entry.tags.join(", "))
                    };
                    println!(
                        "[{}] [{}] (ID: {}){}:\n  {}",
                        entry.timestamp.to_rfc3339(),
                        entry.category,
                        entry.id,
                        tags_str,
                        entry.content.replace("\n", "\n  ")
                    );
                }
            }
        }
        MemoryCommand::Consolidate { project } => {
            let project_name = match project {
                Some(p) => p,
                None => repodesk_core::projects::read_active_project()?,
            };
            let markdown = repodesk_core::persistence::db::consolidate_project_memory(&project_name)?;
            println!("Memory consolidated successfully for project '{}'!", project_name);
            println!("Written to memory.md ({} bytes).", markdown.len());
        }
    }
    Ok(())
}
