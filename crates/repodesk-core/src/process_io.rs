//! Shared bounded process-I/O primitives.
//!
//! Readers always drain to EOF so a child cannot deadlock on a full pipe, while
//! retained memory and optionally persisted output stay within caller-provided
//! byte budgets.

use std::io::{self, Read, Write};

#[derive(Debug)]
pub(crate) struct BoundedBytes {
    pub(crate) bytes: Vec<u8>,
    pub(crate) truncated: bool,
}

#[derive(Debug)]
pub(crate) struct BoundedTee {
    pub(crate) bytes: Vec<u8>,
    pub(crate) retained_truncated: bool,
    pub(crate) persisted_truncated: bool,
}

pub(crate) fn drain_bounded_bytes(mut reader: impl Read, max: usize) -> io::Result<BoundedBytes> {
    let mut retained = Vec::with_capacity(max.min(64 * 1024));
    let mut buffer = [0u8; 8 * 1024];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        let remaining = max.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
        if keep < read {
            truncated = true;
        }
    }

    Ok(BoundedBytes {
        bytes: retained,
        truncated,
    })
}

/// Drain `reader` completely while retaining at most `retain_max` bytes in
/// memory and writing at most `persist_max` bytes to `writer`.
///
/// The two budgets are independent: a run record can stay compact while a larger
/// diagnostic prefix is persisted. Excess bytes are deliberately discarded only
/// after being read, which keeps the child pipe flowing without allowing either
/// RAM or per-stream disk usage to grow with untrusted output volume.
pub(crate) fn drain_bounded_to_writer(
    mut reader: impl Read,
    mut writer: impl Write,
    retain_max: usize,
    persist_max: usize,
) -> io::Result<BoundedTee> {
    let mut retained = Vec::with_capacity(retain_max.min(64 * 1024));
    let mut persisted = 0usize;
    let mut retained_truncated = false;
    let mut persisted_truncated = false;
    let mut buffer = [0u8; 8 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        let retain_remaining = retain_max.saturating_sub(retained.len());
        let retain = retain_remaining.min(read);
        retained.extend_from_slice(&buffer[..retain]);
        if retain < read {
            retained_truncated = true;
        }

        let persist_remaining = persist_max.saturating_sub(persisted);
        let persist = persist_remaining.min(read);
        if persist > 0 {
            writer.write_all(&buffer[..persist])?;
            persisted = persisted.saturating_add(persist);
        }
        if persist < read {
            persisted_truncated = true;
        }
    }

    writer.flush()?;
    Ok(BoundedTee {
        bytes: retained,
        retained_truncated,
        persisted_truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn drain_retains_only_the_budget_but_consumes_the_stream() {
        let input = vec![b'x'; 1024 * 1024];
        let capture = drain_bounded_bytes(Cursor::new(input), 32).unwrap();
        assert_eq!(capture.bytes, vec![b'x'; 32]);
        assert!(capture.truncated);
    }

    #[test]
    fn tee_drain_bounds_memory_and_persisted_bytes_independently() {
        let input = (0..=255).cycle().take(1024).collect::<Vec<u8>>();
        let mut persisted = Vec::new();

        let capture = drain_bounded_to_writer(Cursor::new(&input), &mut persisted, 32, 64).unwrap();

        assert_eq!(capture.bytes, input[..32]);
        assert_eq!(persisted, input[..64]);
        assert!(capture.retained_truncated);
        assert!(capture.persisted_truncated);
    }

    #[test]
    fn tee_drain_keeps_small_stream_complete() {
        let input = b"hello executor";
        let mut persisted = Vec::new();

        let capture = drain_bounded_to_writer(Cursor::new(input), &mut persisted, 64, 64).unwrap();

        assert_eq!(capture.bytes, input);
        assert_eq!(persisted, input);
        assert!(!capture.retained_truncated);
        assert!(!capture.persisted_truncated);
    }
}
