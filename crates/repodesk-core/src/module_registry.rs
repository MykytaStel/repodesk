use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BrainModule {
    pub name: &'static str,
    pub layer: &'static str,
    pub status: &'static str,
    pub purpose: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleAudit {
    pub modules_count: usize,
    pub missing_recommended: Vec<&'static str>,
    pub notes: Vec<&'static str>,
}

pub fn list_modules() -> Vec<BrainModule> {
    vec![
        BrainModule {
            name: "project_registry",
            layer: "memory",
            status: "active",
            purpose: "Tracks local projects and active project state.",
        },
        BrainModule {
            name: "task_manager",
            layer: "memory",
            status: "active",
            purpose: "Tracks active task and run folder.",
        },
        BrainModule {
            name: "context_builder",
            layer: "cognition",
            status: "active",
            purpose: "Builds bounded context packs.",
        },
        BrainModule {
            name: "token_budget",
            layer: "metabolism",
            status: "active",
            purpose: "Estimates token usage and warns about waste.",
        },
        BrainModule {
            name: "safety_scanner",
            layer: "immune",
            status: "active",
            purpose: "Detects secrets and unsafe context markers.",
        },
        BrainModule {
            name: "guard",
            layer: "immune",
            status: "active",
            purpose: "Runs preflight before agent use.",
        },
        BrainModule {
            name: "judge",
            layer: "executive",
            status: "active",
            purpose: "Combines guard, budget, and safety into allow/warn/block.",
        },
        BrainModule {
            name: "checks_runner",
            layer: "senses",
            status: "active",
            purpose: "Runs local checks and summarizes failures.",
        },
        BrainModule {
            name: "prompt_generator",
            layer: "language",
            status: "active",
            purpose: "Creates bounded prompts for agents.",
        },
        BrainModule {
            name: "ai_adapters",
            layer: "periphery",
            status: "active",
            purpose: "Represents local and external AI systems.",
        },
        BrainModule {
            name: "access_control",
            layer: "immune",
            status: "active",
            purpose: "Controls which agent can use which peripheral.",
        },
        BrainModule {
            name: "event_journal",
            layer: "memory",
            status: "active",
            purpose: "Records important workflow decisions and actions.",
        },
        BrainModule {
            name: "session_manager",
            layer: "memory",
            status: "active",
            purpose: "Groups work into local sessions.",
        },
        BrainModule {
            name: "desktop_ui",
            layer: "interface",
            status: "active",
            purpose: "Human cockpit for the control brain.",
        },
    ]
}

pub fn recommend_modules(need: &str) -> Vec<BrainModule> {
    let needle = need.to_ascii_lowercase();

    list_modules()
        .into_iter()
        .filter(|module| {
            module.name.contains(&needle)
                || module.layer.contains(&needle)
                || module.purpose.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

pub fn audit_modules() -> ModuleAudit {
    let modules = list_modules();
    // Every registered brain module is active; flag any that regress to a
    // non-active status so the audit still surfaces real gaps.
    let missing_recommended: Vec<&'static str> = modules
        .iter()
        .filter(|module| module.status != "active")
        .map(|module| module.name)
        .collect();

    ModuleAudit {
        modules_count: modules.len(),
        missing_recommended,
        notes: vec![
            "Core modules should not depend on desktop UI.",
            "Agent modules should remain adapters, not the brain itself.",
            "Security/access checks must run before external agent usage.",
        ],
    }
}

pub fn format_modules(modules: &[BrainModule]) -> String {
    let mut output = String::new();
    output.push_str("Brain modules:\n\n");

    for module in modules {
        output.push_str(&format!(
            "- {} [{} / {}]\n  {}\n",
            module.name, module.layer, module.status, module.purpose
        ));
    }

    output
}

pub fn format_module_recommendations(need: &str, modules: &[BrainModule]) -> String {
    if modules.is_empty() {
        return format!("No modules matched need: {need}\n");
    }

    let mut output = format!("Recommended modules for `{need}`:\n\n");
    output.push_str(&format_modules(modules));
    output
}

pub fn format_module_audit(audit: &ModuleAudit) -> String {
    let mut output = String::new();

    output.push_str("Module audit:\n\n");
    output.push_str(&format!("Modules registered: {}\n", audit.modules_count));

    if audit.missing_recommended.is_empty() {
        output.push_str("Missing recommended modules: none\n");
    } else {
        output.push_str("Missing/planned modules:\n");
        for item in &audit.missing_recommended {
            output.push_str(&format!("  - {item}\n"));
        }
    }

    output.push_str("\nNotes:\n");
    for note in &audit.notes {
        output.push_str(&format!("  - {note}\n"));
    }

    output
}
