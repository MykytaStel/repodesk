import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CREDENTIAL_KEYS,
  credentialDelete,
  credentialSet,
  credentialStatus,
  type CredentialMetadata,
} from "../../shared/api/credentials";
import { queryKeys } from "../../shared/api/queries";
import { useToast } from "../../shared/ui/Toast";

const CREDENTIAL_LABELS: Record<string, string> = {
  [CREDENTIAL_KEYS.openai]: "OpenAI API key",
  [CREDENTIAL_KEYS.anthropic]: "Anthropic API key",
  [CREDENTIAL_KEYS.gemini]: "Gemini API key",
};

function sourceLabel(meta: CredentialMetadata | undefined): string {
  if (!meta || meta.source === "none") return "Not configured";
  if (meta.source === "keychain") return `Keychain · ${meta.hint}`;
  return `Environment · ${meta.hint}`;
}

function inputPlaceholder(meta: CredentialMetadata | undefined): string {
  if (meta?.source === "keychain") return "Enter a new value to replace the keychain credential";
  if (meta?.source === "environment") return "Enter a value to create a keychain override";
  return "Paste key to store securely";
}

export function CredentialsSection() {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const statusQuery = useQuery({
    queryKey: queryKeys.credentials.status,
    queryFn: credentialStatus,
  });

  const updateCredentialCache = (metadata: CredentialMetadata) => {
    queryClient.setQueryData<CredentialMetadata[]>(queryKeys.credentials.status, (current = []) => {
      const withoutCurrent = current.filter((entry) => entry.key !== metadata.key);
      return [...withoutCurrent, metadata];
    });
  };

  const refreshCredentialDependents = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.credentials.status }),
      queryClient.invalidateQueries({ queryKey: queryKeys.models.health }),
      queryClient.invalidateQueries({ queryKey: queryKeys.routing.apiEnv }),
    ]);
  };

  const save = async (key: string) => {
    const value = drafts[key]?.trim() ?? "";
    if (!value) return;

    setBusyKey(key);
    try {
      const metadata = await credentialSet(key, value);
      updateCredentialCache(metadata);
      setDrafts((current) => ({ ...current, [key]: "" }));
      await refreshCredentialDependents();
      toast.success("Credential stored in the OS keychain");
    } catch (error) {
      toast.error((error as { message?: string })?.message || "Could not store credential");
    } finally {
      setBusyKey(null);
    }
  };

  const remove = async (key: string) => {
    setBusyKey(key);
    try {
      const metadata = await credentialDelete(key);
      updateCredentialCache(metadata);
      await refreshCredentialDependents();
      toast.success(
        metadata.source === "environment"
          ? "Keychain override removed; environment fallback is active"
          : "Credential removed from the OS keychain",
      );
    } catch (error) {
      toast.error((error as { message?: string })?.message || "Could not remove credential");
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <section className="panel wide-panel flex-col gap-lg" aria-labelledby="credentials-heading">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Provider authentication</p>
          <h2 id="credentials-heading">Credentials</h2>
          <p className="lead">
            RepoDesk writes provider secrets only to your OS keychain. Environment variables remain
            read-only fallbacks, and the full credential is never returned to this UI.
          </p>
        </div>
      </div>

      {statusQuery.isError && (
        <div className="callout warning" role="alert">
          Credential status is unavailable. RepoDesk will not treat an unreadable keychain as an unconfigured credential.
        </div>
      )}

      <div className="flex-col gap-md">
        {Object.values(CREDENTIAL_KEYS).map((key) => {
          const meta = statusQuery.data?.find((entry) => entry.key === key);
          const label = CREDENTIAL_LABELS[key];
          const busy = busyKey === key;
          const canDelete = !statusQuery.isError && meta?.source === "keychain";
          const statusText = statusQuery.isLoading
            ? "Checking…"
            : statusQuery.isError
              ? "Status unavailable"
              : sourceLabel(meta);
          const statusTone = !statusQuery.isError && meta?.configured ? "ok" : "warn";

          return (
            <div key={key} className="form-stack">
              <label htmlFor={`credential-${key}`}>
                <span className="flex items-center gap-sm" style={{ marginBottom: 4 }}>
                  <strong>{label}</strong>
                  <span className={`pill ${statusTone}`}>{statusText}</span>
                </span>
                <div className="input-with-action">
                  <input
                    id={`credential-${key}`}
                    aria-label={label}
                    type="password"
                    autoComplete="off"
                    spellCheck={false}
                    value={drafts[key] ?? ""}
                    placeholder={inputPlaceholder(meta)}
                    onChange={(event) => setDrafts((current) => ({ ...current, [key]: event.target.value }))}
                  />
                  <button
                    type="button"
                    className="ghost-button"
                    aria-label={`Save ${label}`}
                    disabled={busyKey !== null || !(drafts[key] ?? "").trim()}
                    onClick={() => void save(key)}
                  >
                    {busy ? "Saving…" : "Save"}
                  </button>
                  {canDelete && (
                    <button
                      type="button"
                      className="ghost-button"
                      aria-label={`Delete ${label}`}
                      disabled={busyKey !== null}
                      onClick={() => void remove(key)}
                    >
                      Delete
                    </button>
                  )}
                </div>
              </label>
              {meta?.source === "environment" && (
                <span className="text-sm text-muted">
                  Read-only environment fallback. Saving a value here creates an OS-keychain override.
                </span>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
