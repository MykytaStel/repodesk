import { useEffect, useRef } from "react";
import type { LanguageToolInstallPreview } from "../../shared/api/languageIntelligence";

function commandText(program: string, args: string[]): string {
  return [program, ...args].join(" ");
}

export function LanguageInstallDialog({
  preview,
  confirming,
  error,
  onConfirm,
  onClose,
}: {
  preview: LanguageToolInstallPreview;
  confirming: boolean;
  error: string | null;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || confirming) return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [confirming, onClose]);

  return (
    <div className="language-install-backdrop" role="presentation">
      <section
        className="language-install-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="language-install-title"
      >
        <header>
          <div>
            <span>Managed language tool</span>
            <h2 id="language-install-title">Install {preview.server_label}</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="language-tool-icon-button"
            onClick={onClose}
            disabled={confirming}
            aria-label="Close install confirmation"
          >×</button>
        </header>

        <div className="language-install-body">
          <p>
            RepoDesk will install a pinned tool outside the active repository and only promote it after a successful executable probe.
          </p>

          <dl>
            <div><dt>Package</dt><dd><code>{preview.package}@{preview.version}</code></dd></div>
            <div><dt>Languages</dt><dd>{preview.languages.join(", ")}</dd></div>
            <div><dt>Installer</dt><dd>{preview.installer}</dd></div>
            <div><dt>Destination</dt><dd><code>{preview.destination}</code></dd></div>
            <div><dt>Network</dt><dd>{preview.network_required ? "Required" : "Not required"}</dd></div>
          </dl>

          <div className="language-install-command">
            <span>Exact install command</span>
            <code>{commandText(preview.install_command.program, preview.install_command.args)}</code>
          </div>
          <div className="language-install-command">
            <span>Verification probe</span>
            <code>{commandText(preview.probe_command.program, preview.probe_command.args)}</code>
          </div>

          {preview.writes_outside_repository.length > 0 ? (
            <div className="language-install-writes">
              <span>May create files only under</span>
              {preview.writes_outside_repository.map((path) => <code key={path}>{path}</code>)}
            </div>
          ) : null}

          {!preview.prerequisite_available ? (
            <p className="language-tool-detail danger">
              {preview.prerequisite_hint ?? `${preview.installer} is required before RepoDesk can install this tool.`}
            </p>
          ) : null}
          {error ? <p className="language-tool-detail danger">{error}</p> : null}
        </div>

        <footer>
          <button type="button" onClick={onClose} disabled={confirming}>Cancel</button>
          <button
            type="button"
            className="primary"
            onClick={onConfirm}
            disabled={confirming || !preview.prerequisite_available}
          >
            {confirming ? "Starting installation…" : "Install language server"}
          </button>
        </footer>
      </section>
    </div>
  );
}
