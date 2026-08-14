use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::errors::{RepoDeskError, RepoDeskResult};

use super::{GitFileChange, run_git_captured_bounded, status_label};

const MAX_STATUS_ENTRIES: usize = 50_000;
const MAX_STATUS_FIELD_BYTES: usize = 64 * 1024;
const MAX_STATUS_STDERR_BYTES: usize = 8 * 1024;
const STATUS_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct GitStatusRecord {
    path: String,
    status_code: String,
    original_path: Option<String>,
}

impl GitStatusRecord {
    fn into_change(self) -> GitFileChange {
        let mut chars = self.status_code.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let untracked = x == '?' && y == '?';
        GitFileChange {
            path: self.path,
            status_label: status_label(x, y).to_string(),
            status_code: self.status_code,
            staged: !untracked && x != ' ',
            unstaged: !untracked && y != ' ',
            untracked,
            deleted: x == 'D' || y == 'D',
            renamed: x == 'R' || y == 'R',
        }
    }

    fn diagnostic_line(&self) -> String {
        match &self.original_path {
            Some(original_path) => format!(
                "{} {} <- {}\n",
                self.status_code,
                escape_path(&self.path),
                escape_path(original_path)
            ),
            None => format!("{} {}\n", self.status_code, escape_path(&self.path)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GitStatusSnapshot {
    records: BTreeSet<GitStatusRecord>,
}

impl GitStatusSnapshot {
    pub(crate) fn changes(&self) -> Vec<GitFileChange> {
        self.records
            .iter()
            .cloned()
            .map(GitStatusRecord::into_change)
            .collect()
    }

    pub(crate) fn changed_since(&self, previous: &Self) -> Vec<GitFileChange> {
        self.records
            .difference(&previous.records)
            .cloned()
            .map(GitStatusRecord::into_change)
            .collect()
    }

    /// Compatibility projection for UI/debug consumers that historically read
    /// raw porcelain text. The canonical source remains the typed record set.
    pub(crate) fn diagnostic_porcelain(&self, max_bytes: usize) -> (String, bool) {
        let mut output = String::new();
        let mut truncated = false;

        for record in &self.records {
            let line = record.diagnostic_line();
            if output.len().saturating_add(line.len()) > max_bytes {
                truncated = true;
                break;
            }
            output.push_str(&line);
        }

        (output, truncated)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.records.len()
    }
}

enum NulField {
    Eof,
    Value(Vec<u8>),
    TooLarge,
}

/// Read one NUL-delimited field without allowing a single hostile/pathological
/// pathname to grow the retained buffer beyond `max` bytes. Overflow bytes are
/// consumed and discarded until the delimiter so the pipe keeps draining.
fn read_nul_field(reader: &mut impl BufRead, max: usize) -> io::Result<NulField> {
    let mut retained = Vec::with_capacity(max.min(4 * 1024));
    let mut overflow = false;
    let mut saw_bytes = false;

    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if !saw_bytes {
                return Ok(NulField::Eof);
            }
            return Ok(if overflow {
                NulField::TooLarge
            } else {
                NulField::Value(retained)
            });
        }

        let delimiter = buffer.iter().position(|byte| *byte == 0);
        let take = delimiter.unwrap_or(buffer.len());
        saw_bytes |= take > 0;
        if !overflow {
            let remaining = max.saturating_sub(retained.len());
            let keep = remaining.min(take);
            retained.extend_from_slice(&buffer[..keep]);
            if keep < take {
                overflow = true;
            }
        }

        let consumed = take + usize::from(delimiter.is_some());
        reader.consume(consumed);
        if delimiter.is_some() {
            return Ok(if overflow {
                NulField::TooLarge
            } else {
                NulField::Value(retained)
            });
        }
    }
}

fn parse_status_stream(
    reader: impl Read,
    max_entries: usize,
    max_field_bytes: usize,
) -> RepoDeskResult<GitStatusSnapshot> {
    let mut reader = BufReader::new(reader);
    let mut records = BTreeSet::new();

    loop {
        let field = match read_nul_field(&mut reader, max_field_bytes)? {
            NulField::Eof => break,
            NulField::TooLarge => {
                let _ = io::copy(&mut reader, &mut io::sink());
                return Err(RepoDeskError::GitStatusLimitExceeded {
                    detail: format!("a porcelain status field exceeded {max_field_bytes} bytes"),
                });
            }
            NulField::Value(field) => field,
        };

        if field.len() < 3 || field[2] != b' ' {
            let _ = io::copy(&mut reader, &mut io::sink());
            return Err(RepoDeskError::Api(
                "git status returned malformed porcelain v1 -z output".to_string(),
            ));
        }

        let x = field[0] as char;
        let y = field[1] as char;
        let path = String::from_utf8_lossy(&field[3..]).into_owned();
        if path.is_empty() {
            let _ = io::copy(&mut reader, &mut io::sink());
            return Err(RepoDeskError::Api(
                "git status returned an empty pathname".to_string(),
            ));
        }

        let original_path = if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            match read_nul_field(&mut reader, max_field_bytes)? {
                NulField::Value(value) => Some(String::from_utf8_lossy(&value).into_owned()),
                NulField::TooLarge => {
                    let _ = io::copy(&mut reader, &mut io::sink());
                    return Err(RepoDeskError::GitStatusLimitExceeded {
                        detail: format!(
                            "a porcelain rename source exceeded {max_field_bytes} bytes"
                        ),
                    });
                }
                NulField::Eof => {
                    return Err(RepoDeskError::Api(
                        "git status ended inside a porcelain rename record".to_string(),
                    ));
                }
            }
        } else {
            None
        };

        records.insert(GitStatusRecord {
            path,
            status_code: format!("{x}{y}"),
            original_path,
        });
        if records.len() > max_entries {
            let _ = io::copy(&mut reader, &mut io::sink());
            return Err(RepoDeskError::GitStatusLimitExceeded {
                detail: format!("more than {max_entries} changed paths were reported"),
            });
        }
    }

    Ok(GitStatusSnapshot { records })
}

fn drain_diagnostic(mut reader: impl Read, max: usize) -> io::Result<String> {
    let mut retained = Vec::with_capacity(max.min(8 * 1024));
    let mut buffer = [0u8; 8 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = max.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
    }
    Ok(String::from_utf8_lossy(&retained).into_owned())
}

/// Read the exact pre/post working-tree identity required for executor
/// provenance without materializing raw porcelain output. Returns `None` when
/// `cwd` is not inside a Git work tree.
pub(crate) fn read_git_status(cwd: &Path) -> RepoDeskResult<Option<GitStatusSnapshot>> {
    let inside = run_git_captured_bounded(cwd, &["rev-parse", "--is-inside-work-tree"], 16);
    if !inside.success || inside.truncated || inside.text.trim() != "true" {
        return Ok(None);
    }

    let mut command = Command::new("git");
    command
        .args(["status", "--porcelain=v1", "-z"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RepoDeskError::Api("failed to capture git status stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| RepoDeskError::Api("failed to capture git status stderr".to_string()))?;

    let status_reader = thread::spawn(move || {
        parse_status_stream(stdout, MAX_STATUS_ENTRIES, MAX_STATUS_FIELD_BYTES)
    });
    let stderr_reader = thread::spawn(move || drain_diagnostic(stderr, MAX_STATUS_STDERR_BYTES));

    let status = match child.wait_timeout(STATUS_TIMEOUT) {
        Ok(Some(status)) => status,
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = status_reader.join();
            let _ = stderr_reader.join();
            return Err(RepoDeskError::GitStatusLimitExceeded {
                detail: format!(
                    "git status exceeded the {} second provenance timeout",
                    STATUS_TIMEOUT.as_secs()
                ),
            });
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = status_reader.join();
            let _ = stderr_reader.join();
            return Err(RepoDeskError::Api(format!(
                "failed while waiting for git status: {error}"
            )));
        }
    };

    let snapshot = status_reader
        .join()
        .map_err(|_| RepoDeskError::Api("git status parser thread panicked".to_string()))??;
    let stderr = stderr_reader.join().map_err(|_| {
        RepoDeskError::Api("git status stderr reader thread panicked".to_string())
    })??;
    if !status.success() {
        return Err(RepoDeskError::Api(format!(
            "git status failed: {}",
            stderr.trim()
        )));
    }

    Ok(Some(snapshot))
}

fn escape_path(path: &str) -> String {
    path.chars().flat_map(char::escape_default).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_nul_status_without_filename_quoting_or_newline_ambiguity() {
        let raw = b" M normal.txt\0?? weird\nname.txt\0R  new name.txt\0old name.txt\0";
        let snapshot = parse_status_stream(Cursor::new(raw), 10, 1024).unwrap();
        assert_eq!(snapshot.len(), 3);
        let changes = snapshot.changed_since(&GitStatusSnapshot::default());
        assert!(changes.iter().any(|change| change.path == "normal.txt"));
        assert!(
            changes
                .iter()
                .any(|change| change.path == "weird\nname.txt")
        );
        assert!(
            changes
                .iter()
                .any(|change| change.path == "new name.txt" && change.renamed)
        );
    }

    #[test]
    fn changed_since_detects_status_transition_for_existing_path() {
        let before = parse_status_stream(Cursor::new(b" M seed.txt\0"), 10, 1024).unwrap();
        let after = parse_status_stream(Cursor::new(b"M  seed.txt\0"), 10, 1024).unwrap();
        let changes = after.changed_since(&before);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "seed.txt");
        assert!(changes[0].staged);
    }

    #[test]
    fn diagnostic_projection_is_bounded_and_escaped() {
        let snapshot = parse_status_stream(Cursor::new(b"?? weird\nname.txt\0 M other.txt\0"), 10, 1024)
            .unwrap();
        let (diagnostic, truncated) = snapshot.diagnostic_porcelain(20);
        assert!(diagnostic.len() <= 20);
        assert!(truncated);
        assert!(!diagnostic.contains('\n') || diagnostic.ends_with('\n'));
    }

    #[test]
    fn status_entry_limit_fails_closed() {
        let error = parse_status_stream(Cursor::new(b" M a\0 M b\0"), 1, 1024)
            .expect_err("second record must exceed the configured limit");
        assert!(matches!(
            error,
            RepoDeskError::GitStatusLimitExceeded { .. }
        ));
    }

    #[test]
    fn status_field_limit_fails_closed_without_retaining_the_field() {
        let error = parse_status_stream(Cursor::new(b" M abcdef\0"), 10, 4)
            .expect_err("oversized field must fail closed");
        assert!(matches!(
            error,
            RepoDeskError::GitStatusLimitExceeded { .. }
        ));
    }
}
