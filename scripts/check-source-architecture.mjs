import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const HARD_SOURCE_LIMIT_BYTES = 28 * 1024;
const SOURCE_EXTENSIONS = new Set([".rs", ".ts", ".tsx", ".mjs"]);

const CHANGES_SEMANTIC_ADAPTER = "apps/desktop/src/features/changes/changesSemantic.ts";
const SHARED_PRIMITIVES_INDEX = "apps/desktop/src/shared/ui/primitives/index.ts";
const CHANGES_TYPED_SURFACES = [
  "apps/desktop/src/features/changes/ChangesTab.tsx",
  "apps/desktop/src/features/changes/ChangeGovernancePanel.tsx",
];

function extensionOf(path) {
  const index = path.lastIndexOf(".");
  return index >= 0 ? path.slice(index) : "";
}

function countMatches(text, pattern) {
  if (!text) return 0;
  return text.match(pattern)?.length ?? 0;
}

export function evaluateSourceChange({ path, baseSize, currentSize, hardLimit = HARD_SOURCE_LIMIT_BYTES }) {
  if (currentSize == null) return null;

  if (baseSize == null) {
    return currentSize > hardLimit
      ? `${path}: new source file is ${currentSize} bytes; limit is ${hardLimit} bytes`
      : null;
  }

  if (baseSize <= hardLimit && currentSize > hardLimit) {
    return `${path}: grew across the ${hardLimit}-byte source limit (${baseSize} -> ${currentSize})`;
  }

  if (baseSize > hardLimit && currentSize > baseSize) {
    return `${path}: existing god-file grew (${baseSize} -> ${currentSize}); files above ${hardLimit} bytes may only stay flat or shrink`;
  }

  return null;
}

