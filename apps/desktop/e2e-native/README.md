# Native E2E (tauri-driver + WebdriverIO)

Real-backend end-to-end smoke: drives the **compiled** RepoDesk binary (WebView
frontend → Tauri IPC → `repodesk-core`) through `tauri-driver`.

## Platform support
`tauri-driver` works on **Linux** (WebKitWebDriver) and **Windows** (Edge WebDriver)
only — **not macOS** (WKWebView exposes no WebDriver). On a Mac, use the Playwright
mock-IPC smoke instead: `pnpm --dir apps/desktop e2e`. This native harness runs in CI
on Linux (see `.github/workflows/e2e-native.yml`).

## Run on Linux
```bash
# 1. Driver + WebKitWebDriver
cargo install tauri-driver --locked
sudo apt-get install -y webkit2gtk-driver xvfb        # provides WebKitWebDriver

# 2. Build the app
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop tauri build

# 3. Install harness deps + run (throwaway REPODESK_HOME = first-run state)
pnpm --dir apps/desktop/e2e-native install
REPODESK_HOME="$(mktemp -d)" xvfb-run -a pnpm --dir apps/desktop/e2e-native test
```

Or just: `./scripts/e2e-native.sh` from the repo root.

## Knobs
- `TAURI_APP_BINARY` — explicit path to the built binary (default: auto-resolve
  `src-tauri/target/release/{repodesk-desktop,RepoDesk}`).
- `TAURI_DRIVER` — path to the `tauri-driver` binary (default: on PATH).
