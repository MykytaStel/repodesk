import { useEffect, useMemo, useRef, useState } from "react";

export type Command = {
  id: string;
  label: string;
  hint?: string;
  run: () => void | Promise<void>;
};

/** Spotlight-style fuzzy command palette (opened with ⌘K / Ctrl-K). */
export function CommandPalette({ open, onClose, commands }: { open: boolean; onClose: () => void; commands: Command[] }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      // Focus after the element mounts.
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    // Subsequence match so "gst" still matches "Go to Settings"…
    const subseq = (text: string) => {
      let i = 0;
      for (const ch of q) {
        i = text.indexOf(ch, i);
        if (i === -1) return false;
        i++;
      }
      return true;
    };
    // …but rank contiguous-substring hits first, so typing "git" lands on
    // "Go to Git" rather than "Go to His(t)ory" (a subsequence-only match).
    const rank = (c: Command): number | null => {
      const label = c.label.toLowerCase();
      const hint = (c.hint ?? "").toLowerCase();
      if (label.includes(q)) return 0;
      if (hint.includes(q)) return 1;
      if (subseq(label) || subseq(hint)) return 2;
      return null;
    };
    return commands
      .map((c, index) => ({ c, index, score: rank(c) }))
      .filter((entry): entry is { c: Command; index: number; score: number } => entry.score !== null)
      .sort((a, b) => a.score - b.score || a.index - b.index)
      .map((entry) => entry.c);
  }, [commands, query]);

  if (!open) return null;

  const choose = (index: number) => {
    const cmd = filtered[index];
    if (cmd) {
      void cmd.run();
      onClose();
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(active);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };

  return (
    <div className="cmdk-overlay" onClick={onClose}>
      <div className="cmdk-panel" onClick={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          className="cmdk-input"
          placeholder="Search tabs and actions…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
        <div className="cmdk-list">
          {filtered.length === 0 ? (
            <p className="muted cmdk-empty">No matching commands.</p>
          ) : (
            filtered.map((cmd, i) => (
              <button
                key={cmd.id}
                className={`cmdk-item ${i === active ? "active" : ""}`}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(i)}
              >
                <span>{cmd.label}</span>
                {cmd.hint && <span className="cmdk-hint">{cmd.hint}</span>}
              </button>
            ))
          )}
        </div>
        <div className="cmdk-footer">
          <span>↑↓ navigate</span><span>↵ run</span><span>esc close</span>
        </div>
      </div>
    </div>
  );
}
