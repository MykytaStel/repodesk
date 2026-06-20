import React from "react";
import { statusTone } from "../../shared/ui/SharedComponents";
import type { TabId } from "../../shared/types/api";
import { useModels } from "./useModels";
import { useEffect, useState } from "react";
import { systemModelRecommendations, startLocalServer } from "../../shared/api/models";
import { useSettings } from "../settings/useSettings";
import { useToast } from "../../shared/ui/Toast";

interface ModelsTabProps {
  setActiveTab: (tab: TabId) => void;
}

export function ModelsTab({ setActiveTab }: ModelsTabProps) {
  const { models, workingProviders, modelCount, refreshModels, isRefreshing } = useModels();
  const { providerSettings, saveSettings } = useSettings();
  const toast = useToast();
  const [recommendations, setRecommendations] = useState<string[]>([]);

  useEffect(() => {
    systemModelRecommendations().then(setRecommendations).catch(console.error);
  }, []);

  const isBusy = isRefreshing;

  const handleLaunch = async (providerId: string) => {
    try {
      await startLocalServer(providerId);
      toast.success(`Launched ${providerId}, waiting for server...`);
      setTimeout(() => refreshModels(), 3000);
    } catch (error: any) {
      toast.error(error?.message || `Could not launch ${providerId}`);
    }
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

  return (
    <div className="content-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Models</p>
        <h1>{workingProviders} providers working, {modelCount} models visible.</h1>
        <p className="lead">RepoDesk checks local runtimes and enabled API providers live. API keys are read from environment variables only and are never displayed.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void refreshModels()} disabled={isBusy}>Refresh model health</button>
          <button className="ghost-button" onClick={() => setActiveTab("settings")}>Provider settings</button>
        </div>
      </section>

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
        {(models?.providers ?? []).map((provider: any) => (
          <section className="panel provider-panel" key={provider.id}>
            <div className="panel-title-row">
              <div><p className="eyebrow">{provider.id}</p><h2>{provider.label}</h2></div>
              <span className={`pill ${statusTone(provider.reachability)}`}>{provider.reachability}</span>
            </div>
            <div className="provider-meta" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <span>auth: {provider.auth_status}</span>
                <span style={{ marginLeft: "8px" }}>{provider.models.length} models</span>
              </div>
              {provider.reachability !== "working" && (provider.id === "ollama" || provider.id === "lm_studio") && (
                <button className="tiny-button" onClick={() => void handleLaunch(provider.id)}>Launch App</button>
              )}
            </div>
            {provider.error_summary && <div className="notice warn">{provider.error_summary}</div>}
            <div className="model-list">
              {provider.models.length === 0 ? <p className="muted">No models visible for this provider.</p> : provider.models.slice(0, 80).map((model: any) => {
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
          </section>
        ))}
      </div>
    </div>
  );
}
