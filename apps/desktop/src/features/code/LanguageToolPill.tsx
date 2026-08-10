import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  LANGUAGE_INTELLIGENCE_KEY,
  languageToolInstallCancel,
  languageToolInstallConfirm,
  languageToolInstallPreview,
  languageToolInstallStatus,
  type LanguageServerDescriptor,
  type LanguageServerStatus,
  type LanguageToolInstallPreview,
  type LanguageToolInstallStatus,
} from "../../shared/api/languageIntelligence";
import { errorToMessage } from "../../shared/utils/helpers";
import { LanguageInstallDialog } from "./LanguageInstallDialog";
import { LanguageToolPopover, type LanguageToolViewState } from "./LanguageToolPopover";
import "./language-tools.css";

function languageLabel(language: string): string {
  if (language === "typescript") return "TypeScript";
  if (language === "javascript") return "JavaScript";
  if (language === "toml") return "TOML";
  if (language === "json") return "JSON";
  if (language === "yaml") return "YAML";
  if (language === "rust") return "Rust";
  return language;
}

function stateLabel(state: LanguageToolViewState): string {
  if (state === "ready") return "Ready";
  if (state === "starting") return "Starting";
  if (state === "missing") return "Missing";
  if (state === "installing") return "Installing";
  if (state === "error") return "Error";
  return "Discovery only";
}

function syntheticStatus(
  recipeId: string,
  state: LanguageToolInstallStatus["state"],
  progress: number,
  message: string,
): LanguageToolInstallStatus {
  return {
    recipe_id: recipeId,
    state,
    progress,
    message,
    started_at: new Date().toISOString(),
    finished_at: state === "installing" ? null : new Date().toISOString(),
    error: state === "error" ? message : null,
  };
}

export function LanguageToolPill({
  language,
  server,
  sessionStatus,
  sessionError,
  onRetrySession,
}: {
  language: string;
  server: LanguageServerDescriptor;
  sessionStatus: LanguageServerStatus | null;
  sessionError: string | null;
  onRetrySession: () => void;
}) {
  const queryClient = useQueryClient();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<LanguageToolInstallPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [installStatus, setInstallStatus] = useState<LanguageToolInstallStatus | null>(null);
  const [installInFlight, setInstallInFlight] = useState(false);
  const [uiError, setUiError] = useState<string | null>(null);
  const recipeId = server.install_recipe_id;

  useEffect(() => {
    if (!open || !recipeId) return;
    let disposed = false;
    void languageToolInstallStatus(recipeId)
      .then((next) => {
        if (!disposed && next) setInstallStatus(next);
      })
      .catch(() => undefined);
    return () => { disposed = true; };
  }, [open, recipeId]);

  useEffect(() => {
    if (!installInFlight || !recipeId) return;
    let disposed = false;
    const poll = () => {
      void languageToolInstallStatus(recipeId)
        .then((next) => {
          if (!disposed && next) setInstallStatus(next);
        })
        .catch(() => undefined);
    };
    poll();
    const timer = window.setInterval(poll, 450);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [installInFlight, recipeId]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || preview) return;
      setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open, preview]);

  const viewState = useMemo<LanguageToolViewState>(() => {
    if (server.profile_state === "discovery_only") return "discovery_only";
    if (installStatus?.state === "installing") return "installing";
    if (
      uiError
      || installStatus?.state === "error"
      || sessionError
      || sessionStatus?.state === "error"
    ) return "error";
    if (installStatus?.state === "ready") return "ready";
    if (server.availability === "missing") return "missing";
    if (sessionStatus?.state === "ready") return "ready";
    return "starting";
  }, [installStatus?.state, server.availability, server.profile_state, sessionError, sessionStatus?.state, uiError]);

  const detail = uiError
    ?? installStatus?.error
    ?? sessionError
    ?? sessionStatus?.last_error
    ?? (installStatus?.state === "cancelled" ? installStatus.message : null)
    ?? (viewState === "missing"
      ? `${server.label} is not available for this project.`
      : viewState === "starting"
        ? `Starting ${server.label} for the active project.`
        : viewState === "ready"
          ? installStatus?.state === "ready"
            ? installStatus.message
            : `${server.label} is active for this document.`
          : null);

  const beginPreview = async () => {
    if (!recipeId || previewLoading) return;
    setUiError(null);
    setPreviewLoading(true);
    try {
      setPreview(await languageToolInstallPreview(recipeId));
      setOpen(true);
    } catch (cause) {
      setUiError(errorToMessage(cause));
      setOpen(true);
    } finally {
      setPreviewLoading(false);
    }
  };

  const confirmInstall = () => {
    if (!preview || !recipeId || installInFlight) return;
    const token = preview.confirmation_token;
    setPreview(null);
    setUiError(null);
    setInstallStatus(syntheticStatus(recipeId, "installing", 5, "Starting managed installation"));
    setInstallInFlight(true);
    setOpen(true);

    void languageToolInstallConfirm(token)
      .then(async (result) => {
        setInstallStatus(result.status);
        if (result.status.state === "ready") {
          await queryClient.invalidateQueries({ queryKey: LANGUAGE_INTELLIGENCE_KEY });
        }
      })
      .catch((cause) => {
        const message = errorToMessage(cause);
        setUiError(message);
        setInstallStatus(syntheticStatus(recipeId, "error", 0, message));
      })
      .finally(() => setInstallInFlight(false));
  };

  const cancelInstall = () => {
    if (!recipeId) return;
    void languageToolInstallCancel(recipeId)
      .then((cancelled) => {
        if (cancelled) {
          setInstallStatus(syntheticStatus(recipeId, "cancelled", installStatus?.progress ?? 0, "Installation cancelled"));
        }
      })
      .catch((cause) => setUiError(errorToMessage(cause)));
  };

  const retry = () => {
    setUiError(null);
    if (server.availability === "missing" || installStatus?.state === "error") {
      void beginPreview();
      return;
    }
    onRetrySession();
  };

  const label = languageLabel(language);
  const visualLabel = stateLabel(viewState);

  return (
    <div ref={rootRef} className="language-tool-control">
      <button
        type="button"
        className={`language-tool-pill code-language-service ${viewState}`}
        aria-expanded={open}
        aria-haspopup="dialog"
        aria-label={`${label} language tool: ${visualLabel}`}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="language-tool-state-dot" aria-hidden="true" />
        <strong>{label}</strong>
        <span>{viewState === "installing" && installStatus ? `${installStatus.progress}%` : visualLabel}</span>
      </button>

      {open ? (
        <LanguageToolPopover
          state={viewState}
          server={server}
          detail={detail}
          installStatus={installStatus}
          previewLoading={previewLoading}
          onInstall={() => { void beginPreview(); }}
          onRetry={retry}
          onCancel={cancelInstall}
          onClose={() => setOpen(false)}
        />
      ) : null}

      {preview ? (
        <LanguageInstallDialog
          preview={preview}
          confirming={installInFlight}
          error={uiError}
          onConfirm={confirmInstall}
          onClose={() => setPreview(null)}
        />
      ) : null}
    </div>
  );
}
