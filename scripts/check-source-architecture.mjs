import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const HARD_SOURCE_LIMIT_BYTES = 28 * 1024;
const SOURCE_EXTENSIONS = new Set([".rs", ".ts", ".tsx", ".mjs"]);
const LEGACY_POLISH_STYLESHEET = /-polish\.css$/i;

const SHARED_PRIMITIVES_INDEX = "apps/desktop/src/shared/ui/primitives/index.ts";
const CHANGES_SEMANTIC_ADAPTER = "apps/desktop/src/features/changes/changesSemantic.ts";
const CHANGES_TYPED_SURFACES = [
  "apps/desktop/src/features/changes/ChangesTab.tsx",
  "apps/desktop/src/features/changes/ChangeGovernancePanel.tsx",
];
const WORK_SEMANTIC_ADAPTER = "apps/desktop/src/features/work/workSemantic.ts";
const WORK_TYPED_SURFACES = [
  "apps/desktop/src/features/work/WorkSurface.tsx",
  "apps/desktop/src/features/work/WorkTab.tsx",
  "apps/desktop/src/features/work/ReviewPanel.tsx",
];
const WORK_PRIMITIVE_SURFACES = [
  ...WORK_TYPED_SURFACES,
  "apps/desktop/src/features/work/ExecutionStrategyControls.tsx",
];
const WORK_OBSOLETE_VISUAL_PATHS = [
  "apps/desktop/src/features/work/work-focus-polish.css",
  "apps/desktop/src/app/styles/work-hierarchy-v3.css",
];
const WORK_CANONICAL_HIERARCHY = "apps/desktop/src/app/styles/work-hierarchy.css";
const WORK_ROUTE_STYLES = "apps/desktop/src/features/work/work-route.css";
const RUNS_SEMANTIC_ADAPTER = "apps/desktop/src/features/history/runsSemantic.ts";
const RUNS_TYPED_SURFACES = ["apps/desktop/src/features/history/RunsWorkspace.tsx"];
const RUNS_PRIMITIVE_SURFACES = [
  "apps/desktop/src/features/history/HistoryTab.tsx",
  "apps/desktop/src/features/history/RunsWorkspace.tsx",
];
const PROJECTS_SEMANTIC_ADAPTER = "apps/desktop/src/features/projects/projectsSemantic.ts";
const PROJECTS_TYPED_SURFACES = ["apps/desktop/src/features/projects/ProjectsTab.tsx"];
const CODE_SEMANTIC_ADAPTER = "apps/desktop/src/features/code/codeSemantic.ts";
const CODE_TYPED_SURFACES = [
  "apps/desktop/src/features/code/CodeTab.tsx",
  "apps/desktop/src/features/code/CodeWorkspaceTree.tsx",
  "apps/desktop/src/features/code/CodeSemanticStrip.tsx",
  "apps/desktop/src/features/code/RepositoryIntelligenceDrawer.tsx",
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

export function evaluateLegacyPolishDebtCleanupContract(paths = null) {
  const candidates = paths ?? git(["ls-files", "apps/desktop/src"]).split("\n").filter(Boolean);
  return candidates
    .filter((path) => LEGACY_POLISH_STYLESHEET.test(path))
    .map((path) => `${path}: legacy polish stylesheet must be retired; use the canonical design-system layer`);
}

function evaluateTypedSemanticContract({
  label,
  adapterPath,
  adapterImport,
  typedSurfaces,
  primitiveSurfaces = typedSurfaces,
}) {
  const failures = [];

  if (!existsSync(SHARED_PRIMITIVES_INDEX)) {
    failures.push(`${SHARED_PRIMITIVES_INDEX}: ${label} migration requires the shared semantic primitive boundary`);
  }
  if (!existsSync(adapterPath)) {
    failures.push(`${adapterPath}: ${label} migration requires one typed domain-to-semantic adapter`);
  }

  const statusSubstringInference = /\.includes\(\s*["'`](?:ok|error|failed|warn|danger|block|ready|done|stale|complete|accepted|passed|proven)/i;
  const adapter = readSource(adapterPath);
  if (adapter) {
    if (statusSubstringInference.test(adapter)) {
      failures.push(`${adapterPath}: typed ${label} state must not be inferred from status text substrings`);
    }
    if (/statusTone\s*\(/.test(adapter)) {
      failures.push(`${adapterPath}: typed ${label} state must not delegate to statusTone()`);
    }
  }

  for (const path of primitiveSurfaces) {
    const source = readSource(path);
    if (!source) {
      failures.push(`${path}: expected ${label} semantic surface is missing`);
      continue;
    }
    if (!source.includes('from "../../shared/ui/primitives"')) {
      failures.push(`${path}: migrated ${label} surfaces must consume the shared semantic primitive boundary`);
    }
    if (/statusTone\s*\(/.test(source)) {
      failures.push(`${path}: typed ${label} state must not call statusTone()`);
    }
    if (statusSubstringInference.test(source)) {
      failures.push(`${path}: typed ${label} state must not be inferred from status text substrings`);
    }
  }

  for (const path of typedSurfaces) {
    const source = readSource(path);
    if (source && !source.includes(`from "${adapterImport}"`)) {
      failures.push(`${path}: migrated ${label} typed surfaces must consume ${adapterImport}`);
    }
  }

  return failures;
}

export function evaluateChangesSemanticContract() {
  return evaluateTypedSemanticContract({
    label: "Changes",
    adapterPath: CHANGES_SEMANTIC_ADAPTER,
    adapterImport: "./changesSemantic",
    typedSurfaces: CHANGES_TYPED_SURFACES,
  });
}

export function evaluateWorkSemanticContract() {
  return evaluateTypedSemanticContract({
    label: "Work",
    adapterPath: WORK_SEMANTIC_ADAPTER,
    adapterImport: "./workSemantic",
    typedSurfaces: WORK_TYPED_SURFACES,
    primitiveSurfaces: WORK_PRIMITIVE_SURFACES,
  });
}

export function evaluateRunsSemanticContract() {
  return evaluateTypedSemanticContract({
    label: "Runs",
    adapterPath: RUNS_SEMANTIC_ADAPTER,
    adapterImport: "./runsSemantic",
    typedSurfaces: RUNS_TYPED_SURFACES,
    primitiveSurfaces: RUNS_PRIMITIVE_SURFACES,
  });
}

export function evaluateProjectsSemanticContract() {
  return evaluateTypedSemanticContract({
    label: "Projects",
    adapterPath: PROJECTS_SEMANTIC_ADAPTER,
    adapterImport: "./projectsSemantic",
    typedSurfaces: PROJECTS_TYPED_SURFACES,
  });
}

export function evaluateCodeSemanticContract() {
  const failures = evaluateTypedSemanticContract({
    label: "Code",
    adapterPath: CODE_SEMANTIC_ADAPTER,
    adapterImport: "./codeSemantic",
    typedSurfaces: CODE_TYPED_SURFACES,
  });

  const codeTab = readSource("apps/desktop/src/features/code/CodeTab.tsx");
  if (codeTab && /code-workspace-v\d+/i.test(codeTab)) {
    failures.push("apps/desktop/src/features/code/CodeTab.tsx: canonical Code shell must not use a versioned class name");
  }

  const workspaceTree = readSource("apps/desktop/src/features/code/CodeWorkspaceTree.tsx");
  if (workspaceTree && /\bstatusTone\s*\(/.test(workspaceTree)) {
    failures.push("apps/desktop/src/features/code/CodeWorkspaceTree.tsx: typed file state must use codeSemantic.ts instead of statusTone()");
  }

  return failures;
}

export function evaluateWorkVisualDebtCleanupContract() {
  const failures = [];

  for (const path of WORK_OBSOLETE_VISUAL_PATHS) {
    if (existsSync(path)) {
      failures.push(`${path}: obsolete Work visual generation must be retired after semantic convergence`);
    }
  }

  if (!existsSync(WORK_CANONICAL_HIERARCHY)) {
    failures.push(`${WORK_CANONICAL_HIERARCHY}: Work must own one canonical non-versioned hierarchy stylesheet`);
  }

  const workSurface = readSource("apps/desktop/src/features/work/WorkSurface.tsx");
  if (workSurface && /work-workbench-v\d+/i.test(workSurface)) {
    failures.push("apps/desktop/src/features/work/WorkSurface.tsx: canonical Work shell must not use a versioned class name");
  }

  const routeStyles = readSource(WORK_ROUTE_STYLES);
  if (routeStyles) {
    if (/work-focus-polish\.css/.test(routeStyles)) {
      failures.push(`${WORK_ROUTE_STYLES}: obsolete polish stylesheet must not be imported`);
    }
    if (/work-hierarchy-v\d+\.css/.test(routeStyles)) {
      failures.push(`${WORK_ROUTE_STYLES}: canonical hierarchy import must not use a version suffix`);
    }
    if (!/work-hierarchy\.css/.test(routeStyles)) {
      failures.push(`${WORK_ROUTE_STYLES}: canonical Work hierarchy stylesheet must be imported`);
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

  failures.push(...evaluateLegacyPolishDebtCleanupContract());
  failures.push(...evaluateChangesSemanticContract());
  failures.push(...evaluateWorkSemanticContract());
  failures.push(...evaluateRunsSemanticContract());
  failures.push(...evaluateProjectsSemanticContract());
  failures.push(...evaluateCodeSemanticContract());
  failures.push(...evaluateWorkVisualDebtCleanupContract());

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