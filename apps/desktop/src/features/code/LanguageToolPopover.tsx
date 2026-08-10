import type {
  LanguageServerDescriptor,
  LanguageToolInstallStatus,
} from "../../shared/api/languageIntelligence";

export type LanguageToolViewState =
  | "ready"
  | "starting"
  | "missing"
  | "installing"
  | "error"
  | "discovery_only";

const CAPABILITY_LABELS: Array<[keyof LanguageServerDescriptor["capabilities"], string]> = [
  ["diagnostics", "Diagnostics"],
  ["hover", "Hover"],
  ["definition", "Definitions"],
  ["references", "References"],
  ["document_symbols", "Symbols"],
  ["completion", "Completion"],
  ["rename", "Rename"],
  ["formatting", "Formatting"],
];

function sourceLabel(server: LanguageServerDescriptor): string {
  if (server.source === "project_local") return "Project local";
  if (server.source === "managed") return "RepoDesk managed";
  if (server.source === "path") return "System PATH";
  return "Not discovered";
}

export function LanguageToolPopover({
  state,
  server,
  detail,
  installStatus,
  previewLoading,
  onInstall,
  onRetry,
  onCancel,
  onClose,
}: {
  state: LanguageToolViewState;
  server: LanguageServerDescriptor;
  detail: string | null;
  installStatus: LanguageToolInstallStatus | null;
  previewLoading: boolean;
  onInstall: () => void;
  onRetry: () => void;
  onCancel: () => void;
  onClose: () => void;
}) {
  const capabilities = CAPABILITY_LABELS
    .filter(([key]) => server.capabilities[key])
    .map(([, label]) => label);
  const canInstall = server.profile_state === "active" && Boolean(server.install_recipe_id);

  return (
    <aside className="language-tool-popover" role="dialog" aria-label={`${server.label} language tool`}>
      <header>
        <div>
          <strong>{server.label}</strong>
          <span>{sourceLabel(server)}</span>
        </div>
        <button type="button" className="language-tool-icon-button" onClick={onClose} aria-label="Close language tool details">×</button>
      </header>

      <div className="language-tool-popover-body">
        <div className="language-tool-kv">
          <span>Executable</span>
          <code>{server.executable}</code>
        </div>
        <div className="language-tool-kv">
          <span>Languages</span>
          <strong>{server.languages.join(", ")}</strong>
        </div>

        {detail ? <p className={`language-tool-detail ${state === "error" ? "danger" : ""}`}>{detail}</p> : null}

        {installStatus ? (
          <div className="language-tool-progress" aria-live="polite">
            <div>
              <span>{installStatus.message}</span>
              <strong>{installStatus.progress}%</strong>
            </div>
            <progress max={100} value={installStatus.progress} />
          </div>
        ) : null}

        <div className="language-tool-capabilities" aria-label="Advertised capabilities">
          {capabilities.length > 0
            ? capabilities.map((capability) => <span key={capability}>{capability}</span>)
            : <span>Discovery only</span>}
        </div>

        {server.profile_state === "discovery_only" ? (
          <p className="language-tool-note">RepoDesk can discover this server, but live support is not enabled for this profile yet.</p>
        ) : null}
      </div>

      <footer>
        {state === "missing" && canInstall ? (
          <button type="button" className="primary" onClick={onInstall} disabled={previewLoading}>
            {previewLoading ? "Preparing…" : "Install"}
          </button>
        ) : null}
        {state === "installing" ? (
          <button type="button" onClick={onCancel}>Cancel installation</button>
        ) : null}
        {state === "error" ? (
          <button type="button" onClick={onRetry}>Retry</button>
        ) : null}
      </footer>
    </aside>
  );
}
