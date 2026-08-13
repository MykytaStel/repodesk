import { useCallback, useEffect, useRef, useState } from "react";

type DecisionOptions = {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
};

type PendingDecision = DecisionOptions & {
  resolve: (value: boolean) => void;
};

/**
 * Small modal decision boundary for editor-only destructive/recovery choices.
 * It deliberately returns a Promise so callers can keep their existing
 * sequential safety flow without falling back to browser-native confirm().
 */
export function useIdeDecisionDialog() {
  const [pending, setPending] = useState<PendingDecision | null>(null);
  const pendingRef = useRef<PendingDecision | null>(null);

  useEffect(() => {
    pendingRef.current = pending;
  }, [pending]);

  useEffect(() => () => {
    pendingRef.current?.resolve(false);
    pendingRef.current = null;
  }, []);

  const settle = useCallback((value: boolean) => {
    const current = pendingRef.current;
    if (!current) return;
    pendingRef.current = null;
    setPending(null);
    current.resolve(value);
  }, []);

  const confirm = useCallback((options: DecisionOptions): Promise<boolean> => {
    pendingRef.current?.resolve(false);
    return new Promise<boolean>((resolve) => {
      const request = { ...options, resolve };
      pendingRef.current = request;
      setPending(request);
    });
  }, []);

  useEffect(() => {
    if (!pending) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      settle(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [pending, settle]);

  return {
    confirm,
    dialog: pending ? (
      <div
        className="ide-dialog-backdrop"
        role="presentation"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) settle(false);
        }}
      >
        <section
          className="ide-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="code-decision-dialog-title"
          aria-describedby="code-decision-dialog-message"
        >
          <div className="ide-dialog-head">
            <strong id="code-decision-dialog-title">{pending.title}</strong>
            <span>RepoDesk editor</span>
          </div>
          <p id="code-decision-dialog-message" className="ide-dialog-message">{pending.message}</p>
          <div className="ide-dialog-actions">
            <button type="button" className="ghost-button" onClick={() => settle(false)} autoFocus>
              {pending.cancelLabel ?? "Cancel"}
            </button>
            <button
              type="button"
              className={pending.danger ? "danger-button" : "primary-button"}
              onClick={() => settle(true)}
            >
              {pending.confirmLabel}
            </button>
          </div>
        </section>
      </div>
    ) : null,
  };
}
