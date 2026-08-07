import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  WORK_ENGINEERING_SNAPSHOT_KEY,
  saveWorkItemContract,
  workEngineeringSnapshot,
  type ScopeComplianceStatus,
  type WorkEngineeringSnapshot,
  type WorkItemContractSnapshot,
  type WorkItemContractUpdate,
} from "../../shared/api/engineering";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { errorToMessage } from "../../shared/utils/helpers";

function toLines(values: string[]): string {
  return values.join("\n");
}

function fromLines(value: string): string[] {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}

function complianceLabel(status: ScopeComplianceStatus): string {
  switch (status) {
    case "compliant":
      return "Scope compliant";
    case "violation":
      return "Scope violation";
    case "unconfigured":
      return "Scope not governed";
    case "not_evaluated":
      return "Awaiting changeset";
  }
}

function complianceTone(status: ScopeComplianceStatus): string {
  switch (status) {
    case "compliant":
      return "ok";
    case "violation":
      return "danger";
    case "unconfigured":
      return "warn";
    case "not_evaluated":
      return "neutral";
  }
}

function ReadinessDot({ ready, label }: { ready: boolean; label: string }) {
  return (
    <span className={`work-contract-readiness${ready ? " ready" : ""}`}>
      <span aria-hidden="true" />
      {label}
    </span>
  );
}

function ContractSummary({ snapshot }: { snapshot: WorkItemContractSnapshot }) {
  const { contract, readiness, compliance } = snapshot;

  return (
    <>
      <div className="work-contract-topline">
        <div className="work-contract-title">
          <p className="eyebrow">Engineering Contract</p>
          <strong>{contract.goal || "Define what this Work Item is allowed to accomplish."}</strong>
        </div>
        <span className={`pill ${complianceTone(compliance.status)}`}>
          {complianceLabel(compliance.status)}
        </span>
      </div>

      <div className="work-contract-readiness-row" aria-label="Work Item contract readiness">
        <ReadinessDot ready={readiness.goal_defined} label="Goal" />
        <ReadinessDot ready={readiness.scope_defined} label="Scope" />
        <ReadinessDot ready={readiness.protected_paths_defined} label="Protected" />
        <ReadinessDot ready={readiness.acceptance_defined} label="Acceptance" />
      </div>

      <div className="work-contract-counts">
        <span><strong>{contract.allowed_paths.length}</strong> allowed paths</span>
        <span><strong>{contract.protected_paths.length}</strong> protected paths</span>
        <span><strong>{contract.acceptance_criteria.length}</strong> acceptance checks</span>
        {compliance.changed_files.length > 0 ? (
          <span><strong>{compliance.changed_files.length}</strong> observed changes</span>
        ) : null}
      </div>

      {compliance.status === "violation" ? (
        <div className="work-contract-violation" role="alert">
          <strong>Changes escaped the contract.</strong>
          {compliance.out_of_scope_files.length > 0 ? (
            <span>Outside scope: {compliance.out_of_scope_files.slice(0, 4).join(", ")}</span>
          ) : null}
          {compliance.protected_changed_files.length > 0 ? (
            <span>Protected: {compliance.protected_changed_files.slice(0, 4).join(", ")}</span>
          ) : null}
        </div>
      ) : null}
    </>
  );
}

