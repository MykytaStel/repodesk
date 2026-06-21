import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import * as api from "../../shared/api/cost";
import type { AgentRate, CostConfig } from "../../shared/api/cost";

// Editable cost rate card (costs.toml). Rates are USD per 1K tokens. Lets the
// user replace RepoDesk's default estimates with their real plan economics so
// the cost ledger reflects actual spend instead of placeholder numbers.
export function RateCardEditor() {
  const config = useQuery({ queryKey: ["cost-config"], queryFn: api.getCostConfig });
  const [draft, setDraft] = useState<CostConfig | null>(null);
  const [savedAt, setSavedAt] = useState<string | null>(null);

  useEffect(() => {
    if (config.data) setDraft(config.data);
  }, [config.data]);

  const save = useMutation({
    mutationFn: (cfg: CostConfig) => api.saveCostConfig(cfg),
    onSuccess: (cfg) => {
      setDraft(cfg);
      setSavedAt(new Date().toLocaleTimeString());
    },
  });
  const reset = useMutation({
    mutationFn: () => api.resetCostConfig(),
    onSuccess: (cfg) => {
      setDraft(cfg);
      setSavedAt(new Date().toLocaleTimeString());
    },
  });

  if (config.isLoading || !draft) {
    return (
      <section className="panel wide-panel">
        <p className="muted">Loading rate card…</p>
      </section>
    );
  }

  const updateRate = (index: number, patch: Partial<AgentRate>) => {
    setDraft({
      ...draft,
      rates: draft.rates.map((r, i) => (i === index ? { ...r, ...patch } : r)),
    });
  };

  const error = (save.error as Error | null)?.message ?? (reset.error as Error | null)?.message ?? null;

  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Rate card</p>
          <h2>Cost rates ({draft.currency_label} per 1K tokens)</h2>
        </div>
        <span className="pill neutral" title="Defaults are list-price estimates; edit to match your plan.">
          {draft.rates.length} providers
        </span>
      </div>
      <p className="muted">
        These power the cost ledger and route estimates. Defaults are public list-price estimates —
        edit them to match your actual plan, then Save.
      </p>

      {error && <div className="notice danger">{error}</div>}

      <div className="rate-card-table">
        <div className="rate-card-head">
          <span>Provider</span>
          <span>Model</span>
          <span>Input /1K</span>
          <span>Output /1K</span>
        </div>
        {draft.rates.map((rate, index) => (
          <div className="rate-card-row" key={rate.agent}>
            <code title={rate.notes}>{rate.agent}</code>
            <input
              value={rate.model}
              onChange={(e) => updateRate(index, { model: e.target.value })}
            />
            <input
              type="number"
              step="0.0001"
              min="0"
              value={rate.input_cost_per_1k_units}
              onChange={(e) => updateRate(index, { input_cost_per_1k_units: Number(e.target.value) })}
            />
            <input
              type="number"
              step="0.0001"
              min="0"
              value={rate.output_cost_per_1k_units}
              onChange={(e) => updateRate(index, { output_cost_per_1k_units: Number(e.target.value) })}
            />
          </div>
        ))}
      </div>

      <div className="phase-actions">
        <button className="primary-button" onClick={() => save.mutate(draft)} disabled={save.isPending}>
          {save.isPending ? "Saving…" : "Save rates"}
        </button>
        <button
          className="ghost-button"
          onClick={() => reset.mutate()}
          disabled={reset.isPending}
          title="Restore the built-in default rate card"
        >
          Reset to defaults
        </button>
        {savedAt && <span className="muted">Saved {savedAt}.</span>}
      </div>
    </section>
  );
}
