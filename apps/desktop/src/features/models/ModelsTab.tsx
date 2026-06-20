import React from "react";
import type { TabId } from "../../shared/types/api";
import { useModels } from "./useModels";
import { useEffect, useState } from "react";
import { systemModelRecommendations, startLocalServer, type ProviderHealth } from "../../shared/api/models";
import { useSettings } from "../settings/useSettings";
import { useToast } from "../../shared/ui/Toast";

interface ModelsTabProps {
  setActiveTab: (tab: TabId) => void;
}

// Local engines we can launch directly; everything else is fixed via Settings.
const LAUNCHABLE = ["ollama", "lm_studio"];
const LOCAL_IDS = ["ollama", "lm_studio", "llamafile", "localai"];

type FixKind = "launch" | "enable" | "settings";
interface Guidance {
  // ready = good to go, attention = enabled but broken, off = disabled
  rank: 0 | 1 | 2;
  human: string;
  detail: string;
  tone: "ok" | "warn" | "neutral";
  fix?: { kind: FixKind; label: string };
}

/** Turn raw reachability/auth into a human status + one concrete fix. */
function providerGuidance(provider: ProviderHealth): Guidance {
  const reach = (provider.reachability || "").toLowerCase();
  const auth = (provider.auth_status || "").toLowerCase();
  const isLocal = LOCAL_IDS.includes(provider.id);
  const canLaunch = LAUNCHABLE.includes(provider.id);
  const modelCount = provider.models?.length ?? 0;

  if (reach === "working") {
    return { rank: 0, human: "Ready", detail: `${modelCount} model${modelCount === 1 ? "" : "s"} available.`, tone: "ok" };
  }
  if (reach === "disabled" || auth === "disabled") {
    return { rank: 2, human: "Off", detail: "Turned off in settings.", tone: "neutral", fix: { kind: "enable", label: "Turn on" } };
  }
  if (reach === "auth_missing" || auth === "auth_missing") {
    return { rank: 1, human: "Needs API key", detail: "Reachable, but the API key is missing or rejected.", tone: "warn", fix: { kind: "settings", label: "Add key" } };
  }
  if (reach === "rate_limited") {
    return { rank: 1, human: "Rate limited", detail: "The provider is throttling requests — try again shortly.", tone: "warn" };
  }
  if (reach === "unreachable") {
    return isLocal
      ? { rank: 1, human: "Offline", detail: "The local engine isn't running.", tone: "warn", fix: canLaunch ? { kind: "launch", label: "Launch app" } : { kind: "settings", label: "Check URL" } }
      : { rank: 1, human: "Unreachable", detail: "Could not reach the provider endpoint.", tone: "warn", fix: { kind: "settings", label: "Check settings" } };
  }
  if (reach === "manual" || auth === "not_required") {
    return { rank: 2, human: "Manual", detail: "Used by hand — no live health check.", tone: "neutral" };
  }
  if (reach === "unknown" || auth === "cli_auth") {
    return { rank: 2, human: "Unknown", detail: "Status can't be probed automatically.", tone: "neutral" };
  }
  return { rank: 1, human: provider.reachability || "unknown", detail: provider.error_summary || "", tone: "neutral" };
}

