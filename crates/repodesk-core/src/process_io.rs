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

pub(crate) fn drain_bounded_bytes(
    mut reader: impl Read,
    max: usize,
) -> io::Result<BoundedBytes> {
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
}
