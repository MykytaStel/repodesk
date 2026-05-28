use super::types::DesktopAction;

pub fn action_catalog() -> Vec<DesktopAction> {
    vec![
        DesktopAction {
            id: "workflow-next".into(),
            title: "Decide next workflow step".into(),
            description:
                "Ask the workflow brain what should happen next based on current project state."
                    .into(),
            category: "Brain".into(),
            risk: "safe".into(),
            command_preview: "repodesk workflow next".into(),
            args: vec!["workflow".into(), "next".into()],
        },
        DesktopAction {
            id: "doctor-workflow".into(),
            title: "Run workflow doctor".into(),
            description: "Check project, task, context, prompts, checks, and guard state.".into(),
            category: "Brain".into(),
            risk: "safe".into(),
            command_preview: "repodesk doctor workflow".into(),
            args: vec!["doctor".into(), "workflow".into()],
        },
        DesktopAction {
            id: "context-build".into(),
            title: "Build context".into(),
            description: "Build the full active task context.md artifact.".into(),
            category: "Context".into(),
            risk: "safe".into(),
            command_preview: "repodesk context build".into(),
            args: vec!["context".into(), "build".into()],
        },
        DesktopAction {
            id: "smart-context-build".into(),
            title: "Build smart context".into(),
            description: "Build a smaller task-focused context pack to reduce token waste.".into(),
            category: "Context".into(),
            risk: "safe".into(),
            command_preview: "repodesk smart-context build".into(),
            args: vec!["smart-context".into(), "build".into()],
        },
        DesktopAction {
            id: "prompt-all".into(),
            title: "Generate agent prompts".into(),
            description: "Generate Codex, ChatGPT, and review prompts for the active task.".into(),
            category: "Agents".into(),
            risk: "safe".into(),
            command_preview: "repodesk prompt all".into(),
            args: vec!["prompt".into(), "all".into()],
        },
        DesktopAction {
            id: "safety-scan-context".into(),
            title: "Scan context for secrets".into(),
            description: "Scan active context for token/secret/password/private key risk signals."
                .into(),
            category: "Security".into(),
            risk: "safe".into(),
            command_preview: "repodesk safety scan-context".into(),
            args: vec!["safety".into(), "scan-context".into()],
        },
        DesktopAction {
            id: "security-audit".into(),
            title: "Run security audit".into(),
            description: "Show RepoDesk security policy and risk warnings.".into(),
            category: "Security".into(),
            risk: "safe".into(),
            command_preview: "repodesk security audit".into(),
            args: vec!["security".into(), "audit".into()],
        },
        DesktopAction {
            id: "judge-codex".into(),
            title: "Judge Codex readiness".into(),
            description: "Run guard, safety, and budget judgement for Codex.".into(),
            category: "Judge".into(),
            risk: "safe".into(),
            command_preview: "repodesk judge agent --agent codex".into(),
            args: vec![
                "judge".into(),
                "agent".into(),
                "--agent".into(),
                "codex".into(),
            ],
        },
        DesktopAction {
            id: "judge-chatgpt".into(),
            title: "Judge ChatGPT readiness".into(),
            description: "Run guard, safety, and budget judgement for ChatGPT.".into(),
            category: "Judge".into(),
            risk: "safe".into(),
            command_preview: "repodesk judge agent --agent chatgpt".into(),
            args: vec![
                "judge".into(),
                "agent".into(),
                "--agent".into(),
                "chatgpt".into(),
            ],
        },
        DesktopAction {
            id: "runtime-route-patch".into(),
            title: "Route patch work".into(),
            description: "Ask runtime router which provider should handle patch work.".into(),
            category: "Runtime".into(),
            risk: "safe".into(),
            command_preview: "repodesk runtime route --need patch".into(),
            args: vec![
                "runtime".into(),
                "route".into(),
                "--need".into(),
                "patch".into(),
            ],
        },
        DesktopAction {
            id: "runtime-route-compression".into(),
            title: "Route compression work".into(),
            description: "Ask runtime router which provider should compress context.".into(),
            category: "Runtime".into(),
            risk: "safe".into(),
            command_preview: "repodesk runtime route --need compression".into(),
            args: vec![
                "runtime".into(),
                "route".into(),
                "--need".into(),
                "compression".into(),
            ],
        },
        DesktopAction {
            id: "checks-run".into(),
            title: "Run configured checks".into(),
            description: "Run active project checks and save checks-summary.md.".into(),
            category: "Checks".into(),
            risk: "bounded-write".into(),
            command_preview: "repodesk checks run".into(),
            args: vec!["checks".into(), "run".into()],
        },
        DesktopAction {
            id: "git-audit".into(),
            title: "Audit git backup state".into(),
            description: "Review branch, remotes, and backup plan before pushing.".into(),
            category: "Backup".into(),
            risk: "safe".into(),
            command_preview: "repodesk git audit".into(),
            args: vec!["git".into(), "audit".into()],
        },
    ]
}

pub fn find_action(action_id: &str) -> Option<DesktopAction> {
    action_catalog()
        .into_iter()
        .find(|action| action.id == action_id)
}
