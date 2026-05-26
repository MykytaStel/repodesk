# Debugging RepoDesk Desktop UI

## Where to look when something does not work

1. **Toast notifications**
   - Top-right messages show success/error for user-triggered actions.

2. **Debug tab**
   - Shows every Tauri command call.
   - Shows command name, args, duration, status, error and payload preview.

3. **Terminal running Tauri**
   - Backend Rust panics/build/runtime errors appear in the terminal where you ran:
     `./scripts/dev-desktop.sh`

4. **AI Discovery tab**
   - Shows exactly which tools were found/missing.
   - Shows local endpoints like Ollama and LM Studio.

5. **Artifacts tab**
   - Shows generated context/prompts/check summaries.
   - If missing, run the related Workflow action.

## Important

The UI intentionally does not run arbitrary shell commands. If a command is missing or blocked, it should appear in Debug instead of failing silently.
