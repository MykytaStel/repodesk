//! Human review of a coding-agent run's changeset: **accept** (stage the files
//! the agent changed, ready for a commit the human makes) or **reject** (discard
//! in-place changes, or leave isolated-worktree changes out of the active tree).
//!
//! This is the actionable half of agent-run diff capture
//! ([`crate::executors`]): the run records *which* files it changed, and this
//! module lets the operator keep or drop exactly those files. It is bounded by
//! construction — it only ever touches paths the recorded run reported, never the
//! whole tree — and it never commits, pushes, or merges. RepoDesk stays the
//! control plane; the human stays the operator.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::errors::{RepoDeskError, RepoDeskResult};
use crate::git_workspace::{GitFileChange, parse_porcelain_status};
use crate::worktree::RunWorktree;

use super::runner::load_run;
use super::types::{OrchestrationRun, SubAgentResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    /// Stage the agent-changed files (`git add`) so the human can commit them.
    Accept,
    /// Discard the agent's changes: restore tracked files to HEAD, remove
    /// untracked ones.
    Reject,
}

impl ReviewAction {
    pub fn from_label(value: &str) -> RepoDeskResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "accept" => Ok(Self::Accept),
            "reject" => Ok(Self::Reject),
            other => Err(RepoDeskError::RoutingFailed {
                detail: format!("unknown review action '{other}' (expected accept|reject)"),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewedFile {
    pub path: String,
    /// What happened: `staged`, `applied and staged`, `restored`, `deleted`,
    /// `left in isolated worktree`, or `skipped: <reason>`.
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReview {
    pub run_id: String,
    pub action: ReviewAction,
    pub project: String,
    pub processed: Vec<ReviewedFile>,
    pub warnings: Vec<String>,
}

/// Apply `action` to every file the persisted run `run_id` reported changing,
/// against the active project's working tree.
pub fn review_run(run_id: &str, action: ReviewAction) -> RepoDeskResult<RunReview> {
    let run = load_run(run_id)?.ok_or_else(|| RepoDeskError::RoutingFailed {
        detail: format!("no orchestration run '{run_id}' for the active task"),
    })?;
    let project = crate::projects::get_active_project()?;
    let paths = collect_changed_files(&run);
    let has_isolated_changes = run
        .results
        .iter()
        .any(|result| result.workspace.is_some() && !result.changed_files.is_empty());

    let mut review = RunReview {
        run_id: run_id.to_string(),
        action,
        project: project.name.clone(),
        processed: Vec::new(),
        warnings: Vec::new(),
    };

    if paths.is_empty() {
        review
            .warnings
            .push("run recorded no changed files to review".to_string());
        return Ok(review);
    }
    if !is_git_repo(project.path.as_path()) {
        review
            .warnings
            .push("active project is not a git repository; nothing to review".to_string());
        return Ok(review);
    }

    if has_isolated_changes {
        review_run_results(project.path.as_path(), &run, action, &mut review)?;
    } else {
        for path in paths {
            review
                .processed
                .push(review_one(project.path.as_path(), &path, action));
        }
    }
    Ok(review)
}

/// Persist a [`ReviewReceipt`] for `run_id` from the result of [`review_run`].
///
/// **Accept is atomic**: if any intended file was `skipped` (not applied/staged),
/// no Accepted receipt is written and the Review phase stays open — a partial
/// apply must never look reviewed. The receipt binds to the run and the run's
/// changeset digest, so a later run (new digest) can't inherit this review, and
/// any review supersedes a prior verification/finish.
pub fn record_review(run_id: &str, action: ReviewAction, review: &RunReview) -> RepoDeskResult<()> {
    use crate::workflow::receipt::{
        ReviewDecision, ReviewReceipt, index_tree_sha, load_receipt_for_run, save_receipt,
    };

    let mut receipt =
        load_receipt_for_run(run_id)?.ok_or_else(|| RepoDeskError::RoutingFailed {
            detail: format!("no run receipt for '{run_id}' — cannot record review"),
        })?;

    let reviewed_paths: Vec<String> = review.processed.iter().map(|f| f.path.clone()).collect();
    // The reviewed changeset is, by construction, the run's recorded changeset;
    // bind to the execution digest so the phase signal matches exactly.
    let changeset_digest = receipt
        .execution
        .changeset_digest
        .clone()
        .unwrap_or_else(|| crate::workflow::receipt::changeset_digest(&reviewed_paths));

    let decision = match action {
        ReviewAction::Accept => {
            let skipped: Vec<String> = review
                .processed
                .iter()
                .filter(|f| f.outcome.starts_with("skipped"))
                .map(|f| f.path.clone())
                .collect();
            if !skipped.is_empty() {
                return Err(RepoDeskError::RoutingFailed {
                    detail: format!(
                        "accept blocked: {} of the run's files were not applied ({}). \
                         Resolve them before accepting so Review reflects the whole changeset.",
                        skipped.len(),
                        skipped.join(", ")
                    ),
                });
            }
            ReviewDecision::Accepted
        }
        ReviewAction::Reject => ReviewDecision::Rejected,
    };

    let index_tree_after_accept = if decision == ReviewDecision::Accepted {
        crate::projects::get_active_project()
            .ok()
            .and_then(|project| index_tree_sha(&project.path))
    } else {
        None
    };

    receipt.review = Some(ReviewReceipt {
        run_id: run_id.to_string(),
        decision,
        reviewed_paths,
        changeset_digest,
        index_tree_after_accept,
    });
    // A new review decision invalidates any prior verification/commit evidence.
    receipt.verification = None;
    receipt.finish = None;
    save_receipt(&receipt)?;
    Ok(())
}

/// Distinct repo-relative paths the run's steps reported changing, in first-seen
/// order.
fn collect_changed_files(run: &OrchestrationRun) -> Vec<String> {
    let mut seen = Vec::new();
    for result in &run.results {
        for path in &result.changed_files {
            if !seen.iter().any(|existing| existing == path) {
                seen.push(path.clone());
            }
        }
    }
    seen
}

fn review_run_results(
    project_path: &Path,
    run: &OrchestrationRun,
    action: ReviewAction,
    review: &mut RunReview,
) -> RepoDeskResult<()> {
    for result in &run.results {
        if result.changed_files.is_empty() {
            continue;
        }
        if let Some(worktree) = &result.workspace {
            let mut processed = review_isolated_result(project_path, result, worktree, action)?;
            review.processed.append(&mut processed);
            if action == ReviewAction::Reject {
                review.warnings.push(format!(
                    "isolated worktree kept for manual inspection/cleanup: {}",
                    worktree.path
                ));
            }
        } else {
            for path in &result.changed_files {
                review
                    .processed
                    .push(review_one(project_path, path, action));
            }
        }
    }
    Ok(())
}

fn review_isolated_result(
    project_path: &Path,
    result: &SubAgentResult,
    worktree: &RunWorktree,
    action: ReviewAction,
) -> RepoDeskResult<Vec<ReviewedFile>> {
    let worktree_path = Path::new(&worktree.path);
    if !worktree_path.exists() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!("isolated worktree is missing: {}", worktree.path),
        });
    }
    if !is_git_repo(worktree_path) {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "isolated worktree is not a git repository: {}",
                worktree.path
            ),
        });
    }

    let paths = safe_unique_paths(&result.changed_files);
    if paths.is_empty() {
        return Ok(result
            .changed_files
            .iter()
            .map(|path| ReviewedFile {
                path: path.clone(),
                outcome: "skipped: path is absolute or escapes the project".to_string(),
            })
            .collect());
    }

    if action == ReviewAction::Reject {
        return Ok(paths
            .into_iter()
            .map(|path| ReviewedFile {
                path,
                outcome: "left in isolated worktree; active checkout untouched".to_string(),
            })
            .collect());
    }

    accept_isolated_changes(project_path, worktree_path, &paths)
}

