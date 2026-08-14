use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::errors::RepoDeskResult;

/// Read at most `max + 1` bytes so truncation can be detected without ever
/// materializing the rest of a potentially huge agent log in memory.
pub(super) fn read_bounded(path: &Path, max: usize) -> RepoDeskResult<(String, bool)> {
    let file = File::open(path)?;
    let (bytes, truncated) = read_prefix(file, max)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[output truncated]");
    }
    Ok((text, truncated))
}

fn read_prefix(reader: impl Read, max: usize) -> io::Result<(Vec<u8>, bool)> {
    let probe_len = max.saturating_add(1);
    let limit = u64::try_from(probe_len).unwrap_or(u64::MAX);
    let mut bytes = Vec::with_capacity(probe_len.min(64 * 1024));
    reader.take(limit).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > max;
    if truncated {
        bytes.truncate(max);
    }
    Ok((bytes, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn prefix_reader_keeps_small_streams_intact() {
        let (bytes, truncated) = read_prefix(Cursor::new(b"hello"), 8).unwrap();
        assert_eq!(bytes, b"hello");
        assert!(!truncated);
    }

    #[test]
    fn prefix_reader_reads_only_one_byte_past_the_budget() {
        let input = vec![b'x'; 1024 * 1024];
        let (bytes, truncated) = read_prefix(Cursor::new(input), 32).unwrap();
        assert_eq!(bytes, vec![b'x'; 32]);
        assert!(truncated);
    }

    #[test]
    fn bounded_file_reader_marks_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.log");
        std::fs::write(&path, b"abcdefghij").unwrap();

        let (text, truncated) = read_bounded(&path, 4).unwrap();
        assert_eq!(text, "abcd\n[output truncated]");
        assert!(truncated);
    }
}
