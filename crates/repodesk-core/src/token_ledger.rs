use std::fs::OpenOptions;
use std::io::Write;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::RepoDeskResult;
use crate::init;
use crate::paths::RepoDeskPaths;
use crate::projects::read_active_project;
use crate::tasks::show_active_task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTokenInput {
    pub agent: String,
    pub model: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub category: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReport {
    pub entries_count: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_tokens: usize,
    pub today_tokens: usize,
    pub by_agent: Vec<AgentTokenTotal>,
    pub by_model: Vec<ModelTokenTotal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTokenTotal {
    pub agent: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTokenTotal {
    pub model: String,
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
            "timestamp,project,task_id,agent,model,category,input_tokens,output_tokens,total_tokens,notes"
        )?;
    }

    let project = read_active_project().unwrap_or_else(|_| "unknown".to_string());
    let task_id = show_active_task()
        .map(|task| task.config.id)
        .unwrap_or_else(|_| "unknown".to_string());

    let total_tokens = input.input_tokens + input.output_tokens;
    let model = input.model.unwrap_or_default();
    let notes = input.notes.unwrap_or_default();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger_file)?;

    writeln!(
        file,
        "{},{},{},{},{},{},{},{},{},{}",
        Utc::now().to_rfc3339(),
        csv_escape(&project),
        csv_escape(&task_id),
        csv_escape(&input.agent),
        csv_escape(&model),
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
            today_tokens: 0,
            by_agent: Vec::new(),
            by_model: Vec::new(),
        });
    }

    let content = std::fs::read_to_string(ledger_file)?;
    Ok(parse_token_report_content(&content))
}

fn parse_token_report_content(content: &str) -> TokenReport {
    let mut entries_count = 0usize;
    let mut total_input_tokens = 0usize;
    let mut total_output_tokens = 0usize;
    let mut today_tokens = 0usize;
    let mut by_agent: Vec<AgentTokenTotal> = Vec::new();
    let mut by_model: Vec<ModelTokenTotal> = Vec::new();

    for line in content.lines().skip(1) {
        let columns = split_simple_csv(line);

        if columns.len() < 9 {
            continue;
        }

        let timestamp_str = &columns[0];
        let today = Utc::now().date_naive();
        let is_today = chrono::DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&Utc).date_naive() == today)
            .unwrap_or(false);

        let agent = columns[3].clone();
        let (model, input_index, output_index) = if columns.len() >= 10 {
            let model = if columns[4].trim().is_empty() {
                "unknown".to_string()
            } else {
                columns[4].clone()
            };
            (model, 6, 7)
        } else {
            ("unknown".to_string(), 5, 6)
        };
        let input_tokens = columns[input_index].parse::<usize>().unwrap_or(0);
        let output_tokens = columns[output_index].parse::<usize>().unwrap_or(0);

        if is_today {
            today_tokens += input_tokens + output_tokens;
        }

        entries_count += 1;
        total_input_tokens += input_tokens;
        total_output_tokens += output_tokens;

        if let Some(existing) = by_agent.iter_mut().find(|item| item.agent == agent) {
            existing.input_tokens += input_tokens;
            existing.output_tokens += output_tokens;
            existing.total_tokens += input_tokens + output_tokens;
        } else {
            by_agent.push(AgentTokenTotal {
                agent: agent.clone(),
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            });
        }

        if let Some(existing) = by_model
            .iter_mut()
            .find(|item| item.agent == agent && item.model == model)
        {
            existing.input_tokens += input_tokens;
            existing.output_tokens += output_tokens;
            existing.total_tokens += input_tokens + output_tokens;
        } else {
            by_model.push(ModelTokenTotal {
                model,
                agent,
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            });
        }
    }

    by_agent.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    by_model.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));

    TokenReport {
        entries_count,
        total_input_tokens,
        total_output_tokens,
        total_tokens: total_input_tokens + total_output_tokens,
        today_tokens,
        by_agent,
        by_model,
    }
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

    if !report.by_model.is_empty() {
        output.push_str("\nBy model:\n");
        for model in &report.by_model {
            output.push_str(&format!(
                "  - {}/{}: total={}, input={}, output={}\n",
                model.agent,
                model.model,
                model.total_tokens,
                model.input_tokens,
                model.output_tokens
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::parse_token_report_content;

    #[test]
    fn aggregates_new_ledger_by_agent_and_model() {
        let content = "\
timestamp,project,task_id,agent,model,category,input_tokens,output_tokens,total_tokens,notes
2026-01-01T00:00:00Z,repo,task,openai,gpt-5,chat,100,20,120,ok
2026-01-01T00:01:00Z,repo,task,openai,gpt-5,chat,10,5,15,ok
2026-01-01T00:02:00Z,repo,task,ollama,llama3.1,local,30,0,30,ok
";

        let report = parse_token_report_content(content);

        assert_eq!(report.entries_count, 3);
        assert_eq!(report.total_input_tokens, 140);
        assert_eq!(report.total_output_tokens, 25);
        assert_eq!(report.by_agent[0].agent, "openai");
        assert_eq!(report.by_agent[0].total_tokens, 135);
        assert_eq!(report.by_model[0].model, "gpt-5");
        assert_eq!(report.by_model[0].total_tokens, 135);
    }

    #[test]
    fn reads_legacy_ledger_without_model_column() {
        let content = "\
timestamp,project,task_id,agent,category,input_tokens,output_tokens,total_tokens,notes
2026-01-01T00:00:00Z,repo,task,codex,patch,50,25,75,legacy
";

        let report = parse_token_report_content(content);

        assert_eq!(report.entries_count, 1);
        assert_eq!(report.total_tokens, 75);
        assert_eq!(report.by_model[0].agent, "codex");
        assert_eq!(report.by_model[0].model, "unknown");
    }
}