fn accept_isolated_changes(
    project_path: &Path,
    worktree_path: &Path,
    paths: &[String],
) -> RepoDeskResult<Vec<ReviewedFile>> {
    let statuses = worktree_status_by_path(worktree_path, paths)?;
    let mut tracked_paths = Vec::new();
    let mut untracked_paths = Vec::new();
    // (old, new) pairs for staged renames, recorded by the diff delta as a single
    // "old -> new" path string the working-tree status pathspec can't re-match.
    let mut rename_pairs: Vec<(String, String)> = Vec::new();
    let mut processed = Vec::new();

    for path in paths {
        // A rename arrives as a combined "old -> new" path (porcelain rename
        // notation), so detect it before the status lookup — the pathspec can't
        // match that literal string.
        if let Some((old, new)) = split_rename(path) {
            if is_safe_relative(&old) && is_safe_relative(&new) {
                rename_pairs.push((old, new));
            } else {
                processed.push(ReviewedFile {
                    path: path.clone(),
                    outcome: "skipped: rename path is absolute or escapes the project".to_string(),
                });
            }
            continue;
        }
        let Some(change) = statuses.get(path) else {
            processed.push(ReviewedFile {
                path: path.clone(),
                outcome: "skipped: no longer changed in isolated worktree".to_string(),
            });
            continue;
        };
        if change.untracked {
            untracked_paths.push(path.clone());
        } else {
            tracked_paths.push(path.clone());
        }
    }

    // Pre-flight all destinations together so a partial apply can't happen: the
    // active checkout must be clean for every touched path (incl. both ends of a
    // rename), and new destinations (untracked + rename targets) must be free.
    let rename_old: Vec<String> = rename_pairs.iter().map(|(old, _)| old.clone()).collect();
    let rename_new: Vec<String> = rename_pairs.iter().map(|(_, new)| new.clone()).collect();
    let clean_paths = tracked_paths
        .iter()
        .chain(untracked_paths.iter())
        .chain(rename_old.iter())
        .chain(rename_new.iter())
        .cloned()
        .collect::<Vec<_>>();
    ensure_active_paths_are_clean(project_path, &clean_paths)?;
    ensure_untracked_destinations_are_free(project_path, &untracked_paths)?;
    ensure_untracked_destinations_are_free(project_path, &rename_new)?;

    if !tracked_paths.is_empty() {
        let patch = git_diff_head(worktree_path, &tracked_paths)?;
        if !patch.trim().is_empty() {
            git_apply_stdin(project_path, &["apply", "--index", "--check", "-"], &patch)?;
            git_apply_stdin(project_path, &["apply", "--index", "-"], &patch)?;
        }
        processed.extend(tracked_paths.into_iter().map(|path| ReviewedFile {
            path,
            outcome: "applied and staged".to_string(),
        }));
    }

    // Apply each rename as a rename-aware diff (`-M`): `git apply --index` then
    // performs the move (and any content edit) in the active index + worktree.
    for (old, new) in rename_pairs {
        let patch = git_diff_head_renames(worktree_path, &[old.clone(), new.clone()])?;
        let display = format!("{old} -> {new}");
        if patch.trim().is_empty() {
            processed.push(ReviewedFile {
                path: display,
                outcome: "skipped: rename no longer present in isolated worktree".to_string(),
            });
            continue;
        }
        git_apply_stdin(project_path, &["apply", "--index", "--check", "-"], &patch)?;
        git_apply_stdin(project_path, &["apply", "--index", "-"], &patch)?;
        processed.push(ReviewedFile {
            path: display,
            outcome: "renamed and staged".to_string(),
        });
    }

    for path in untracked_paths {
        copy_untracked_file(worktree_path, project_path, &path)?;
        if git_ok(project_path, &["add", "--", &path]) {
            processed.push(ReviewedFile {
                path,
                outcome: "copied and staged".to_string(),
            });
        } else {
            processed.push(ReviewedFile {
                path,
                outcome: "skipped: copied file but git add failed".to_string(),
            });
        }
    }

    Ok(processed)
}

