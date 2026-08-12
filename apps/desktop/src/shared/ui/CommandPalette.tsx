import { useEffect, useMemo, useRef, useState } from "react";

export type Command = {
  id: string;
  label: string;
  hint?: string;
  group?: string;
  keywords?: string[];
  shortcut?: string;
  priority?: number;
  run: () => void | Promise<void>;
};

const GROUP_ORDER = ["Current", "Files", "Navigate", "Work", "Projects", "View", "Appearance", "Other"];

function groupRank(group: string): number {
  const index = GROUP_ORDER.indexOf(group);
  return index === -1 ? GROUP_ORDER.length : index;
}

/** IDE-style command + quick-open palette (⌘K / Ctrl-K, or ⌘⇧P / Ctrl-Shift-P). */
export function CommandPalette({ open, onClose, commands }: { open: boolean; onClose: () => void; commands: Command[] }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const subseq = (text: string) => {
      let i = 0;
      for (const ch of q) {
        i = text.indexOf(ch, i);
        if (i === -1) return false;
        i++;
      }
      return true;
    };
    const rank = (command: Command): number | null => {
      if (!q) return 0;
      const label = command.label.toLowerCase();
      const hint = (command.hint ?? "").toLowerCase();
      const keywords = (command.keywords ?? []).join(" ").toLowerCase();
      if (label.startsWith(q)) return 0;
      if (label.includes(q)) return 1;
      if (hint.includes(q) || keywords.includes(q)) return 2;
      if (subseq(label) || subseq(hint) || subseq(keywords)) return 3;
      return null;
    };

    return commands
      .map((command, index) => ({
        command,
        index,
        score: rank(command),
        group: command.group ?? "Other",
      }))
      .filter((entry): entry is { command: Command; index: number; score: number; group: string } => entry.score !== null)
      .sort((a, b) => {
        if (q && a.score !== b.score) return a.score - b.score;
        const groupDelta = groupRank(a.group) - groupRank(b.group);
        if (groupDelta !== 0) return groupDelta;
        const priorityDelta = (b.command.priority ?? 0) - (a.command.priority ?? 0);
        return priorityDelta || a.index - b.index;
      })
      .map((entry) => entry.command);
  }, [commands, query]);

  const grouped = useMemo(() => {
    const groups: Array<{ name: string; commands: Array<{ command: Command; flatIndex: number }> }> = [];
    filtered.forEach((command, flatIndex) => {
      const name = command.group ?? "Other";
      const previous = groups[groups.length - 1];
      if (!previous || previous.name !== name) groups.push({ name, commands: [] });
      groups[groups.length - 1].commands.push({ command, flatIndex });
    });
    return groups;
  }, [filtered]);

  useEffect(() => {
    if (active >= filtered.length) setActive(Math.max(0, filtered.length - 1));
  }, [active, filtered.length]);

  if (!open) return null;

  const choose = (index: number) => {
    const cmd = filtered[index];
    if (!cmd) return;
    void cmd.run();
    onClose();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((value) => Math.min(value + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((value) => Math.max(value - 1, 0));
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
      <div className="cmdk-panel cmdk-panel-v2" onClick={(e) => e.stopPropagation()} role="dialog" aria-label="RepoDesk command palette">
        <div className="cmdk-search-row">
          <span aria-hidden="true">⌘</span>
          <input
            ref={inputRef}
            className="cmdk-input"
            placeholder="Type a command, surface, project, or file…"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            onKeyDown={onKeyDown}
          />
          <kbd>esc</kbd>
        </div>
        <div className="cmdk-list cmdk-list-v2">
          {filtered.length === 0 ? (
            <div className="cmdk-zero-state">
              <strong>No matching command</strong>
              <span>Try a surface name, repository path, project, or action.</span>
            </div>
          ) : (
            grouped.map((group) => (
              <section className="cmdk-group" key={`${group.name}-${group.commands[0]?.flatIndex ?? 0}`}>
                <div className="cmdk-group-label">{group.name}</div>
                {group.commands.map(({ command, flatIndex }) => (
                  <button
                    key={command.id}
                    className={`cmdk-item ${flatIndex === active ? "active" : ""}`}
                    onMouseEnter={() => setActive(flatIndex)}
                    onClick={() => choose(flatIndex)}
                  >
                    <span className="cmdk-item-copy">
                      <strong>{command.label}</strong>
                      {command.hint ? <small>{command.hint}</small> : null}
                    </span>
                    {command.shortcut ? <kbd>{command.shortcut}</kbd> : null}
                  </button>
                ))}
              </section>
            ))
          )}
        </div>
        <div className="cmdk-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
          <span><kbd>↵</kbd> run</span>
          <span>{filtered.length} result{filtered.length === 1 ? "" : "s"}</span>
        </div>
      </div>
    </div>
  );
}
