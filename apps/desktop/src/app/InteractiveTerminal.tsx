import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  createTerminal,
  killTerminal,
  resizeTerminal,
  writeTerminal,
  type TerminalExitPayload,
  type TerminalOutputPayload,
  type TerminalSessionInfo,
} from "../shared/api/terminal";
import { errorToMessage } from "../shared/utils/helpers";

type TerminalStatus = "idle" | "starting" | "running" | "exited" | "error";

interface InteractiveTerminalProps {
  active: boolean;
}

function decodeBase64(value: string): Uint8Array {
  const binary = window.atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function cssVariable(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function terminalTheme() {
  return {
    background: cssVariable("--bg", "#14110d"),
    foreground: cssVariable("--text", "#f0e9dd"),
    cursor: cssVariable("--accent", "#d8964e"),
    cursorAccent: cssVariable("--bg", "#14110d"),
    selectionBackground: cssVariable("--accent-soft", "rgba(216, 150, 78, 0.18)"),
    black: "#202020",
    red: cssVariable("--danger", "#e57360"),
    green: cssVariable("--success", "#9ec27a"),
    yellow: cssVariable("--warning", "#e6c14d"),
    blue: "#7aa2c7",
    magenta: "#b48ead",
    cyan: "#78b8b0",
    white: cssVariable("--text", "#f0e9dd"),
    brightBlack: cssVariable("--muted-2", "#877b67"),
    brightRed: "#f08a78",
    brightGreen: "#b5d58f",
    brightYellow: "#f0d06a",
    brightBlue: "#93b8dc",
    brightMagenta: "#c9a0c4",
    brightCyan: "#93d0c8",
    brightWhite: "#fffdf9",
  };
}

export function InteractiveTerminal({ active }: InteractiveTerminalProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<TerminalSessionInfo | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const initializedRef = useRef(false);
  const inputBufferRef = useRef("");
  const inputTimerRef = useRef<number | null>(null);
  const resizeTimerRef = useRef<number | null>(null);
  const [session, setSession] = useState<TerminalSessionInfo | null>(null);
  const [status, setStatus] = useState<TerminalStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  const startSessionRef = useRef<() => Promise<void>>(async () => undefined);

  useEffect(() => {
    const host = hostRef.current;
    if (!active || initializedRef.current || !host) return;
    initializedRef.current = true;

    const terminal = new Terminal({
      cursorBlink: false,
      convertEol: false,
      disableStdin: false,
      fontFamily: cssVariable("--font-mono", "monospace"),
      fontSize: 12,
      lineHeight: 1.15,
      scrollback: 2_000,
      smoothScrollDuration: 0,
      theme: terminalTheme(),
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    fitAddon.fit();
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    let disposed = false;
    let outputUnlisten: UnlistenFn | null = null;
    let exitUnlisten: UnlistenFn | null = null;

    const flushInput = () => {
      inputTimerRef.current = null;
      const data = inputBufferRef.current;
      inputBufferRef.current = "";
      const current = sessionRef.current;
      if (!data || !current) return;
      void writeTerminal(current.session_id, data).catch((cause) => {
        if (!disposed) {
          setError(errorToMessage(cause));
          setStatus("error");
        }
      });
    };

    const queueInput = (data: string) => {
      inputBufferRef.current += data;
      if (inputBufferRef.current.length >= 4_096) {
        if (inputTimerRef.current !== null) window.clearTimeout(inputTimerRef.current);
        flushInput();
        return;
      }
      if (inputTimerRef.current === null) {
        inputTimerRef.current = window.setTimeout(flushInput, 8);
      }
    };

    const dataDisposable = terminal.onData(queueInput);

    const sendResize = () => {
      resizeTimerRef.current = null;
      fitAddon.fit();
      const current = sessionRef.current;
      if (!current) return;
      void resizeTerminal(current.session_id, terminal.rows, terminal.cols).catch(() => {
        // Resize is best-effort. The shell remains usable at its previous PTY size.
      });
    };

    const resizeObserver = new ResizeObserver(() => {
      if (resizeTimerRef.current !== null) window.clearTimeout(resizeTimerRef.current);
      resizeTimerRef.current = window.setTimeout(sendResize, 50);
    });
    resizeObserver.observe(host);

    const themeObserver = new MutationObserver(() => {
      terminal.options.theme = terminalTheme();
    });
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });

    const startSession = async () => {
      if (disposed) return;
      setStatus("starting");
      setError(null);
      try {
        fitAddon.fit();
        const created = await createTerminal(terminal.rows, terminal.cols);
        if (disposed) {
          void killTerminal(created.session_id).catch(() => undefined);
          return;
        }
        sessionRef.current = created;
        setSession(created);
        setStatus("running");
        terminal.focus();
      } catch (cause) {
        if (!disposed) {
          setStatus("error");
          setError(errorToMessage(cause));
        }
      }
    };
    startSessionRef.current = startSession;

    const attachOutput = listen<TerminalOutputPayload>("terminal-output", (event) => {
      if (event.payload.session_id !== sessionRef.current?.session_id) return;
      terminal.write(decodeBase64(event.payload.data));
    }).then((unlisten) => {
      if (disposed) unlisten();
      else outputUnlisten = unlisten;
    });

    const attachExit = listen<TerminalExitPayload>("terminal-exit", (event) => {
      if (event.payload.session_id !== sessionRef.current?.session_id) return;
      sessionRef.current = null;
      setStatus(event.payload.error ? "error" : "exited");
      setError(event.payload.error);
      terminal.write(
        `\r\n\x1b[2m[RepoDesk: shell exited${
          event.payload.exit_code == null ? "" : ` with code ${event.payload.exit_code}`
        }]\x1b[0m\r\n`,
      );
    }).then((unlisten) => {
      if (disposed) unlisten();
      else exitUnlisten = unlisten;
    });

    void Promise.all([attachOutput, attachExit]).then(() => startSession());

    cleanupRef.current = () => {
      disposed = true;
      const current = sessionRef.current;
      sessionRef.current = null;
      if (current) void killTerminal(current.session_id).catch(() => undefined);
      dataDisposable.dispose();
      resizeObserver.disconnect();
      themeObserver.disconnect();
      outputUnlisten?.();
      exitUnlisten?.();
      if (inputTimerRef.current !== null) window.clearTimeout(inputTimerRef.current);
      if (resizeTimerRef.current !== null) window.clearTimeout(resizeTimerRef.current);
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [active]);

  useEffect(
    () => () => {
      cleanupRef.current?.();
    },
    [],
  );

  useEffect(() => {
    if (!active) return;
    requestAnimationFrame(() => {
      fitAddonRef.current?.fit();
      terminalRef.current?.focus();
    });
  }, [active]);

  const restart = async () => {
    const current = sessionRef.current;
    if (current) {
      await killTerminal(current.session_id).catch(() => undefined);
      sessionRef.current = null;
    }
    terminalRef.current?.clear();
    await startSessionRef.current();
  };

  const stop = async () => {
    const current = sessionRef.current;
    if (!current) return;
    await killTerminal(current.session_id).catch((cause) => setError(errorToMessage(cause)));
  };

  return (
    <div className={`interactive-terminal${active ? " active" : ""}`} aria-hidden={!active}>
      <div className="interactive-terminal-toolbar">
        <div className="interactive-terminal-meta">
          <span className={`terminal-status-dot ${status}`} aria-hidden="true" />
          <strong>{status === "running" ? "Terminal" : status}</strong>
          <span title={session?.cwd}>{session?.cwd ?? "Active project shell"}</span>
          {session?.pid ? <small>PID {session.pid}</small> : null}
        </div>
        <div className="interactive-terminal-actions">
          <button type="button" onClick={() => void restart()} disabled={status === "starting"}>
            Restart
          </button>
          <button type="button" onClick={() => void stop()} disabled={status !== "running"}>
            Stop
          </button>
        </div>
      </div>
      {error ? <div className="interactive-terminal-error">{error}</div> : null}
      <div className="interactive-terminal-host" ref={hostRef} />
    </div>
  );
}