export function WorkItemContractCard() {
  const { hasTask } = useWorkspace();
  const queryClient = useQueryClient();
  const snapshot = useQuery({
    queryKey: WORK_ENGINEERING_SNAPSHOT_KEY,
    queryFn: () => workEngineeringSnapshot(),
    enabled: hasTask,
    refetchInterval: 4000,
  });
  const [editing, setEditing] = useState(false);
  const [goal, setGoal] = useState("");
  const [allowedPaths, setAllowedPaths] = useState("");
  const [protectedPaths, setProtectedPaths] = useState("");
  const [acceptance, setAcceptance] = useState("");

  const contractSnapshot = snapshot.data?.work_item_contract ?? null;

  useEffect(() => {
    if (!contractSnapshot || editing) return;
    setGoal(contractSnapshot.contract.goal);
    setAllowedPaths(toLines(contractSnapshot.contract.allowed_paths));
    setProtectedPaths(toLines(contractSnapshot.contract.protected_paths));
    setAcceptance(toLines(contractSnapshot.contract.acceptance_criteria));
  }, [contractSnapshot, editing]);

  const save = useMutation({
    mutationFn: (update: WorkItemContractUpdate) => saveWorkItemContract(update),
    onSuccess: (saved) => {
      queryClient.setQueryData<WorkEngineeringSnapshot>(WORK_ENGINEERING_SNAPSHOT_KEY, (current) =>
        current ? { ...current, work_item_contract: saved } : current,
      );
      setEditing(false);
      void queryClient.invalidateQueries({ queryKey: ["work"] });
    },
  });

  if (!hasTask) return null;
  if (snapshot.isLoading || !contractSnapshot) {
    return (
      <div className="work-contract-shell">
        <section className="work-contract-card">
          <p className="eyebrow">Engineering Contract</p>
          <span className="muted">Loading bounded Work Item rules…</span>
        </section>
      </div>
    );
  }
  if (snapshot.isError) {
    return (
      <div className="work-contract-shell">
        <section className="work-contract-card">
          <p className="eyebrow">Engineering Contract</p>
          <span className="notice danger">{errorToMessage(snapshot.error)}</span>
        </section>
      </div>
    );
  }

  const submit = () => {
    save.mutate({
      goal: goal.trim(),
      allowed_paths: fromLines(allowedPaths),
      protected_paths: fromLines(protectedPaths),
      acceptance_criteria: fromLines(acceptance),
    });
  };

  return (
    <div className="work-contract-shell">
      <section className={`work-contract-card${editing ? " editing" : ""}`} aria-label="Work Item engineering contract">
        <ContractSummary snapshot={contractSnapshot} />

        {!editing ? (
          <div className="work-contract-actions">
            <button type="button" className="tiny-button" onClick={() => setEditing(true)}>
              {contractSnapshot.configured ? "Edit contract" : "Define contract"}
            </button>
            <span className="muted">
              Typed scope drives context selection; compliance is compared with the latest changeset.
            </span>
          </div>
        ) : (
          <div className="work-contract-editor">
            <label>
              <span>Goal</span>
              <textarea
                rows={2}
                value={goal}
                onChange={(event) => setGoal(event.target.value)}
                placeholder="What must be true when this Work Item is complete?"
              />
            </label>

            <div className="work-contract-editor-grid">
              <label>
                <span>Allowed paths</span>
                <textarea
                  rows={5}
                  value={allowedPaths}
                  onChange={(event) => setAllowedPaths(event.target.value)}
                  placeholder={"src/feature\ntests/feature"}
                  spellCheck={false}
                />
                <small>One project-relative file or directory per line. No globs in v0.</small>
              </label>
              <label>
                <span>Protected paths</span>
                <textarea
                  rows={5}
                  value={protectedPaths}
                  onChange={(event) => setProtectedPaths(event.target.value)}
                  placeholder={"src/security\ninfra/production"}
                  spellCheck={false}
                />
                <small>Protected rules override a broader allowed parent.</small>
              </label>
            </div>

            <label>
              <span>Acceptance criteria</span>
              <textarea
                rows={4}
                value={acceptance}
                onChange={(event) => setAcceptance(event.target.value)}
                placeholder={"cargo test -p repodesk-core passes\nNo changes outside the configured scope"}
              />
              <small>One concrete statement per line. Later verification receipts will bind evidence to these criteria.</small>
            </label>

            {save.isError ? <div className="notice danger">{errorToMessage(save.error)}</div> : null}

            <div className="work-contract-editor-actions">
              <button type="button" className="primary-button" disabled={save.isPending} onClick={submit}>
                {save.isPending ? "Saving…" : "Save contract"}
              </button>
              <button type="button" className="ghost-button" disabled={save.isPending} onClick={() => setEditing(false)}>
                Cancel
              </button>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
