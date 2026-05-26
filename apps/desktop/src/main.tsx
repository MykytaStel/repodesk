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
  schema_version: number;
  tables: string[];
  mode: string;
};

type DesktopActionSpec = {
  id: string;
  label: string;
  risk: string;
  description: string;
};

type DesktopActionResult = {
  action: string;
  label: string;
  verdict: string;
  status: string;
  duration_ms: number;
  output: string;
  recorded_in_db: boolean;
};

type StoredActionRun = {
  id: number;
  created_at: string;
  action: string;
  verdict: string;
  status: string;
  duration_ms: number;
  output_preview: string;
};

type Tab = 'dashboard' | 'actions' | 'security' | 'runtime' | 'storage' | 'raw';

const tabs: Array<{ id: Tab; label: string }> = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'actions', label: 'Actions' },
  { id: 'security', label: 'Security' },
  { id: 'runtime', label: 'Runtime' },
  { id: 'storage', label: 'DB / Logs' },
  { id: 'raw', label: 'Raw' },
];

function valueText(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—';
  if (typeof value === 'boolean') return value ? 'yes' : 'no';
  return String(value);
}

function toneFor(value: unknown): 'good' | 'warn' | 'bad' | 'neutral' {
  const label = valueText(value).toLowerCase();
  if (label.includes('block') || label.includes('failed') || label.includes('missing') || label === 'no') return 'bad';
  if (label.includes('warn') || label.includes('medium') || label.includes('bounded')) return 'warn';
  if (label.includes('ok') || label.includes('yes') || label.includes('success') || label.includes('allow')) return 'good';
  return 'neutral';
}

function Pill({ value }: { value: unknown }) {
  return <span className={`pill ${toneFor(value)}`}>{valueText(value)}</span>;
}