/// Split a porcelain rename path (`old -> new`, each side optionally quoted) into
/// its two repo-relative paths. Returns `None` when there is no rename arrow.
fn split_rename(path: &str) -> Option<(String, String)> {
    let (old, new) = path.split_once(" -> ")?;
    Some((unquote_path(old), unquote_path(new)))
}

/// Strip the surrounding quotes git adds to paths with special characters.
fn unquote_path(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed)
        .to_string()
}

fn safe_unique_paths(paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        let trimmed = path.trim();
        if is_safe_relative(trimmed) && !seen.contains(trimmed) {
            seen.insert(trimmed.to_string());
            out.push(trimmed.to_string());
        }
    }
    out
}

fn worktree_status_by_path(
    worktree_path: &Path,
    paths: &[String],
) -> RepoDeskResult<HashMap<String, GitFileChange>> {
    let mut args = vec![
        "status".to_string(),
        "--porcelain=v1".to_string(),
        "--".to_string(),
    ];
    args.extend(paths.iter().cloned());
    let raw = git_capture_owned(worktree_path, &args)?;
    Ok(parse_porcelain_status(&raw)
        .into_iter()
        .map(|change| (change.path.clone(), change))
        .collect())
}

fn ensure_active_paths_are_clean(project_path: &Path, paths: &[String]) -> RepoDeskResult<()> {
    let dirty = paths
        .iter()
        .filter(|path| active_path_has_changes(project_path, path))
        .cloned()
        .collect::<Vec<_>>();
    if dirty.is_empty() {
        Ok(())
    } else {
        Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "cannot apply isolated changes because active checkout has local changes in: {}",
                dirty.join(", ")
            ),
        })
    }
}

