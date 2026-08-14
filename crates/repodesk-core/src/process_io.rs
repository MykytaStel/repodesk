//! Shared bounded process-I/O primitives.
//!
//! Readers always drain to EOF so a child cannot deadlock on a full pipe, while
//! retained memory stays within the caller-provided byte budget.

use std::io::{self, Read};

#[derive(Debug)]
pub(crate) struct BoundedBytes {
    pub(crate) bytes: Vec<u8>,
    pub(crate) truncated: bool,
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
