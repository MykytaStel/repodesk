# RepoDesk Testing Guide

## Fast verification

Run before most commits:

```bash
./scripts/verify-fast.sh
```

This checks:

- Rust formatting
- Rust workspace compilation
- Desktop frontend build, when available

## Full verification

Run before push or PR:

```bash
./scripts/verify-all.sh
```

This checks:

- Rust formatting
- Rust workspace compilation
- Rust tests
- Desktop frontend build
- Basic secret scan

## Desktop manual smoke test

Start the app:

```bash
./scripts/dev-desktop.sh
```

Check:

1. Startup loader appears.
2. No blank white screen.
3. Active project is visible or setup flow is visible.
4. Active task is visible or task creation is visible.
5. Git tab shows branch and workspace status.
6. Workflow tab shows next safe step.
7. Running an action shows loading state.
8. Finished action shows toast/success/error.
9. Debug tab records command, duration, status, and result/error.
10. AI Discovery shows found/missing tools.

## Debug bundle

When something is unclear or broken:

```bash
./scripts/debug-bundle.sh
```

It writes:

```txt
.repodesk-debug/<timestamp>/
```

Share selected logs from that folder. Do not share secrets.

## Health report

Generate a local status report:

```bash
./scripts/health-report.sh
```

Output:

```txt
tmp/repodesk-health-report.md
```

## Git review before commit

```bash
git status --short
git diff --stat
git diff --cached --stat
```

Do not commit generated folders, secrets, or local debug bundles.
