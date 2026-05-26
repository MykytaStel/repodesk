# RepoDesk Security Model

RepoDesk is local-first and should treat every AI/runtime/provider as a bounded module, not as a trusted operator.

## Core rules

1. The desktop UI must not expose unrestricted shell execution.
2. All desktop actions must go through explicit Rust/Tauri allowlists.
3. Secret files must not be sent to AI context.
4. Paid agents must only receive bounded context packs.
5. Local tools like Ollama are preferred for compression and low-risk review.
6. Patch/execution agents require guard/judge checks before action.
7. Debug output should be visible, but must avoid leaking secrets.

## Blocked or sensitive paths

- .env
- .env.*
- *.pem
- *.key
- credentials.*
- secrets.*
- id_rsa
- id_ed25519
- node_modules/
- target/
- .git/

## Desktop action policy

Allowed actions should be named, explicit, and auditable.

Good:

- build_context
- build_smart_context
- safety_scan
- run_project_checks
- generate_prompts
- git_workspace_snapshot

Bad:

- run_shell_command
- execute_script
- arbitrary_command
- write_file_anywhere
- read_secret_file

## AI provider policy

Local providers:

- Allowed for summaries, compression, low-risk analysis.
- Must remain local-only unless explicitly configured otherwise.

Paid/cloud providers:

- Disabled by default where possible.
- Must show token/cost/security warning before use.
- Should receive smart-context, not full repository dumps.

## Logging policy

Store:

- action id
- command/action name
- timestamp
- duration
- status
- short error/result summary

Avoid storing:

- API keys
- auth headers
- raw secret files
- large raw logs without filtering
