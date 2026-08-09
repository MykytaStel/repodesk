# rust-analyzer Process Errors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Start the locally configured Rust language server successfully and replace generic stdout-closure errors with bounded, actionable process diagnostics.

**Architecture:** Extend the existing `SessionInner` process boundary with an atomic intentional-shutdown flag and a mutex-protected bounded stderr tail. The stderr reader records output and signals completion; the stdout reader briefly waits for that signal at EOF, inspects the child status, and builds one actionable error that flows through the existing `LanguageServerStatus.last_error` contract.

**Tech Stack:** Rust, Tauri, rustup, Cargo tests, Clippy

## Global Constraints

- RepoDesk must not install toolchain components or make network-backed repairs automatically.
- Retained stderr must remain bounded by `MAX_SERVER_ERROR_CHARS`.
- Intentional shutdown must not be reported as a crash.
- Preserve unrelated formatting-only changes already present in `apps/desktop/src-tauri/src/language_server.rs`.
- Do not create a git commit unless the user explicitly requests one.

---

### Task 1: Bounded actionable child-process errors

**Files:**
- Modify: `apps/desktop/src-tauri/src/language_server.rs`
- Test: `apps/desktop/src-tauri/src/language_server.rs` (`tests` module)

**Interfaces:**
- Produces: `append_bounded_stderr(target: &mut String, line: &str, max_chars: usize)`.
- Produces: `language_server_exit_error(server_id: &str, exit_status: Option<&str>, stderr: &str) -> String`.
- Extends: `SessionInner` with `stderr_tail: Mutex<String>` and `stopping: AtomicBool`.

- [x] **Step 1: Write failing helper tests**

Add tests asserting:

```rust
#[test]
fn early_exit_reports_status_stderr_and_rustup_repair() {
    let error = language_server_exit_error(
        "rust-analyzer",
        Some("exit status: 1"),
        "error: unknown binary 'rust-analyzer' in toolchain 'stable'",
    );
    assert!(error.contains("exit status: 1"));
    assert!(error.contains("unknown binary 'rust-analyzer'"));
    assert!(error.contains("rustup component add rust-analyzer"));
}

#[test]
fn stderr_tail_is_bounded() {
    let mut tail = String::new();
    append_bounded_stderr(&mut tail, "0123456789", 8);
    assert_eq!(tail.chars().count(), 8);
    assert!(tail.ends_with("3456789"));
}

#[test]
fn exit_without_evidence_uses_generic_fallback() {
    assert_eq!(
        language_server_exit_error("rust-analyzer", None, ""),
        "rust-analyzer closed its stdout stream",
    );
}
```

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p repodesk-desktop language_server::tests -- --nocapture`

Expected: compilation fails because both helper functions are undefined.

- [x] **Step 3: Implement bounded stderr and error construction**

Implement both helpers. The exit error must include the status when present, a trimmed stderr tail when present, the rustup repair hint only for the known missing-component message, and the generic fallback only when both pieces of evidence are absent.

- [x] **Step 4: Connect helpers to the process lifecycle**

Add `stderr_tail` and `stopping` to `SessionInner`. Start the stderr reader before the stdout reader, record each stderr line with `append_bounded_stderr`, and use an `mpsc` completion channel so EOF handling can wait at most 100 ms for final stderr. At stdout EOF, skip error reporting when `stopping` is true; otherwise call `child.try_wait()`, build the error with `language_server_exit_error`, set it on the session, and emit status. Set `stopping` before intentional exit/kill in both `shutdown` and `Drop`.

- [x] **Step 5: Run native tests and strict Clippy**

Run:

```bash
cargo test -p repodesk-desktop language_server::tests -- --nocapture
cargo clippy -p repodesk-desktop --all-targets --all-features -- -D warnings
git diff --check
```

Expected: all language-server tests pass, Clippy exits successfully, and diff check is clean.

---

### Task 2: Recover and verify the active Rust toolchain

**Files:**
- External toolchain state: `stable-aarch64-apple-darwin`

**Interfaces:**
- Consumes: rustup's `stable-aarch64-apple-darwin` toolchain.
- Produces: a working `rust-analyzer` component discoverable through the existing rustup shim.

- [x] **Step 1: Install the component explicitly**

Run: `rustup component add rust-analyzer --toolchain stable-aarch64-apple-darwin`

Expected: rustup reports that `rust-analyzer` was installed or is up to date.

- [x] **Step 2: Verify the executable and repository gates**

Run:

```bash
rust-analyzer --version
cargo clippy -p repodesk-desktop --all-targets --all-features -- -D warnings
cargo test -p repodesk-desktop language_server::tests -- --nocapture
pnpm --dir apps/desktop build
pnpm --dir apps/desktop exec playwright test
git diff --check
```

Expected: the binary prints a version, Rust gates pass, the frontend production build succeeds, all UI tests pass, and diff check is clean.
