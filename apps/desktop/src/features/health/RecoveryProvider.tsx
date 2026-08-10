import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  RECOVERY_HISTORY_QUERY_KEY,
  RECOVERY_QUERY_KEY,
  recoveryCheck,
  recoveryHistory,
  recoveryRepairCancel,
  recoveryRepairConfirm,
  recoveryRepairPreview,
  recoverySnapshot,
  subscribeRecoveryChanges,
  type RecoveryAttempt,
  type RecoveryRecord,
  type RecoveryRepairPreview,
  type RecoverySnapshot,
} from "../../shared/api/recovery";

const ACTIONABLE_STATES = new Set(["degraded", "needs_approval", "blocked"]);

type RecoveryContextValue = {
  snapshot: RecoverySnapshot | null;
  history: RecoveryAttempt[];
  selected: RecoveryRecord | null;
  previewState: RecoveryRepairPreview | null;
  mutationProgress: string | null;
  mutationError: string | null;
  panelOpen: boolean;
  openHealth: (capabilityId?: string) => void;
  closeHealth: () => void;
  dismissPreview: () => void;
  check: (capabilityId: string) => Promise<RecoveryRecord>;
  preview: (capabilityId: string, actionId: string) => Promise<RecoveryRepairPreview>;
  confirm: (confirmationToken: string) => Promise<RecoveryRecord>;
  cancel: (recipeId: string) => Promise<boolean>;
};

const RecoveryContext = createContext<RecoveryContextValue | null>(null);

function actionableCount(records: RecoveryRecord[]): number {
  return records.filter((record) => ACTIONABLE_STATES.has(record.state)).length;
}

function mergeRecord(
  snapshot: RecoverySnapshot | null | undefined,
  nextRecord: RecoveryRecord,
): RecoverySnapshot | null | undefined {
  if (!snapshot) return snapshot;
  const current = snapshot.records.find(
    (record) => record.capability_id === nextRecord.capability_id,
  );
  if (current && nextRecord.generation < current.generation) return snapshot;
  const records = current
    ? snapshot.records.map((record) =>
        record.capability_id === nextRecord.capability_id ? nextRecord : record,
      )
    : [...snapshot.records, nextRecord];
  return {
    ...snapshot,
    records,
    actionable_count: actionableCount(records),
    generated_at: nextRecord.observed_at,
  };
}

function recoveryError(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "Recovery operation failed";
}

export function RecoveryProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const [panelOpen, setPanelOpen] = useState(false);
  const [selectedCapabilityId, setSelectedCapabilityId] = useState<string | null>(null);
  const [previewState, setPreviewState] = useState<RecoveryRepairPreview | null>(null);
  const [mutationProgress, setMutationProgress] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const snapshotQuery = useQuery({
    queryKey: RECOVERY_QUERY_KEY,
    queryFn: recoverySnapshot,
  });
  const historyQuery = useQuery({
    queryKey: RECOVERY_HISTORY_QUERY_KEY,
    queryFn: recoveryHistory,
  });
  const snapshot = snapshotQuery.data ?? null;
  const history = historyQuery.data ?? [];

  const applyRecord = useCallback(
    (record: RecoveryRecord) => {
      queryClient.setQueryData<RecoverySnapshot | null>(RECOVERY_QUERY_KEY, (current) =>
        mergeRecord(current, record),
      );
    },
    [queryClient],
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    void subscribeRecoveryChanges((record) => {
      if (active) applyRecord(record);
    }).then((dispose) => {
      if (active) unlisten = dispose;
      else void dispose();
    });
    return () => {
      active = false;
      if (unlisten) void unlisten();
    };
  }, [applyRecord]);

  const openHealth = useCallback(
    (capabilityId?: string) => {
      const fallback = snapshot?.records.find((record) => ACTIONABLE_STATES.has(record.state))
        ?? snapshot?.records[0];
      setSelectedCapabilityId(capabilityId ?? fallback?.capability_id ?? null);
      setMutationError(null);
      setPanelOpen(true);
    },
    [snapshot],
  );

  const dismissPreview = useCallback(() => {
    if (mutationProgress === "Repairing") return;
    setPreviewState(null);
    setMutationError(null);
  }, [mutationProgress]);

  const closeHealth = useCallback(() => {
    setPanelOpen(false);
    setPreviewState(null);
    setMutationProgress(null);
    setMutationError(null);
  }, []);

  const check = useCallback(
    async (capabilityId: string) => {
      setMutationError(null);
      setMutationProgress("Checking");
      try {
        const record = await recoveryCheck(capabilityId);
        applyRecord(record);
        void queryClient.invalidateQueries({ queryKey: RECOVERY_HISTORY_QUERY_KEY });
        return record;
      } catch (error) {
        setMutationError(recoveryError(error));
        throw error;
      } finally {
        setMutationProgress(null);
      }
    },
    [applyRecord, queryClient],
  );

  const preview = useCallback(async (capabilityId: string, actionId: string) => {
    setMutationError(null);
    setMutationProgress("Preparing repair");
    try {
      const nextPreview = await recoveryRepairPreview(capabilityId, actionId);
      setPreviewState(nextPreview);
      return nextPreview;
    } catch (error) {
      setMutationError(recoveryError(error));
      throw error;
    } finally {
      setMutationProgress(null);
    }
  }, []);

  const confirm = useCallback(
    async (confirmationToken: string) => {
      setMutationError(null);
      setMutationProgress("Repairing");
      try {
        const record = await recoveryRepairConfirm(confirmationToken);
        applyRecord(record);
        setPreviewState(null);
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: RECOVERY_HISTORY_QUERY_KEY }),
          queryClient.invalidateQueries({ queryKey: RECOVERY_QUERY_KEY }),
        ]);
        return record;
      } catch (error) {
        setMutationError(recoveryError(error));
        throw error;
      } finally {
        setMutationProgress(null);
      }
    },
    [applyRecord, queryClient],
  );

  const cancel = useCallback(async (recipeId: string) => {
    setMutationError(null);
    setMutationProgress("Cancelling repair");
    try {
      const cancelled = await recoveryRepairCancel(recipeId);
      if (cancelled) {
        setPreviewState(null);
        void queryClient.invalidateQueries({ queryKey: RECOVERY_HISTORY_QUERY_KEY });
      }
      return cancelled;
    } catch (error) {
      setMutationError(recoveryError(error));
      throw error;
    } finally {
      setMutationProgress(null);
    }
  }, [queryClient]);

  const selected = useMemo(
    () =>
      snapshot?.records.find((record) => record.capability_id === selectedCapabilityId)
      ?? null,
    [selectedCapabilityId, snapshot],
  );

  const value = useMemo<RecoveryContextValue>(
    () => ({
      snapshot,
      history,
      selected,
      previewState,
      mutationProgress,
      mutationError,
      panelOpen,
      openHealth,
      closeHealth,
      dismissPreview,
      check,
      preview,
      confirm,
      cancel,
    }),
    [
      cancel,
      check,
      closeHealth,
      confirm,
      dismissPreview,
      history,
      mutationError,
      mutationProgress,
      openHealth,
      panelOpen,
      preview,
      previewState,
      selected,
      snapshot,
    ],
  );

  return <RecoveryContext.Provider value={value}>{children}</RecoveryContext.Provider>;
}

export function useRecovery(): RecoveryContextValue {
  const value = useContext(RecoveryContext);
  if (!value) throw new Error("useRecovery must be used within RecoveryProvider");
  return value;
}
