# RepoDesk

Personal local AI operations hub for managing AI-assisted development workflows.

## Slice 1

This first slice implements:

- `repodesk init`
- `repodesk project add`
- `repodesk project list`
- `repodesk project use`
- `repodesk project info`

## Run locally

```bash
cargo run -p repodesk-cli -- init
cargo run -p repodesk-cli -- project add repopilot ~/Documents/projects/repopilot --type rust-cli
cargo run -p repodesk-cli -- project list
cargo run -p repodesk-cli -- project use repopilot
cargo run -p repodesk-cli -- project info
```

For safe testing without touching your real home config:

```bash
REPODESK_HOME=/tmp/repodesk-dev cargo run -p repodesk-cli -- init
```
