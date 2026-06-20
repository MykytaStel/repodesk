import { ErrorState } from "../../shared/ui/ErrorState";
import { ActorBadge } from "../../shared/ui/SharedComponents";
import type { MemoryProposal } from "../../shared/api/memory";
import { AGENTS } from "./constants";

export function CapturePanel({
  agent,
  text,
  pending,
  error,
  result,
  onAgentChange,
  onTextChange,
  onCapture,
}: {
  agent: string;
  text: string;
  pending: boolean;
  error: unknown;
  result?: MemoryProposal[];
  onAgentChange: (agent: string) => void;
  onTextChange: (text: string) => void;
  onCapture: () => void;
}) {
  return (
    <section className="panel wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Merge between AI</p>
          <h2>Capture from a response</h2>
        </div>
        <ActorBadge mode="auto" />
      </div>
      <div className="form-stack">
        <label>
          Paste an AI response
          <textarea
            rows={4}
            placeholder="Paste what ChatGPT / Codex / Gemini / Ollama returned. RepoDesk extracts decisions, constraints, risks..."
            value={text}
            onChange={(event) => onTextChange(event.target.value)}
            style={{ resize: "vertical", minHeight: 90 }}
          />
        </label>
        <div className="settings-grid">
          <label>
            Produced by
            <select value={agent} onChange={(event) => onAgentChange(event.target.value)}>
              {AGENTS.map((item) => (
                <option key={item} value={item}>
                  {item}
                </option>
              ))}
            </select>
          </label>
        </div>
        {Boolean(error) && <ErrorState compact scope="memory:capture" error={error} />}
        {result && <div className="notice">Captured {result.length} candidate(s) - see the review queue below.</div>}
        <div className="button-row compact-buttons">
          <button className="primary-button" onClick={onCapture} disabled={pending || !text.trim()}>
            {pending ? "Extracting..." : "Extract memory"}
          </button>
        </div>
      </div>
    </section>
  );
}
