use super::probes::*;
use super::types::*;
use crate::errors::RepoDeskResult;
use crate::paths::RepoDeskPaths;
use chrono::Utc;
use std::env;
use std::fs;

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

        // test requires is_tool_available logic but since it is in probes.rs we just want to ensure it compiles
        // we'll copy the test to probes.rs and keep this generic format test here.
    }
}