export function ModelsTab({ setActiveTab }: ModelsTabProps) {
  const { models, workingProviders, modelCount, refreshModels, isRefreshing } = useModels();
  const { providerSettings, saveSettings } = useSettings();
  const toast = useToast();
  const [recommendations, setRecommendations] = useState<string[]>([]);

  useEffect(() => {
    systemModelRecommendations()
      .then((recs) => setRecommendations(Array.isArray(recs) ? recs : []))
      .catch(console.error);
  }, []);

  const isBusy = isRefreshing;

  const handleLaunch = async (providerId: string) => {
    try {
      await startLocalServer(providerId);
      toast.success(`Launched ${providerId}, waiting for server…`);
      setTimeout(() => void refreshModels(), 3000);
    } catch (error: any) {
      toast.error(error?.message || `Could not launch ${providerId}`);
    }
  };

  const handleEnable = async (providerId: string) => {
    if (!providerSettings) return;
    try {
      await saveSettings({ ...providerSettings, [`${providerId}_enabled`]: true } as NonNullable<typeof providerSettings>);
      toast.success(`Turned on ${providerId}`);
      setTimeout(() => void refreshModels(), 500);
    } catch (error: any) {
      toast.error(error?.message || `Could not enable ${providerId}`);
    }
  };

  const runFix = (providerId: string, kind: FixKind) => {
    if (kind === "launch") void handleLaunch(providerId);
    else if (kind === "enable") void handleEnable(providerId);
    else setActiveTab("settings");
  };

  const setAsActive = async (providerId: string, modelId: string) => {
    if (!providerSettings) return;
    try {
      if (providerId === "ollama") {
        await saveSettings({ ...providerSettings, ollama_model: modelId });
        toast.success(`Set ${modelId} as active for Ollama`);
      } else {
        toast.info(`Setting active model for ${providerId} is not supported yet`);
      }
    } catch (error: any) {
      toast.error(error?.message || "Failed to set active model");
    }
  };

  const providers = models?.providers ?? [];
  // Surface what needs attention first, then disabled, then the ready ones.
  const ordered = [...providers].sort((a, b) => {
    const ra = providerGuidance(a).rank;
    const rb = providerGuidance(b).rank;
    return ra === rb ? 0 : ra === 1 ? -1 : rb === 1 ? 1 : ra - rb;
  });
  const needsAttention = providers.filter((p) => providerGuidance(p).rank === 1).length;

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Models</p>
        <h1>
          {workingProviders > 0
            ? `Ready for AI — ${workingProviders} provider${workingProviders === 1 ? "" : "s"} working`
            : "No models ready yet"}
        </h1>
        <p className="lead">
          {workingProviders > 0
            ? `${modelCount} model${modelCount === 1 ? "" : "s"} reachable. RepoDesk routes work to the cheapest capable one; local models handle most tasks.`
            : "RepoDesk can't run AI until at least one provider is reachable. Fix one below — start a local engine, or add an API key."}
        </p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void refreshModels()} disabled={isBusy}>
            {isBusy ? "Checking…" : "Re-check models"}
          </button>
          <button className="ghost-button" onClick={() => setActiveTab("settings")}>Provider settings</button>
        </div>
      </section>

      {workingProviders === 0 ? (
        <div className="notice danger wide-panel">
          <strong>Nothing reachable yet.</strong>
          <p style={{ margin: "4px 0 0" }}>
            Quickest path: install &amp; start <strong>Ollama</strong> (free, local) and pull a model, or add a paid API
            key in Settings. Then press <em>Re-check models</em>.
          </p>
        </div>
      ) : needsAttention > 0 ? (
        <div className="notice warn wide-panel">
          {needsAttention} provider{needsAttention === 1 ? "" : "s"} need attention below — but you're ready to work with what's already on.
        </div>
      ) : null}

      {(models?.warnings ?? []).map((warning: string) => <div className="notice warn wide-panel" key={warning}>{warning}</div>)}

      {recommendations.length > 0 && (
        <section className="panel wide-panel">
          <div className="panel-title-row">
            <div>
              <p className="eyebrow">System Analysis</p>
              <h2>Hardware Recommendations</h2>
            </div>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", marginTop: "12px" }}>
            {recommendations.map((rec, i) => (
              <p key={i} className={i === 0 ? "strong" : "text-sm text-muted"}>{rec}</p>
            ))}
          </div>
        </section>
      )}

      <div className="provider-grid">
        {ordered.map((provider) => {
          const g = providerGuidance(provider);
          return (
            <section className="panel provider-panel" key={provider.id}>
              <div className="panel-title-row">
                <div><p className="eyebrow">{provider.id}</p><h2>{provider.label}</h2></div>
                <span className={`pill ${g.tone}`}>{g.human}</span>
              </div>
              <p className="muted" style={{ margin: "0 0 10px", fontSize: 13 }}>{g.detail}</p>
              {g.fix && (
                <div className="button-row" style={{ marginBottom: 10 }}>
                  <button className="tiny-button" onClick={() => runFix(provider.id, g.fix!.kind)}>{g.fix.label}</button>
                </div>
              )}
              {provider.error_summary && g.tone === "warn" && <div className="notice warn">{provider.error_summary}</div>}
              {(provider.models?.length ?? 0) > 0 && (
                <div className="model-list">
                  {(provider.models ?? []).slice(0, 80).map((model: any) => {
                    const isActive = provider.id === "ollama" && providerSettings?.ollama_model === model.id;
                    return (
                      <div className="model-row" key={`${provider.id}-${model.id}`} style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <div>
                          <strong>{model.id}</strong>
                          {model.notes && <span style={{ display: "block", fontSize: "0.8em", color: "var(--muted)" }}>{model.notes}</span>}
                        </div>
                        {provider.id === "ollama" && (
                          <button
                            className={`tiny-button ${isActive ? "primary-button" : "ghost-button"}`}
                            onClick={() => void setAsActive(provider.id, model.id)}
                            disabled={isActive}
                          >
                            {isActive ? "Active" : "Set as active"}
                          </button>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </section>
          );
        })}
      </div>
    </div>
  );
}
