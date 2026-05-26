use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;

pub fn desktop_plan() -> String {
    r#"RepoDesk Desktop Plan

Desktop technology:
- Tauri 2 shell
- React + TypeScript UI
- Rust core stays UI-independent
- Tauri commands call repodesk-core
- Tauri events stream check/log progress later

First screens:
1. Dashboard
2. Projects
3. Active Task
4. Context Builder
5. Token Intelligence
6. Agents / AI Adapters
7. Checks
8. Security

Rule:
The desktop app must not bypass core guardrails.
"#
    .to_string()
}

pub fn tauri_bridge_spec() -> String {
    r#"Tauri bridge specification v0.1

Commands:
- repodesk_init()
- project_list()
- project_use(name)
- task_show()
- context_build()
- context_estimate()
- prompt_generate(kind)
- checks_run()
- guard_preflight(agent)
- brain_status()
- ui_snapshot()

Events later:
- checks_output_line
- checks_finished
- context_progress
- token_warning
- guard_blocked

Security:
- UI calls core only through typed commands.
- No direct shell access from React.
- No unrestricted filesystem access from UI.
- Sensitive files are blocked by policy before reaching agents.
"#
    .to_string()
}

pub fn desktop_events_spec() -> String {
    r#"Desktop event model v0.1

Core events planned:
- TaskChanged
- ContextBuilt
- TokenBudgetWarning
- GuardBlocked
- ChecksStarted
- ChecksOutputLine
- ChecksFinished
- PromptGenerated

MVP approach:
- CLI first
- JSON snapshot for UI polling
- Tauri events later for live logs
"#
    .to_string()
}

pub fn desktop_scaffold_hint() -> RepoDeskResult<String> {
    init::init_home()?;
    let paths = RepoDeskPaths::resolve()?;

    Ok(format!(
        r#"Desktop scaffold hint:

RepoDesk home:
  {}

Suggested workspace layout:
  apps/desktop/
    package.json
    src/
    src-tauri/

Recommended command later:
  pnpm create tauri-app apps/desktop

Do not move core logic into desktop.
Keep all orchestration in repodesk-core.
"#,
        paths.home.display()
    ))
}
