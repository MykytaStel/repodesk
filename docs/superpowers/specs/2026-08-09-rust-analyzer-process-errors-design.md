# rust-analyzer process error handling

## Goal

Make Rust language intelligence start successfully in the current development environment and make RepoDesk report an actionable root cause whenever a language-server process exits.

## Environment recovery

- Install the official `rust-analyzer` component into the active `stable-aarch64-apple-darwin` rustup toolchain.
- Verify the actual binary with `rust-analyzer --version` before testing RepoDesk.
- RepoDesk itself must not install toolchain components, invoke package managers, or make network-backed repairs automatically.

## Native process boundary

- Continue draining child stderr concurrently so a language server cannot block on a full pipe.
- Store only a bounded stderr tail for the active session; never retain unbounded server output.
- When stdout reaches EOF, inspect the child exit status and the captured stderr tail.
- Prefer a concrete message containing the server name, exit status, and stderr over the generic `closed its stdout stream` message.
- Fall back to the generic message only when neither exit status nor stderr is available.
- Preserve the existing bounded `last_error` contract exposed through `LanguageServerStatus`.
- Normal intentional shutdown must not overwrite the session with a misleading crash error.

## User experience

- Existing Rust language status UI continues to display `RA starting`, `RA ready`, or `RA error`.
- The error title/panel receives the actionable native message without requiring a React Error Boundary.
- A missing rustup component should surface the underlying `unknown binary 'rust-analyzer'` text and recommend `rustup component add rust-analyzer`.
- The UI must not claim that a repair was performed by RepoDesk.

## Testing

- Unit-test construction of an early-exit error with exit status and stderr.
- Unit-test bounded stderr behavior with output larger than the configured limit.
- Unit-test the generic fallback when no diagnostic evidence exists.
- Keep protocol framing and existing language-server tests green.
- Run the exact Clippy command with `-D warnings`, relevant Rust tests, and the frontend build/UI suite affected by status display.

## Scope constraints

- Do not redesign the LSP protocol client or add automatic retry loops in this slice.
- Do not add automatic toolchain mutation to RepoDesk.
- Preserve unrelated uncommitted formatting changes already present in `apps/desktop/src-tauri/src/language_server.rs`.
