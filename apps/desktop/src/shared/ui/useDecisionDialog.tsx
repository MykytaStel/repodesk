import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog } from "./Dialog";

export type DecisionOptions = {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  contextLabel?: string;
};

type PendingDecision = DecisionOptions & {
  resolve: (value: boolean) => void;
};

export function useDecisionDialog() {
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

  return {
    confirm,
    dialog: pending ? (
      <Dialog
        open
        title={pending.title}
        eyebrow={pending.contextLabel ?? "RepoDesk decision"}
        onClose={() => settle(false)}
        maxWidth="520px"
        footer={(
          <>
            <button
              type="button"
              className="ghost-button"
              data-dialog-initial-focus
              onClick={() => settle(false)}
            >
              {pending.cancelLabel ?? "Cancel"}
            </button>
            <button
              type="button"
              className={pending.danger ? "danger-button" : "primary-button"}
              onClick={() => settle(true)}
            >
              {pending.confirmLabel}
            </button>
          </>
        )}
      >
        <p className="app-dialog-message">{pending.message}</p>
      </Dialog>
    ) : null,
  };
}
