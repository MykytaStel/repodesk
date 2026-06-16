# Security Policy

RepoDesk is a local-first desktop tool that handles source code and routes work to
AI models. We take its security posture seriously — the full threat model and
enforcement map live in [`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md).

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Use GitHub's private vulnerability reporting:
**Security → Advisories → "Report a vulnerability"** on this repository
(<https://github.com/MykytaStel/repodesk/security/advisories/new>).

If that is unavailable, contact the maintainer privately (see the repository owner's
profile) instead of filing a public issue.

Please include:
- affected version (`Help → About`, or the installer filename / git SHA),
- your OS and architecture,
- reproduction steps and impact,
- any logs — **with secrets and proprietary code redacted**.

We aim to acknowledge a report within a few business days and to keep you updated as
we triage and fix. We'll credit reporters who want it once a fix ships.

## Supported versions

RepoDesk is pre-1.x in spirit (rapid iteration). Security fixes target the latest
released version on `main`. Older builds receive fixes only via upgrading.

## Scope and design guarantees

These are enforced and tested (see `docs/SECURITY_MODEL.md`):
- The generated context pack contains only RepoDesk-managed files + git metadata —
  **never raw repository file bodies**.
- A safety/secret gate runs **before** any content reaches an AI model.
- Paid/cloud providers are **off by default**, require explicit enablement, and are
  blocked when context is unsafe or over budget.
- Check execution is restricted to an allowlist of binaries — no shell metacharacters.
- The desktop CSP allows no outbound hosts except the updater endpoint.

## Out of scope
- Vulnerabilities in third-party AI providers you choose to enable.
- Issues requiring a pre-compromised local machine or malicious OS account.
- Unsigned-build Gatekeeper/SmartScreen warnings (a packaging gap, tracked separately).
