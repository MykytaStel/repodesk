use super::CommandResult;
use repodesk_core::errors::RepoDeskResult;
use repodesk_core::workflow::DesktopAction;

/// Run a whitelisted desktop action by calling `repodesk-core` services
/// directly and returning a typed [`CommandResult`].
///
/// This replaces the deprecated in-process CLI dispatch (`run_cli`), which
/// reparsed argv with clap and captured the result through a process-global
/// stdout/stderr override (the historical flaky-test source). Each action now
/// maps to an explicit core call; the catalog itself is the allowlist, so no
/// separate subcommand allowlist or shell parsing is involved.
pub(crate) async fn run_action(action: &DesktopAction) -> CommandResult {
    let command = action.command_preview.clone();

    match action.id.as_str() {
        "workflow-next" => finish(command, repodesk_core::workflow::workflow_next()),
        "doctor-workflow" => finish(
            command,
            repodesk_core::workflow_doctor::diagnose_workflow().map(|report| {
                repodesk_core::workflow_doctor::format_workflow_doctor_report(&report)
            }),
        ),
        "context-build" => finish(command, build_context_text()),
        "smart-context-build" => match repodesk_core::smart_context::build_smart_context().await {
            Ok(result) => ok(
                command,
                repodesk_core::smart_context::format_smart_context_result(&result),
            ),
            Err(error) => err(command, error.to_string()),
        },
        "prompt-all" => finish(command, prompt_all_text()),
        "safety-scan-context" => finish(
            command,
            repodesk_core::safety::scan_active_context()
                .map(|report| repodesk_core::safety::format_safety_report(&report)),
        ),
        "security-audit" => finish(
            command,
            repodesk_core::security::audit_security_policy()
                .map(|audit| repodesk_core::security::format_security_audit(&audit)),
        ),
        "judge-codex" => finish(command, judge_text("codex")),
        "judge-chatgpt" => finish(command, judge_text("chatgpt")),
        "runtime-route-patch" => ok(command, route_text("patch")),
        "runtime-route-compression" => ok(command, route_text("compression")),
        "checks-run" => finish(command, checks_run_text()),
        "git-audit" => finish(command, repodesk_core::git_audit::git_audit()),
        other => err(
            command,
            format!("Blocked: action '{other}' is not registered or allowed."),
        ),
    }
}

fn ok(command: String, stdout: String) -> CommandResult {
    CommandResult {
        ok: true,
        command,
        stdout,
        stderr: String::new(),
        exit_code: Some(0),
    }
}

fn err(command: String, stderr: String) -> CommandResult {
    CommandResult {
        ok: false,
        command,
        stdout: String::new(),
        stderr,
        exit_code: Some(1),
    }
}

fn finish(command: String, outcome: RepoDeskResult<String>) -> CommandResult {
    match outcome {
        Ok(stdout) => ok(command, stdout),
        Err(error) => err(command, error.to_string()),
    }
}

fn build_context_text() -> RepoDeskResult<String> {
    let result = repodesk_core::context::build_context()?;
    let config = repodesk_core::usage::budget::load_budget_config()?;
    let verdict = repodesk_core::usage::budget::evaluate_context(&result.estimate, &config);

    let mut out = String::new();
    out.push_str("Context built:\n");
    out.push_str(&format!("  context file: {}\n", result.context_file));
    out.push_str(&format!(
        "  token estimate file: {}\n\n",
        result.token_estimate_file
    ));
    out.push_str(&repodesk_core::tokens::format_estimate(&result.estimate));
    out.push_str(&repodesk_core::usage::budget::format_verdict(&verdict));
    out.push('\n');
    Ok(out)
}

fn prompt_all_text() -> RepoDeskResult<String> {
    use repodesk_core::prompts::{PromptKind, generate_prompt};

    let mut out = String::new();
    for kind in [PromptKind::Codex, PromptKind::ChatGpt, PromptKind::Review] {
        let result = generate_prompt(kind)?;
        out.push_str("Prompt generated:\n");
        out.push_str(&format!("  kind: {}\n", result.prompt_kind.label()));
        out.push_str(&format!("  file: {}\n", result.prompt_file.display()));
        out.push_str(&format!("  context tokens: {}\n", result.estimated_tokens));
    }
    Ok(out)
}

fn judge_text(agent: &str) -> RepoDeskResult<String> {
    let report = repodesk_core::judge::judge_agent(agent)?;
    Ok(repodesk_core::judge::format_judgement(&report))
}

fn route_text(need: &str) -> String {
    let budget = repodesk_core::usage::budget::load_budget_config().unwrap_or_default();
    let request = repodesk_core::routing::route_request_for_need(need);
    let capacities = repodesk_core::routing::default_capacities(&budget);
    let decision = repodesk_core::routing::route_request(&request, &capacities, &budget);
    repodesk_core::routing::format_route_decision(&decision)
}

fn checks_run_text() -> RepoDeskResult<String> {
    let result = repodesk_core::checks::run_checks()?;

    let mut out = String::new();
    out.push_str("Checks finished:\n");
    out.push_str(&format!(
        "  status: {}\n",
        if result.success { "passed" } else { "failed" }
    ));
    out.push_str(&format!("  log: {}\n", result.log_file.display()));
    out.push_str(&format!("  summary: {}\n\n", result.summary_file.display()));
    for command in result.commands {
        out.push_str(&format!(
            "  - {} => {} ({:?})\n",
            command.command, command.status, command.exit_code
        ));
    }
    Ok(out)
}
