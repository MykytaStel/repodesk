pub fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return "  - none".to_string();
    }
    items
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");

    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

pub fn split_simple_csv(line: &str) -> Vec<String> {
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

pub trait ConfigStore: serde::Serialize + serde::de::DeserializeOwned + Default {
    const FILE_NAME: &'static str;

    fn ensure_config() -> crate::errors::RepoDeskResult<Self> {
        crate::init::init_home()?;
        let paths = crate::paths::RepoDeskPaths::resolve()?;
        let file = paths.config_dir.join(Self::FILE_NAME);

        if !file.exists() {
            let config = Self::default();
            let content = toml::to_string_pretty(&config)?;
            std::fs::write(&file, content)?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(file)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    fn load_config() -> crate::errors::RepoDeskResult<Self> {
        Self::ensure_config()
    }
}
