use std::env;
use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::paths::RepoDeskPaths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiProbeStatus {
    Available,
    Missing,
    Maybe,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiProbeCategory {
    LocalRuntime,
    PaidAgent,
    CliAgent,
    Editor,
    DesktopApp,
    RuntimeDependency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolProbe {
    pub id: String,
    pub name: String,
    pub category: AiProbeCategory,
    pub status: AiProbeStatus,
    pub detection: String,
    pub executable_path: Option<String>,
    pub app_path: Option<String>,
    pub local_only: bool,
    pub requires_paid_account: bool,
    pub risk_level: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEndpointProbe {
    pub id: String,
    pub name: String,
    pub url: String,
    pub status: AiProbeStatus,
    pub local_only: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDiscoveryReport {
    pub generated_at: DateTime<Utc>,
    pub host_os: String,
    pub tools: Vec<AiToolProbe>,
    pub endpoints: Vec<AiEndpointProbe>,
    pub recommendations: Vec<String>,
    pub warnings: Vec<String>,
    pub report_path: Option<String>,
}

pub fn discover_ai_systems() -> RepoDeskResult<AiDiscoveryReport> {
    let host_os = env::consts::OS.to_string();
    let mut tools = Vec::new();

    tools.extend(discover_cli_tools());
    tools.extend(discover_desktop_apps());

    let endpoints = discover_local_endpoints();
    let mut recommendations = build_recommendations(&tools, &endpoints);
    let warnings = build_warnings(&tools, &endpoints);

    if recommendations.is_empty() {
        recommendations.push(
            "No AI runtime was confidently detected. Install or start Ollama first for local-first workflows."
                .to_string(),
        );
    }

    Ok(AiDiscoveryReport {
        generated_at: Utc::now(),
        host_os,
        tools,
        endpoints,
        recommendations,
        warnings,
        report_path: None,
    })
}

pub fn write_ai_discovery_report() -> RepoDeskResult<AiDiscoveryReport> {
    let mut report = discover_ai_systems()?;
    crate::init::init_home()?;
    let paths = RepoDeskPaths::resolve()?;
    let state_dir = paths.home.join("state");
    fs::create_dir_all(&state_dir)?;

    let json_path = state_dir.join("ai-discovery.json");
    let md_path = state_dir.join("ai-discovery.md");

    report.report_path = Some(json_path.display().to_string());

    fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
    fs::write(&md_path, format_ai_discovery_report(&report))?;

    Ok(report)
}

pub fn format_ai_discovery_report(report: &AiDiscoveryReport) -> String {
    let mut out = String::new();
    out.push_str("# RepoDesk AI Discovery Report\n\n");
    out.push_str(&format!("Generated: {}\n\n", report.generated_at));
    out.push_str(&format!("Host OS: {}\n\n", report.host_os));

    out.push_str("## Detected tools\n\n");
    for tool in &report.tools {
        out.push_str(&format!(
            "- **{}** (`{}`): {:?}\n",
            tool.name, tool.id, tool.status
        ));
        out.push_str(&format!("  - category: {:?}\n", tool.category));
        out.push_str(&format!("  - detection: {}\n", tool.detection));
        if let Some(path) = &tool.executable_path {
            out.push_str(&format!("  - executable: `{}`\n", path));
        }
        if let Some(path) = &tool.app_path {
            out.push_str(&format!("  - app: `{}`\n", path));
        }
        out.push_str(&format!("  - local only: {}\n", tool.local_only));
        out.push_str(&format!(
            "  - paid account: {}\n",
            tool.requires_paid_account
        ));
        out.push_str(&format!("  - risk: {}\n", tool.risk_level));
        for note in &tool.notes {
            out.push_str(&format!("  - note: {}\n", note));
        }
    }

    out.push_str("\n## Local endpoints\n\n");
    for endpoint in &report.endpoints {
        out.push_str(&format!(
            "- **{}** (`{}`): {:?} at `{}`\n",
            endpoint.name, endpoint.id, endpoint.status, endpoint.url
        ));
        for note in &endpoint.notes {
            out.push_str(&format!("  - note: {}\n", note));
        }
    }

    out.push_str("\n## Recommendations\n\n");
    for recommendation in &report.recommendations {
        out.push_str(&format!("- {}\n", recommendation));
    }

    out.push_str("\n## Warnings\n\n");
    for warning in &report.warnings {
        out.push_str(&format!("- {}\n", warning));
    }

    out
}

fn discover_cli_tools() -> Vec<AiToolProbe> {
    let specs = [
        (
            "ollama",
            "Ollama",
            AiProbeCategory::LocalRuntime,
            true,
            false,
            "low",
            vec!["Best default for local compression, summaries, and private context reduction."],
        ),
        (
            "codex",
            "Codex CLI",
            AiProbeCategory::PaidAgent,
            false,
            true,
            "medium",
            vec!["Use only after context, budget, safety scan, and judge verdict."],
        ),
        (
            "gemini",
            "Gemini CLI",
            AiProbeCategory::PaidAgent,
            false,
            true,
            "medium",
            vec!["Useful as secondary review/runtime, but still requires bounded context."],
        ),
        (
            "claude",
            "Claude CLI",
            AiProbeCategory::PaidAgent,
            false,
            true,
            "medium",
            vec!["Treat as paid/external unless explicitly configured as local gateway."],
        ),
        (
            "aider",
            "Aider",
            AiProbeCategory::CliAgent,
            false,
            false,
            "medium",
            vec!["Patch-capable agent. Must be restricted to allowlisted files and branches."],
        ),
        (
            "opencode",
            "OpenCode",
            AiProbeCategory::CliAgent,
            false,
            false,
            "medium",
            vec!["Patch-capable agent. Run only behind RepoDesk guardrails."],
        ),
        (
            "docker",
            "Docker",
            AiProbeCategory::RuntimeDependency,
            true,
            false,
            "medium",
            vec!["Useful for isolated tools, but containers need explicit volume/network policy."],
        ),
        (
            "node",
            "Node.js",
            AiProbeCategory::RuntimeDependency,
            true,
            false,
            "low",
            vec!["Needed for the desktop frontend and some agent tooling."],
        ),
        (
            "python3",
            "Python 3",
            AiProbeCategory::RuntimeDependency,
            true,
            false,
            "low",
            vec!["Useful for local scripts and future ML/tooling integrations."],
        ),
    ];

    specs
        .into_iter()
        .map(|(id, name, category, local_only, paid, risk, notes)| {
            let path = find_executable(id);
            AiToolProbe {
                id: id.to_string(),
                name: name.to_string(),
                category,
                status: if path.is_some() {
                    AiProbeStatus::Available
                } else {
                    AiProbeStatus::Missing
                },
                detection: "passive PATH lookup; no arbitrary shell execution".to_string(),
                executable_path: path.map(|p| p.display().to_string()),
                app_path: None,
                local_only,
                requires_paid_account: paid,
                risk_level: risk.to_string(),
                notes: notes.into_iter().map(str::to_string).collect(),
            }
        })
        .collect()
}

fn discover_desktop_apps() -> Vec<AiToolProbe> {
    let mut probes = Vec::new();

    let specs = [
        (
            "ollama_app",
            "Ollama.app",
            AiProbeCategory::DesktopApp,
            true,
            false,
            "low",
            vec!["Local-first model runtime."],
            vec!["/Applications/Ollama.app"],
        ),
        (
            "lm_studio",
            "LM Studio",
            AiProbeCategory::DesktopApp,
            true,
            false,
            "low",
            vec!["Local model runtime; often exposes OpenAI-compatible local server on port 1234."],
            vec!["/Applications/LM Studio.app"],
        ),
        (
            "chatgpt_app",
            "ChatGPT Desktop",
            AiProbeCategory::DesktopApp,
            false,
            true,
            "medium",
            vec!["External paid/free AI app. Do not send raw repo context automatically."],
            vec!["/Applications/ChatGPT.app"],
        ),
        (
            "cursor",
            "Cursor",
            AiProbeCategory::Editor,
            false,
            true,
            "medium",
            vec!["AI editor. Use with bounded tasks and explicit diff review."],
            vec!["/Applications/Cursor.app"],
        ),
        (
            "zed",
            "Zed",
            AiProbeCategory::Editor,
            true,
            false,
            "low",
            vec!["Editor that can connect to local/remote assistants depending on config."],
            vec!["/Applications/Zed.app"],
        ),
        (
            "vscode",
            "Visual Studio Code",
            AiProbeCategory::Editor,
            false,
            false,
            "low",
            vec!["Editor host for extensions; extension permissions must be managed separately."],
            vec!["/Applications/Visual Studio Code.app"],
        ),
    ];

    for (id, name, category, local_only, paid, risk, notes, paths) in specs {
        let found = paths.iter().map(Path::new).find(|p| p.exists());
        probes.push(AiToolProbe {
            id: id.to_string(),
            name: name.to_string(),
            category,
            status: if found.is_some() {
                AiProbeStatus::Available
            } else {
                AiProbeStatus::Missing
            },
            detection: "passive filesystem app lookup; no app execution".to_string(),
            executable_path: None,
            app_path: found.map(|p| p.display().to_string()),
            local_only,
            requires_paid_account: paid,
            risk_level: risk.to_string(),
            notes: notes.into_iter().map(str::to_string).collect(),
        });
    }

    probes
}

fn discover_local_endpoints() -> Vec<AiEndpointProbe> {
    let specs = [
        (
            "ollama_api",
            "Ollama local API",
            "127.0.0.1:11434",
            "http://127.0.0.1:11434",
            vec!["Used for local model discovery and local inference routing."],
        ),
        (
            "lm_studio_api",
            "LM Studio local API",
            "127.0.0.1:1234",
            "http://127.0.0.1:1234",
            vec!["Common OpenAI-compatible local endpoint for LM Studio."],
        ),
    ];

    specs
        .into_iter()
        .map(|(id, name, socket, url, notes)| {
            let status = if is_local_port_open(socket) {
                AiProbeStatus::Available
            } else {
                AiProbeStatus::Missing
            };
            AiEndpointProbe {
                id: id.to_string(),
                name: name.to_string(),
                url: url.to_string(),
                status,
                local_only: true,
                notes: notes.into_iter().map(str::to_string).collect(),
            }
        })
        .collect()
}

fn build_recommendations(tools: &[AiToolProbe], endpoints: &[AiEndpointProbe]) -> Vec<String> {
    let mut recommendations = Vec::new();

    let ollama_available =
        is_tool_available(tools, "ollama") || is_endpoint_available(endpoints, "ollama_api");
    let lm_studio_available =
        is_tool_available(tools, "lm_studio") || is_endpoint_available(endpoints, "lm_studio_api");
    let codex_available = is_tool_available(tools, "codex");
    let aider_available = is_tool_available(tools, "aider");
    let opencode_available = is_tool_available(tools, "opencode");

    if ollama_available {
        recommendations.push(
            "Use Ollama as the default local-first runtime for context compression and safe draft summaries."
                .to_string(),
        );
    }

    if lm_studio_available {
        recommendations.push(
            "LM Studio is available; consider it as an OpenAI-compatible local fallback runtime."
                .to_string(),
        );
    }

    if codex_available {
        recommendations.push(
            "Codex CLI is available; route patch work to Codex only after safety scan, budget check, and judge verdict."
                .to_string(),
        );
    }

    if aider_available || opencode_available {
        recommendations.push(
            "Patch-capable local CLI agents are available; use allowlisted file scopes and require diff review before commit."
                .to_string(),
        );
    }

    recommendations
}

fn build_warnings(tools: &[AiToolProbe], _endpoints: &[AiEndpointProbe]) -> Vec<String> {
    let mut warnings = Vec::new();

    if tools
        .iter()
        .any(|tool| tool.status == AiProbeStatus::Available && tool.requires_paid_account)
    {
        warnings.push(
            "Paid/external AI tools were detected. RepoDesk should never send full repo context to them by default."
                .to_string(),
        );
    }

    if tools.iter().any(|tool| {
        tool.status == AiProbeStatus::Available
            && matches!(
                tool.category,
                AiProbeCategory::CliAgent | AiProbeCategory::PaidAgent
            )
    }) {
        warnings.push(
            "Agent-like CLI tools are installed. Keep UI actions behind allowlists, judge verdicts, and action receipts."
                .to_string(),
        );
    }

    warnings.push(
        "This scan is passive: it checks PATH, known app paths, and local localhost ports only. It does not execute AI agents."
            .to_string(),
    );

    warnings
}

fn is_tool_available(tools: &[AiToolProbe], id: &str) -> bool {
    tools
        .iter()
        .any(|tool| tool.id == id && tool.status == AiProbeStatus::Available)
}

fn is_endpoint_available(endpoints: &[AiEndpointProbe], id: &str) -> bool {
    endpoints
        .iter()
        .any(|endpoint| endpoint.id == id && endpoint.status == AiProbeStatus::Available)
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        env::var_os("PATHEXT")
            .map(|value| {
                env::split_paths(&value)
                    .map(|p| p.display().to_string())
                    .collect()
            })
            .unwrap_or_else(|| vec![".exe".to_string(), ".cmd".to_string(), ".bat".to_string()])
    } else {
        vec![String::new()]
    };

    for dir in env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = dir.join(format!("{}{}", name, ext));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn is_local_port_open(socket: &str) -> bool {
    let Ok(addr) = socket.parse::<SocketAddr>() else {
        return false;
    };

    TcpStream::connect_timeout(&addr, Duration::from_millis(180)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_format_contains_core_sections() {
        let report = AiDiscoveryReport {
            generated_at: Utc::now(),
            host_os: "test".to_string(),
            tools: vec![],
            endpoints: vec![],
            recommendations: vec!["Use local-first runtime.".to_string()],
            warnings: vec!["Passive scan only.".to_string()],
            report_path: None,
        };

        let text = format_ai_discovery_report(&report);
        assert!(text.contains("AI Discovery Report"));
        assert!(text.contains("Recommendations"));
        assert!(text.contains("Warnings"));
    }

    #[test]
    fn missing_tool_is_not_available() {
        let tools = vec![AiToolProbe {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            category: AiProbeCategory::LocalRuntime,
            status: AiProbeStatus::Missing,
            detection: "test".to_string(),
            executable_path: None,
            app_path: None,
            local_only: true,
            requires_paid_account: false,
            risk_level: "low".to_string(),
            notes: vec![],
        }];

        assert!(!is_tool_available(&tools, "ollama"));
    }
}
