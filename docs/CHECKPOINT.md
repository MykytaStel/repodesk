# RepoDesk Checkpoint

Use this document before committing or pushing.

## Local verification

Run:

```bash
./scripts/verify-fast.sh
./scripts/secret-scan-basic.sh
./scripts/health-report.sh
```

Before major commits, run:

```bash
./scripts/verify-all.sh
```

## Manual desktop smoke test

1. Start desktop:

```bash
./scripts/dev-desktop.sh
```

2. Verify in UI:

- Startup loader appears.
- Active project is visible.
- Active task is visible or setup flow is shown.
- Git tab shows branch and changed files.
- Workflow tab shows recommended next step.
- Actions show loading state and toast result.
- Debug tab records command name, duration, status, and error/result preview.
- AI Discovery shows found/missing tools and local endpoints.

## Commit safety checklist

Before commit:

```bash
git status --short
git diff --stat
git diff --cached --stat
```

Do not commit:

- target/
- node_modules/
- dist/
- .env or .env.*
- *.pem / *.key
- generated debug bundles
- temporary logs

## Recommended commit point

Commit when:

- cargo fmt passes
- cargo check passes
- cargo test passes
- desktop build passes
- UI smoke test passes
- secret scan shows no obvious secrets