fn ensure_untracked_destinations_are_free(
    project_path: &Path,
    paths: &[String],
) -> RepoDeskResult<()> {
    let existing = paths
        .iter()
        .filter(|path| project_path.join(path).exists())
        .cloned()
        .collect::<Vec<_>>();
    if existing.is_empty() {
        Ok(())
    } else {
        Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "cannot copy isolated untracked files because paths already exist in active checkout: {}",
                existing.join(", ")
            ),
        })
    }
}

fn active_path_has_changes(project_path: &Path, path: &str) -> bool {
    git_capture_owned(
        project_path,
        &[
            "status".to_string(),
            "--porcelain=v1".to_string(),
            "--".to_string(),
            path.to_string(),
        ],
    )
    .map(|output| !output.trim().is_empty())
    .unwrap_or(true)
}

fn git_diff_head(worktree_path: &Path, paths: &[String]) -> RepoDeskResult<String> {
    let mut args = vec![
        "diff".to_string(),
        "--binary".to_string(),
        "--no-color".to_string(),
        "HEAD".to_string(),
        "--".to_string(),
    ];
    args.extend(paths.iter().cloned());
    git_capture_owned(worktree_path, &args)
}

/// Like [`git_diff_head`] but with rename detection (`-M`) on, so a moved file
/// produces a `rename from/to` patch `git apply --index` replays as a real move.
fn git_diff_head_renames(worktree_path: &Path, paths: &[String]) -> RepoDeskResult<String> {
    let mut args = vec![
        "diff".to_string(),
        "--binary".to_string(),
        "--no-color".to_string(),
        "--find-renames".to_string(),
        "HEAD".to_string(),
        "--".to_string(),
    ];
    args.extend(paths.iter().cloned());
    git_capture_owned(worktree_path, &args)
}

fn copy_untracked_file(
    worktree_path: &Path,
    project_path: &Path,
    path: &str,
) -> RepoDeskResult<()> {
    let source = worktree_path.join(path);
    let destination = project_path.join(path);
    if !source.is_file() {
        return Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "cannot copy isolated untracked path '{}' because it is not a file",
                path
            ),
        });
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &destination)?;
    Ok(())
}

