# Verification Replay Inspector

## Goal

Make every verification receipt explainable against the current ChangeSet so RepoDesk can answer whether proof is still reusable, exactly why it is stale, and what action is required next.

## Product contract

Verification replay is a read-only comparison. It never reruns a command, guesses that a receipt is valid, or reports a stale receipt as green.

The comparison uses only deterministic facts already owned by RepoDesk:

- canonical verification receipt;
- receipt run and verification identity;
- reviewed ChangeSet digest;
- verification HEAD and index tree;
- current HEAD and index tree;
- current verification command evidence.

The result is one of `current`, `stale`, `missing`, or `unavailable`, with a stable reason code, human explanation, and recommended next action.

## Runtime shape

Add a `VerificationReplay` projection to `ChangeGovernanceSnapshot`:

- receipt identity and verified-at timestamp;
- verified and current tree identities;
- verified and current ChangeSet digests when available;
- command count and passed/failed counts;
- status and reason code;
- `can_rerun` and `recommended_action`.

The existing `work_engineering_intelligence` command remains the single IPC boundary. The existing `Verify reviewed ChangeSet` action remains the only command execution path.

## UI behavior

Changes shows a compact replay inspector beside the verification state. Current proof is positive only when exact tree and ChangeSet identity match. Stale, missing, and unavailable proof stay attention/critical and retain the reason. The existing verify action is reused for rerun.

## Verification

- Rust unit tests cover current, stale tree, stale ChangeSet, missing receipt, and failed command evidence.
- TypeScript build remains clean.
- Playwright covers current and stale replay states and the rerun action.
