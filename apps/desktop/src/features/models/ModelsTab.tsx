import React from "react";
import { statusTone } from "../../shared/ui/SharedComponents";
import type { TabId } from "../../shared/types/api";
import { useModels } from "./useModels";

interface ModelsTabProps {
  setActiveTab: (tab: TabId) => void;
}

export function ModelsTab({ setActiveTab }: ModelsTabProps) {
  const { models, workingProviders, modelCount, refreshModels, isRefreshing } = useModels();
  const isBusy = isRefreshing;
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

      {(models?.providers ?? []).map((provider: any) => (
        <section className="panel provider-panel" key={provider.id}>
          <div className="panel-title-row">
            <div><p className="eyebrow">{provider.id}</p><h2>{provider.label}</h2></div>
            <span className={`pill ${statusTone(provider.reachability)}`}>{provider.reachability}</span>
          </div>
          <div className="provider-meta">
            <span>auth: {provider.auth_status}</span>
            <span>{provider.models.length} models</span>
          </div>
          {provider.error_summary && <div className="notice warn">{provider.error_summary}</div>}
          <div className="model-list">
            {provider.models.length === 0 ? <p className="muted">No models visible for this provider.</p> : provider.models.slice(0, 80).map((model: any) => (
              <div className="model-row" key={`${provider.id}-${model.id}`}>
                <strong>{model.id}</strong>
                {model.notes && <span>{model.notes}</span>}
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
