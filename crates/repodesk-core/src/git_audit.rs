use std::path::Path;
use std::process::Command;

use crate::errors::RepoDeskResult;
use crate::projects::get_active_project;

pub fn git_audit() -> RepoDeskResult<String> {
    let project = get_active_project()?;

    let branch = run_git(&project.path, &["branch", "--show-current"]);
    let status = run_git(&project.path, &["status", "--short"]);
    let remote = run_git(&project.path, &["remote", "-v"]);
    let last_commits = run_git(&project.path, &["log", "--oneline", "-5"]);

    Ok(format!(
        r#"Git audit:

Project: {}
Path: {}

Branch:
```txt
{}
```

Status:
```txt
{}
```

Remotes:
```txt
{}
```

Last commits:
```txt
{}
```
"#,
        project.name,
        project.path.display(),
        fallback(branch.trim(), "unknown"),
        fallback(status.trim(), "clean"),
        fallback(remote.trim(), "no remotes"),
        fallback(last_commits.trim(), "no commits")
    ))
}

pub fn backup_plan() -> RepoDeskResult<String> {
    let project = get_active_project()?;

    Ok(format!(
        r#"Backup plan for `{}`:

1. Check local state:

```bash
git status
git branch
git remote -v
```

2. Run local checks:

```bash
cargo fmt
cargo check
```

3. Commit safely:

```bash
git add .
git commit -m "Build RepoDesk control brain"
```

4. Push to private repository:

```bash
git push -u origin main
```

Safety notes:
- Do not commit `.env`, secrets, tokens, credentials, keys, or local cache files.
- Run `repodesk safety scan-context` before sending context to paid agents.
- Keep RepoDesk private until the security model is mature.
"#,
        project.name
    ))
}

fn run_git(project_path: &Path, args: &[&str]) -> String {
    match Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Ok(output) => String::from_utf8_lossy(&output.stderr).to_string(),
        Err(error) => format!("failed to run git {}: {}", args.join(" "), error),
    }
}

fn fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() {
        fallback
    } else {
        value
    }
}