fn review_one(project_path: &Path, path: &str, action: ReviewAction) -> ReviewedFile {
    if !is_safe_relative(path) {
        return ReviewedFile {
            path: path.to_string(),
            outcome: "skipped: path is absolute or escapes the project".to_string(),
        };
    }

    match action {
        ReviewAction::Accept => {
            // Stage the change; the human commits. Works for tracked edits and
            // newly added files alike.
            if git_ok(project_path, &["add", "--", path]) {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "staged".to_string(),
                }
            } else {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "skipped: git add failed".to_string(),
                }
            }
        }
        ReviewAction::Reject => {
            if is_tracked(project_path, path) {
                // Reset both the index and working tree for this path to HEAD.
                if git_ok(
                    project_path,
                    &[
                        "restore",
                        "--source=HEAD",
                        "--staged",
                        "--worktree",
                        "--",
                        path,
                    ],
                ) {
                    ReviewedFile {
                        path: path.to_string(),
                        outcome: "restored".to_string(),
                    }
                } else {
                    ReviewedFile {
                        path: path.to_string(),
                        outcome: "skipped: git restore failed".to_string(),
                    }
                }
            } else if git_ok(project_path, &["clean", "-f", "--", path]) {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "deleted".to_string(),
                }
            } else {
                ReviewedFile {
                    path: path.to_string(),
                    outcome: "skipped: untracked cleanup failed".to_string(),
                }
            }
        }
    }
}

