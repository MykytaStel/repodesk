import React, { useCallback, useEffect, useMemo, useState } from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import './styles.css';

type DashboardSnapshot = {
  project_name?: string | null;
  task_id?: string | null;
  context_exists?: boolean;
  smart_context_exists?: boolean;
  checks_summary_exists?: boolean;
  prompts_count?: number;
  repo_files_scanned?: number;
  hotspots_count?: number;
  context_tokens?: number | null;
  budget_level?: string;
  next_action?: string;
  [key: string]: unknown;
};

type LocalStateStatus = {
  repodesk_home: string;
  database_path: string;
  database_exists: boolean;
  mode: string;
};

function valueText(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—';
  if (typeof value === 'boolean') return value ? 'yes' : 'no';
  return String(value);
}

function StatusPill({ value }: { value: unknown }) {
  const label = valueText(value);
  const normalized = label.toLowerCase();
  const tone = normalized.includes('block') || normalized.includes('missing') || normalized === 'no'
    ? 'bad'
    : normalized.includes('warn') || normalized.includes('medium')
      ? 'warn'
      : 'good';

  return <span className={`pill ${tone}`}>{label}</span>;
}

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="card">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function Metric({ label, value }: { label: string; value: unknown }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{valueText(value)}</strong>
    </div>
  );
}

function App() {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [security, setSecurity] = useState<string>('');
  const [localState, setLocalState] = useState<LocalStateStatus | null>(null);
  const [error, setError] = useState<string>('');
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const [dashboardData, securityText, dbStatus] = await Promise.all([
        invoke<DashboardSnapshot>('dashboard_snapshot'),
        invoke<string>('security_audit_text'),
        invoke<LocalStateStatus>('local_state_status'),
      ]);
      setSnapshot(dashboardData);
      setSecurity(securityText);
      setLocalState(dbStatus);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const rawSnapshot = useMemo(() => JSON.stringify(snapshot ?? {}, null, 2), [snapshot]);

  return (
    <main className="shell">
      <header className="hero">
        <div>
          <p className="eyebrow">RepoDesk Desktop</p>
          <h1>Control brain cockpit</h1>
          <p className="subtitle">
            Local-first UI for project state, security posture, budget awareness, and AI workflow control.
          </p>
        </div>
        <button className="primary" type="button" onClick={() => void refresh()} disabled={loading}>
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </header>

      {error ? <div className="error">{error}</div> : null}

      <section className="grid top-grid">
        <Card title="Active work">
          <Metric label="Project" value={snapshot?.project_name} />
          <Metric label="Task" value={snapshot?.task_id} />
          <Metric label="Next action" value={snapshot?.next_action} />
        </Card>

        <Card title="Context health">
          <div className="row"><span>context.md</span><StatusPill value={snapshot?.context_exists} /></div>
          <div className="row"><span>smart-context.md</span><StatusPill value={snapshot?.smart_context_exists} /></div>
          <div className="row"><span>checks-summary.md</span><StatusPill value={snapshot?.checks_summary_exists} /></div>
        </Card>

        <Card title="Budget / tokens">
          <Metric label="Context tokens" value={snapshot?.context_tokens} />
          <div className="row"><span>Budget level</span><StatusPill value={snapshot?.budget_level ?? 'unknown'} /></div>
          <Metric label="Prompts" value={snapshot?.prompts_count} />
        </Card>

        <Card title="Repository signals">
          <Metric label="Files scanned" value={snapshot?.repo_files_scanned} />
          <Metric label="Hotspots" value={snapshot?.hotspots_count} />
          <Metric label="Mode" value="local-only" />
        </Card>
      </section>

      <section className="grid bottom-grid">
        <Card title="Security posture">
          <pre className="text-panel">{security || 'No security audit loaded yet.'}</pre>
        </Card>

        <Card title="Local state / DB">
          <Metric label="RepoDesk home" value={localState?.repodesk_home} />
          <Metric label="Database" value={localState?.database_path} />
          <div className="row"><span>DB exists</span><StatusPill value={localState?.database_exists} /></div>
          <Metric label="Runtime mode" value={localState?.mode} />
        </Card>
      </section>

      <Card title="Raw dashboard snapshot">
        <pre className="json-panel">{rawSnapshot}</pre>
      </Card>
    </main>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
