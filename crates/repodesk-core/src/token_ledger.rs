use std::fs::OpenOptions;
use std::io::Write;

use chrono::Utc;

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::read_active_project;
use crate::tasks::show_active_task;

#[derive(Debug, Clone)]
pub struct LogTokenInput {
    pub agent: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub category: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TokenReport {
    pub entries_count: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_tokens: usize,
    pub by_agent: Vec<AgentTokenTotal>,
}

#[derive(Debug, Clone)]
pub struct AgentTokenTotal {
    pub agent: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

pub fn log_token_event(input: LogTokenInput) -> RepoDeskResult<String> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let ledger_file = paths.logs_dir.join("token-ledger.csv");

    if !ledger_file.exists() {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ledger_file)?;

        writeln!(
            file,
            "timestamp,project,task_id,agent,category,input_tokens,output_tokens,total_tokens,notes"
        )?;
    }

    let project = read_active_project().unwrap_or_else(|_| "unknown".to_string());
    let task_id = show_active_task()
        .map(|task| task.config.id)
        .unwrap_or_else(|_| "unknown".to_string());

    let total_tokens = input.input_tokens + input.output_tokens;
    let notes = input.notes.unwrap_or_default();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger_file)?;

    writeln!(
        file,
        "{},{},{},{},{},{},{},{},{}",
        Utc::now().to_rfc3339(),
        csv_escape(&project),
        csv_escape(&task_id),
        csv_escape(&input.agent),
        csv_escape(&input.category),
        input.input_tokens,
        input.output_tokens,
        total_tokens,
        csv_escape(&notes)
    )?;

    Ok(ledger_file.display().to_string())
}

pub fn read_token_report() -> RepoDeskResult<TokenReport> {
    init::init_home()?;

    let paths = RepoDeskPaths::resolve()?;
    let ledger_file = paths.logs_dir.join("token-ledger.csv");

    if !ledger_file.exists() {
        return Ok(TokenReport {
            entries_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            by_agent: Vec::new(),
        });
    }

    let content = std::fs::read_to_string(ledger_file)?;

    let mut entries_count = 0usize;
    let mut total_input_tokens = 0usize;
    let mut total_output_tokens = 0usize;
    let mut by_agent: Vec<AgentTokenTotal> = Vec::new();

    for line in content.lines().skip(1) {
        let columns = split_simple_csv(line);

        if columns.len() < 9 {
            continue;
        }

        let agent = columns[3].clone();
        let input_tokens = columns[5].parse::<usize>().unwrap_or(0);
        let output_tokens = columns[6].parse::<usize>().unwrap_or(0);

        entries_count += 1;
        total_input_tokens += input_tokens;
        total_output_tokens += output_tokens;

        if let Some(existing) = by_agent.iter_mut().find(|item| item.agent == agent) {
            existing.input_tokens += input_tokens;
            existing.output_tokens += output_tokens;
            existing.total_tokens += input_tokens + output_tokens;
        } else {
            by_agent.push(AgentTokenTotal {
                agent,
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            });
        }
    }

    by_agent.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));

    Ok(TokenReport {
        entries_count,
        total_input_tokens,
        total_output_tokens,
        total_tokens: total_input_tokens + total_output_tokens,
        by_agent,
    })
}

pub fn format_token_report(report: &TokenReport) -> String {
    if report.entries_count == 0 {
        return "No token ledger entries yet.\n".to_string();
    }

    let mut output = String::new();

    output.push_str("Token report:\n\n");
    output.push_str(&format!("Entries: {}\n", report.entries_count));
    output.push_str(&format!("Input tokens: {}\n", report.total_input_tokens));
    output.push_str(&format!("Output tokens: {}\n", report.total_output_tokens));
    output.push_str(&format!("Total tokens: {}\n\n", report.total_tokens));

    output.push_str("By agent:\n");

    for agent in &report.by_agent {
        output.push_str(&format!(
            "  - {}: total={}, input={}, output={}\n",
            agent.agent, agent.total_tokens, agent.input_tokens, agent.output_tokens
        ));
    }

    output
}

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");

    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

fn split_simple_csv(line: &str) -> Vec<String> {
    let mut columns = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                columns.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    columns.push(current);

    columns
}