/// A path is safe to act on only if it is non-empty, relative, and contains no
/// `..` component — so review can never reach outside the project root.
fn is_safe_relative(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() || Path::new(trimmed).is_absolute() {
        return false;
    }
    !Path::new(trimmed)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

pub(crate) fn is_git_repo(project_path: &Path) -> bool {
    git_ok(project_path, &["rev-parse", "--is-inside-work-tree"])
}

fn is_tracked(project_path: &Path, path: &str) -> bool {
    git_ok(project_path, &["ls-files", "--error-unmatch", "--", path])
}

fn git_capture_owned(project_path: &Path, args: &[String]) -> RepoDeskResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .map_err(|error| RepoDeskError::RoutingFailed {
            detail: format!("failed to run git {}: {error}", args.join(" ")),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

pub(crate) fn git_apply_stdin(
    project_path: &Path,
    args: &[&str],
    stdin: &str,
) -> RepoDeskResult<()> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| RepoDeskError::RoutingFailed {
            detail: format!("failed to run git {}: {error}", args.join(" ")),
        })?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| RepoDeskError::RoutingFailed {
            detail: format!("failed to open stdin for git {}", args.join(" ")),
        })?
        .write_all(stdin.as_bytes())?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(RepoDeskError::RoutingFailed {
            detail: format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

/// Run a git subcommand (argv-only, no shell) in `project_path`, returning
/// whether it exited successfully. Output is discarded; callers only need the
/// status. Paths are passed after `--` so they cannot be read as flags.
fn git_ok(project_path: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::types::SubAgentStatus;
    use super::*;
    use crate::worktree::RunWorktree;
    use tempfile::TempDir;

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    fn init_repo() -> TempDir {
        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(git_ok(repo.path(), args), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
        std::fs::write(repo.path().join("keep.txt"), "keep\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "init"]);
        repo
    }

    fn create_worktree(repo: &Path, parent: &Path, id: &str) -> RunWorktree {
        let path = parent.join(id);
        let path_str = path.display().to_string();
        assert!(
            git_ok(repo, &["worktree", "add", "--detach", &path_str, "HEAD"]),
            "git worktree add failed"
        );
        RunWorktree {
            workspace_id: id.to_string(),
            run_id: "run-test".to_string(),
            step_id: "implement".to_string(),
            path: path_str,
            base_commit: git_capture_owned(repo, &["rev-parse".to_string(), "HEAD".to_string()])
                .unwrap()
                .trim()
                .to_string(),
            created_at: "2026-06-20T00:00:00Z".to_string(),
            metadata_path: None,
        }
    }

    fn isolated_result(paths: &[&str], worktree: &RunWorktree) -> SubAgentResult {
        SubAgentResult {
            task_id: "implement".to_string(),
            agent: "codex_cli".to_string(),
            provider: "codex_cli".to_string(),
            model: String::new(),
            status: SubAgentStatus::Ok,
            output: String::new(),
            input_tokens: 1,
            output_tokens: 1,
            cost_units: 0.0,
            captured_proposals: 0,
            changed_files: paths.iter().map(|path| path.to_string()).collect(),
            diff_path: None,
            workspace: Some(worktree.clone()),
            notes: Vec::new(),
        }
    }

    #[test]
    fn rejects_unsafe_paths() {
        assert!(!is_safe_relative(""));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("../outside.txt"));
        assert!(!is_safe_relative("src/../../escape.rs"));
        assert!(is_safe_relative("src/main.rs"));
        assert!(is_safe_relative("added.txt"));
    }

    #[test]
    fn action_parsing() {
        assert_eq!(
            ReviewAction::from_label("accept").unwrap(),
            ReviewAction::Accept
        );
        assert_eq!(
            ReviewAction::from_label("REJECT").unwrap(),
            ReviewAction::Reject
        );
        assert!(ReviewAction::from_label("merge").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn isolated_accept_applies_tracked_and_untracked_changes() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let worktree = create_worktree(repo.path(), parent.path(), "agent-apply");
        let worktree_path = Path::new(&worktree.path);

        std::fs::write(worktree_path.join("seed.txt"), "seed\nagent\n").unwrap();
        std::fs::write(worktree_path.join("new.txt"), "new\n").unwrap();
        std::fs::write(repo.path().join("keep.txt"), "keep\ndeveloper\n").unwrap();

        let result = isolated_result(&["seed.txt", "new.txt"], &worktree);
        let processed =
            review_isolated_result(repo.path(), &result, &worktree, ReviewAction::Accept).unwrap();

        assert!(
            processed
                .iter()
                .any(|file| file.path == "seed.txt" && file.outcome == "applied and staged")
        );
        assert!(
            processed
                .iter()
                .any(|file| file.path == "new.txt" && file.outcome == "copied and staged")
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("seed.txt")).unwrap(),
            "seed\nagent\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("keep.txt")).unwrap(),
            "keep\ndeveloper\n"
        );
        let staged = git_capture_owned(
            repo.path(),
            &[
                "diff".to_string(),
                "--cached".to_string(),
                "--name-only".to_string(),
            ],
        )
        .unwrap();
        assert!(staged.lines().any(|line| line == "seed.txt"));
        assert!(staged.lines().any(|line| line == "new.txt"));
        assert!(!staged.lines().any(|line| line == "keep.txt"));

        let _ = git_ok(
            repo.path(),
            &["worktree", "remove", "--force", &worktree.path],
        );
    }

    #[test]
    fn split_rename_parses_porcelain_arrow() {
        assert_eq!(
            split_rename("old.rs -> new.rs"),
            Some(("old.rs".to_string(), "new.rs".to_string()))
        );
        assert_eq!(
            split_rename("\"old name.rs\" -> \"new name.rs\""),
            Some(("old name.rs".to_string(), "new name.rs".to_string()))
        );
        assert_eq!(split_rename("plain.rs"), None);
    }

    #[cfg(unix)]
    #[test]
    fn isolated_accept_applies_a_rename() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let worktree = create_worktree(repo.path(), parent.path(), "agent-rename");
        let worktree_path = Path::new(&worktree.path);

        // Stage a rename in the isolated worktree (git mv stages it; status → R).
        assert!(git_ok(worktree_path, &["mv", "seed.txt", "renamed.txt"]));

        // The diff delta records a rename as a single "old -> new" path.
        let result = isolated_result(&["seed.txt -> renamed.txt"], &worktree);
        let processed =
            review_isolated_result(repo.path(), &result, &worktree, ReviewAction::Accept).unwrap();

        assert!(
            processed
                .iter()
                .any(|f| f.path == "seed.txt -> renamed.txt" && f.outcome == "renamed and staged"),
            "expected rename to be applied, got {processed:?}"
        );
        // The active checkout reflects the move.
        assert!(!repo.path().join("seed.txt").exists());
        assert_eq!(
            std::fs::read_to_string(repo.path().join("renamed.txt")).unwrap(),
            "seed\n"
        );
        // Both ends of the rename are staged.
        let staged = git_capture_owned(
            repo.path(),
            &[
                "diff".to_string(),
                "--cached".to_string(),
                "--name-status".to_string(),
            ],
        )
        .unwrap();
        assert!(staged.contains("seed.txt"));
        assert!(staged.contains("renamed.txt"));

        let _ = git_ok(
            repo.path(),
            &["worktree", "remove", "--force", &worktree.path],
        );
    }

    #[cfg(unix)]
    #[test]
    fn isolated_accept_refuses_when_active_path_is_dirty() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let worktree = create_worktree(repo.path(), parent.path(), "agent-conflict");
        let worktree_path = Path::new(&worktree.path);

        std::fs::write(worktree_path.join("seed.txt"), "seed\nagent\n").unwrap();
        std::fs::write(repo.path().join("seed.txt"), "seed\ndeveloper\n").unwrap();

        let result = isolated_result(&["seed.txt"], &worktree);
        let error = review_isolated_result(repo.path(), &result, &worktree, ReviewAction::Accept)
            .expect_err("dirty active path must block apply-back");

        assert!(
            error
                .to_string()
                .contains("active checkout has local changes")
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("seed.txt")).unwrap(),
            "seed\ndeveloper\n"
        );
        let staged = git_capture_owned(
            repo.path(),
            &[
                "diff".to_string(),
                "--cached".to_string(),
                "--name-only".to_string(),
            ],
        )
        .unwrap();
        assert!(staged.trim().is_empty());

        let _ = git_ok(
            repo.path(),
            &["worktree", "remove", "--force", &worktree.path],
        );
    }

    #[cfg(unix)]
    #[test]
    fn isolated_reject_leaves_active_checkout_untouched() {
        if !git_available() {
            return;
        }
        let repo = init_repo();
        let parent = tempfile::TempDir::new().unwrap();
        let worktree = create_worktree(repo.path(), parent.path(), "agent-reject");
        let worktree_path = Path::new(&worktree.path);

        std::fs::write(worktree_path.join("seed.txt"), "seed\nagent\n").unwrap();

        let result = isolated_result(&["seed.txt"], &worktree);
        let processed =
            review_isolated_result(repo.path(), &result, &worktree, ReviewAction::Reject).unwrap();

        assert_eq!(
            processed[0].outcome,
            "left in isolated worktree; active checkout untouched"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("seed.txt")).unwrap(),
            "seed\n"
        );

        let _ = git_ok(
            repo.path(),
            &["worktree", "remove", "--force", &worktree.path],
        );
    }

    #[cfg(unix)]
    #[test]
    fn reject_restores_tracked_and_deletes_untracked() {
        if Command::new("git").arg("--version").output().is_err() {
            return; // git not installed
        }
        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(git_ok(repo.path(), args), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "init"]);

        // Simulate an agent: modify the tracked file, add a new untracked one.
        std::fs::write(repo.path().join("seed.txt"), "seed\nchanged\n").unwrap();
        std::fs::write(repo.path().join("added.txt"), "new\n").unwrap();

        let restored = review_one(repo.path(), "seed.txt", ReviewAction::Reject);
        assert_eq!(restored.outcome, "restored");
        assert_eq!(
            std::fs::read_to_string(repo.path().join("seed.txt")).unwrap(),
            "seed\n"
        );

        let deleted = review_one(repo.path(), "added.txt", ReviewAction::Reject);
        assert_eq!(deleted.outcome, "deleted");
        assert!(!repo.path().join("added.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn accept_stages_changed_file() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let repo = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            assert!(git_ok(repo.path(), args), "git {args:?} failed");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("added.txt"), "new\n").unwrap();

        let staged = review_one(repo.path(), "added.txt", ReviewAction::Accept);
        assert_eq!(staged.outcome, "staged");
        // The file is now in the index.
        assert!(is_tracked(repo.path(), "added.txt"));
    }
}
