import type { DecisionReceipt } from "../../shared/api/verification";

type DecisionReceiptDrawerProps = {
  receipt: DecisionReceipt | null;
  open: boolean;
  onOpen: () => void;
  onClose: () => void;
};

function value(value: string | number | null | undefined): string {
  return value == null || value === "" ? "Unknown" : String(value);
}

export function DecisionReceiptDrawer({ receipt, open, onOpen, onClose }: DecisionReceiptDrawerProps) {
  if (!receipt) {
    return (
      <section className="work-control-card decision-receipt-card is-unknown" aria-label="Decision Receipt">
        <div className="work-control-card-heading">
          <div><span className="eyebrow">Decision Receipt</span><h2>Not recorded yet</h2></div>
          <span className="work-control-state is-unknown">Unknown</span>
        </div>
        <p className="work-control-note">Accepting or deferring a recommendation will create a receipt with the exact tree, policy, and evidence state.</p>
      </section>
    );
  }

  return (
    <>
      <section className="work-control-card decision-receipt-card" aria-label="Decision Receipt">
        <div className="work-control-card-heading">
          <div><span className="eyebrow">Decision Receipt</span><h2>{receipt.decision_kind}</h2></div>
          <span className="work-control-state is-measured">Recorded</span>
        </div>
        <div className="work-control-receipt-summary">
          <span>Tree</span><strong>{value(receipt.tree_identity)}</strong>
          <span>Policy</span><strong>{value(receipt.policy_version)}</strong>
        </div>
        <button type="button" className="secondary-cta" onClick={onOpen}>View decision receipt</button>
      </section>

      {open ? (
        <div className="work-receipt-backdrop" role="presentation" onMouseDown={onClose}>
          <section
            className="work-receipt-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="Decision Receipt"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="work-control-card-heading">
              <div><span className="eyebrow">Decision Receipt</span><h2>{receipt.decision_id}</h2></div>
              <button type="button" className="icon-button" title="Close decision receipt" aria-label="Close decision receipt" onClick={onClose}>×</button>
            </div>
            <dl className="work-receipt-list">
              <div><dt>Decision</dt><dd>{receipt.decision_kind}</dd></div>
              <div><dt>Tree identity</dt><dd>{value(receipt.tree_identity)}</dd></div>
              <div><dt>Policy version</dt><dd>{value(receipt.policy_version)}</dd></div>
              <div><dt>Risk</dt><dd>{value(receipt.risk_label)}</dd></div>
              <div><dt>Uncertainty</dt><dd>{value(receipt.uncertainty_label)}</dd></div>
              <div><dt>Created</dt><dd>{value(receipt.created_at)}</dd></div>
            </dl>
            <div className="work-receipt-section">
              <span className="eyebrow">Selected actions</span>
              {receipt.selected_actions.length > 0 ? <ul>{receipt.selected_actions.map((item) => <li key={item}>{item}</li>)}</ul> : <p>None recorded.</p>}
            </div>
            <div className="work-receipt-section">
              <span className="eyebrow">Verification debt</span>
              {receipt.verification_debt.length > 0 ? (
                <ul>{receipt.verification_debt.map((debt) => <li key={debt.check_id}><strong>{debt.title}</strong> — {debt.reason}</li>)}</ul>
              ) : <p>None recorded.</p>}
            </div>
            <div className="work-receipt-section">
              <span className="eyebrow">Observed facts</span>
              <pre>{JSON.stringify(receipt.observed_facts, null, 2)}</pre>
            </div>
          </section>
        </div>
      ) : null}
    </>
  );
}