export function evaluateVisualDebtChange({ path, baseText, currentText, baseSize, currentSize }) {
  if (currentSize == null && currentText == null) return [];
  const failures = [];
  const isNew = baseSize == null;

  if (isNew && /-v\d+\.(?:css|tsx?|mjs)$/i.test(path)) {
    failures.push(`${path}: new versioned visual generation is forbidden; extend the canonical design-system layer instead`);
  }
  if (isNew && /-polish\.css$/i.test(path)) {
    failures.push(`${path}: new route-wide polish stylesheet is forbidden; migrate into canonical or feature-owned styles`);
  }

  if (path.endsWith(".tsx") && currentText != null) {
    const rawHex = /#[0-9a-fA-F]{3,8}\b/g;
    const inlineStyle = /\bstyle\s*=\s*\{\s*\{/g;
    const statusTone = /\bstatusTone\s*\(/g;
    const checks = [
      ["raw hex", rawHex],
      ["static inline style", inlineStyle],
      ["statusTone", statusTone],
    ];

    for (const [label, pattern] of checks) {
      const before = countMatches(baseText, pattern);
      const after = countMatches(currentText, pattern);
      if (after > before) {
        failures.push(`${path}: ${label} debt grew (${before} -> ${after}); use semantic tokens/primitives and lower or preserve the baseline`);
      }
    }
  }

  if (path.startsWith("apps/desktop/src/features/") && path.endsWith(".css") && currentSize != null) {
    if (baseSize == null) {
      failures.push(`${path}: new feature-local CSS file is not allowed after the design-system freeze without a reviewed architecture change`);
    } else if (currentSize > baseSize) {
      failures.push(`${path}: feature CSS grew (${baseSize} -> ${currentSize}); migrated feature CSS may only stay flat or shrink`);
    }
  }

  return failures;
}

function git(args, options = {}) {
  return execFileSync("git", args, {
    cwd: process.cwd(),
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  }).trim();
}

function resolveBaseSha() {
  const requested = process.env.BASE_SHA?.trim();
  if (requested && !/^0+$/.test(requested)) return requested;
  return git(["rev-parse", "HEAD^"]);
}

function changedPaths(baseSha) {
  const output = execFileSync("git", ["diff", "--name-only", "-z", `${baseSha}...HEAD`, "--"], {
    cwd: process.cwd(),
  });

  return output.toString("utf8").split("\0").filter(Boolean);
}

function baseFileSize(baseSha, path) {
  try {
    const size = git(["cat-file", "-s", `${baseSha}:${path}`]);
    const parsed = Number.parseInt(size, 10);
    return Number.isFinite(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function baseFileText(baseSha, path) {
  try {
    return execFileSync("git", ["show", `${baseSha}:${path}`], {
      cwd: process.cwd(),
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch {
    return null;
  }
}

function readSource(path) {
  return existsSync(path) ? readFileSync(path, "utf8") : null;
}

export function evaluateChangesSemanticContract() {
  const failures = [];

  if (!existsSync(SHARED_PRIMITIVES_INDEX)) {
    failures.push(`${SHARED_PRIMITIVES_INDEX}: Changes reference migration requires the shared semantic primitive boundary`);
  }
  if (!existsSync(CHANGES_SEMANTIC_ADAPTER)) {
    failures.push(`${CHANGES_SEMANTIC_ADAPTER}: Changes reference migration requires one typed domain-to-semantic adapter`);
  }

  const adapter = readSource(CHANGES_SEMANTIC_ADAPTER);
  if (adapter) {
    if (/\.includes\(\s*["'`](?:ok|error|failed|warn|danger|block)/i.test(adapter)) {
      failures.push(`${CHANGES_SEMANTIC_ADAPTER}: typed Changes state must not be inferred from status text substrings`);
    }
    if (/statusTone\s*\(/.test(adapter)) {
      failures.push(`${CHANGES_SEMANTIC_ADAPTER}: typed Changes state must not delegate to statusTone()`);
    }
  }

  for (const path of CHANGES_TYPED_SURFACES) {
    const source = readSource(path);
    if (!source) {
      failures.push(`${path}: expected Changes reference surface is missing`);
      continue;
    }
    if (!source.includes('from "../../shared/ui/primitives"')) {
      failures.push(`${path}: migrated Changes surfaces must consume the shared semantic primitive boundary`);
    }
    if (!source.includes('from "./changesSemantic"')) {
      failures.push(`${path}: migrated Changes surfaces must consume the typed Changes semantic adapter`);
    }
    if (/statusTone\s*\(/.test(source)) {
      failures.push(`${path}: typed Changes state must not call statusTone()`);
    }
    if (/\.includes\(\s*["'`](?:ok|error|failed|warn|danger|block)/i.test(source)) {
      failures.push(`${path}: typed Changes state must not be inferred from status text substrings`);
    }
  }

  return failures;
}

export function runArchitectureRatchet() {
  const baseSha = resolveBaseSha();
  const paths = changedPaths(baseSha);
  const failures = [];

  for (const path of paths) {
    const currentSize = existsSync(path) ? statSync(path).size : null;
    const baseSize = baseFileSize(baseSha, path);

    if (SOURCE_EXTENSIONS.has(extensionOf(path))) {
      const failure = evaluateSourceChange({ path, baseSize, currentSize });
      if (failure) failures.push(failure);
    }

    if (path.endsWith(".tsx") || path.endsWith(".css")) {
      failures.push(...evaluateVisualDebtChange({
        path,
        baseText: path.endsWith(".tsx") ? baseFileText(baseSha, path) : null,
        currentText: path.endsWith(".tsx") ? readSource(path) : null,
        baseSize,
        currentSize,
      }));
    }
  }

  failures.push(...evaluateChangesSemanticContract());

  console.log(`Architecture ratchet: ${paths.length} changed file(s), base ${baseSha.slice(0, 12)}.`);
  console.log(`Hard limit for new/crossing source files: ${HARD_SOURCE_LIMIT_BYTES} bytes (28 KiB).`);
  console.log("Design-system debt is frozen: no new visual generations, TSX raw/style/statusTone growth, or feature CSS growth.");

  if (failures.length > 0) {
    console.error("\nSource architecture budget failed:");
    for (const failure of failures) console.error(`- ${failure}`);
    console.error("\nReduce the touched debt or use the canonical design-system boundary; do not raise a baseline to make the gate green.");
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : null;
if (invokedPath && fileURLToPath(import.meta.url) === invokedPath) runArchitectureRatchet();