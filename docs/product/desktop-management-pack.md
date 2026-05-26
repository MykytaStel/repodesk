# Desktop Management Pack

Recommended branch: `feature/desktop-management-pack`

This pack turns the desktop MVP from a mostly read-only cockpit into a usable local management interface.

## Adds

- Project management panel
- Task management panel
- Safe Tauri commands for project/task operations
- Action whitelist remains enforced
- No unrestricted shell from UI
- Better workflow copy for the product
- Frontend API wrappers
- Rust tests for management validation and action allowlist

## Security rule

The UI still cannot execute arbitrary shell commands. It can only call explicit Tauri commands that internally invoke known RepoDesk CLI flows with structured arguments.