function Card({ title, children, wide = false }: { title: string; children: React.ReactNode; wide?: boolean }) {
  return (
    <section className={`card ${wide ? 'wide' : ''}`}>
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
  const [tab, setTab] = useState<Tab>('dashboard');
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [security, setSecurity] = useState('');
  const [runtime, setRuntime] = useState('');
  const [sandbox, setSandbox] = useState('');
  const [localState, setLocalState] = useState<LocalStateStatus | null>(null);
  const [actions, setActions] = useState<DesktopActionSpec[]>([]);
  const [runs, setRuns] = useState<StoredActionRun[]>([]);
  const [lastResult, setLastResult] = useState<DesktopActionResult | null>(null);
  const [error, setError] = useState('');
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const [dashboardData, securityText, dbStatus, actionList, actionRuns, runtimeText, sandboxText] = await Promise.all([
        invoke<DashboardSnapshot>('dashboard_snapshot'),
        invoke<string>('security_audit_text'),
        invoke<LocalStateStatus>('local_state_status'),
        invoke<DesktopActionSpec[]>('desktop_actions'),
        invoke<StoredActionRun[]>('recent_action_runs', { limit: 12 }),
        invoke<string>('runtime_providers_text'),
        invoke<string>('sandbox_policy_text'),
      ]);
      setSnapshot(dashboardData);
      setSecurity(securityText);
      setLocalState(dbStatus);
      setActions(actionList);
      setRuns(actionRuns);
      setRuntime(runtimeText);
      setSandbox(sandboxText);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runAction = useCallback(async (action: string) => {
    setBusyAction(action);
    setError('');
    try {
      const result = await invoke<DesktopActionResult>('run_desktop_action', { action });
      setLastResult(result);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyAction(null);
    }
  }, [refresh]);

  const initDb = useCallback(async () => {
    setBusyAction('init_db');
    setError('');
    try {
      const result = await invoke<LocalStateStatus>('init_local_database');
      setLocalState(result);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyAction(null);
    }
  }, [refresh]);

  const rawSnapshot = useMemo(() => JSON.stringify(snapshot ?? {}, null, 2), [snapshot]);

  return (
    <main className="shell">
      <header className="hero">
        <div>
          <p className="eyebrow">RepoDesk Desktop</p>
          <h1>Control brain cockpit</h1>
          <p className="subtitle">
            Local-first command center for context, checks, AI routing, security, storage, and safe workflow actions.
          </p>
        </div>
        <div className="hero-actions">
          <button className="primary" type="button" onClick={() => void refresh()} disabled={loading || busyAction !== null}>
            {loading ? 'Refreshing…' : 'Refresh'}
          </button>
          <button className="secondary" type="button" onClick={() => void initDb()} disabled={busyAction !== null}>
            {busyAction === 'init_db' ? 'Initializing…' : 'Init DB'}
          </button>
        </div>
      </header>

      <nav className="tabs">
        {tabs.map((item) => (
          <button key={item.id} className={tab === item.id ? 'active' : ''} onClick={() => setTab(item.id)} type="button">
            {item.label}
          </button>
        ))}
      </nav>

      {error ? <div className="error">{error}</div> : null}

      {tab === 'dashboard' ? (
        <>
          <section className="grid top-grid">
            <Card title="Active work">
              <Metric label="Project" value={snapshot?.project_name} />
              <Metric label="Task" value={snapshot?.task_id} />
              <Metric label="Next action" value={snapshot?.next_action} />
            </Card>
            <Card title="Context health">
              <div className="row"><span>context.md</span><Pill value={snapshot?.context_exists} /></div>
              <div className="row"><span>smart-context.md</span><Pill value={snapshot?.smart_context_exists} /></div>
              <div className="row"><span>checks-summary.md</span><Pill value={snapshot?.checks_summary_exists} /></div>
            </Card>
            <Card title="Budget / tokens">
              <Metric label="Context tokens" value={snapshot?.context_tokens} />
              <div className="row"><span>Budget level</span><Pill value={snapshot?.budget_level ?? 'unknown'} /></div>
              <Metric label="Prompts" value={snapshot?.prompts_count} />
            </Card>
            <Card title="Repository signals">
              <Metric label="Files scanned" value={snapshot?.repo_files_scanned} />
              <Metric label="Hotspots" value={snapshot?.hotspots_count} />
              <Metric label="Mode" value="local-only" />
            </Card>
          </section>
          {lastResult ? <ActionResultPanel result={lastResult} /> : null}
        </>
      ) : null}

      {tab === 'actions' ? (
        <section className="grid action-grid">
          {actions.map((action) => (
            <article className="action-card" key={action.id}>
              <div>
                <div className="action-title">{action.label}</div>
                <p>{action.description}</p>
                <Pill value={action.risk} />
              </div>
              <button className="primary small" type="button" onClick={() => void runAction(action.id)} disabled={busyAction !== null}>
                {busyAction === action.id ? 'Running…' : 'Run'}
              </button>
            </article>
          ))}
          {lastResult ? <ActionResultPanel result={lastResult} wide /> : null}
        </section>
      ) : null}

      {tab === 'security' ? (
        <section className="grid two-grid">
          <Card title="Security audit" wide>
            <pre className="text-panel tall">{security || 'No security audit loaded yet.'}</pre>
          </Card>
          <Card title="Sandbox policy" wide>
            <pre className="text-panel tall">{sandbox || 'No sandbox policy loaded yet.'}</pre>
          </Card>
        </section>
      ) : null}

      {tab === 'runtime' ? (
        <Card title="Runtime providers" wide>
          <pre className="text-panel tall">{runtime || 'No runtime provider registry loaded yet.'}</pre>
        </Card>
      ) : null}

      {tab === 'storage' ? (
        <section className="grid two-grid">
          <Card title="SQLite state">
            <Metric label="RepoDesk home" value={localState?.repodesk_home} />
            <Metric label="Database" value={localState?.database_path} />
            <div className="row"><span>DB exists</span><Pill value={localState?.database_exists} /></div>
            <Metric label="Schema" value={localState?.schema_version} />
            <Metric label="Tables" value={localState?.tables?.length} />
            <Metric label="Runtime mode" value={localState?.mode} />
          </Card>
          <Card title="Recent action runs">
            <div className="runs">
              {runs.length === 0 ? <p className="muted">No action runs recorded yet.</p> : null}
              {runs.map((run) => (
                <div className="run" key={run.id}>
                  <div><strong>{run.action}</strong><span>{run.created_at}</span></div>
                  <div><Pill value={run.status} /><small>{run.duration_ms} ms</small></div>
                </div>
              ))}
            </div>
          </Card>
        </section>
      ) : null}

      {tab === 'raw' ? (
        <Card title="Raw dashboard snapshot" wide>
          <pre className="json-panel tall">{rawSnapshot}</pre>
        </Card>
      ) : null}
    </main>
  );
}

function ActionResultPanel({ result, wide = false }: { result: DesktopActionResult; wide?: boolean }) {
  return (
    <Card title="Last action result" wide={wide}>
      <div className="result-head">
        <strong>{result.label}</strong>
        <div className="result-meta">
          <Pill value={result.verdict} />
          <Pill value={result.status} />
          <span>{result.duration_ms} ms</span>
          <span>DB: {result.recorded_in_db ? 'yes' : 'no'}</span>
        </div>
      </div>
      <pre className="text-panel">{result.output}</pre>
    </Card>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
