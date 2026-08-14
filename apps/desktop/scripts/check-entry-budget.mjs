import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import {
  activatedFeatureEagerFiles,
  excludeFiles,
  findChunkBySource,
  measureGraph,
  namedGraphFiles,
  overlappingFiles,
} from "./performance-budget.mjs";

const distPath = fileURLToPath(new URL("../dist/", import.meta.url));
const manifest = JSON.parse(readFileSync(join(distPath, ".vite/manifest.json"), "utf8"));

const ROUTES = {
  work: "src/features/work/WorkSurface.tsx",
  code: "src/features/code/CodeTab.tsx",
  changes: "src/features/changes/ChangesTab.tsx",
  history: "src/features/history/HistoryTab.tsx",
  projects: "src/features/projects/ProjectsTab.tsx",
  dashboard: "src/features/dashboard/DashboardTab.tsx",
  tokens: "src/features/tokens/TokensTab.tsx",
  models: "src/features/models/ModelsTab.tsx",
  git: "src/features/git/GitTab.tsx",
  memory: "src/features/knowledge/KnowledgeTab.tsx",
  orchestrate: "src/features/orchestrate/OrchestrateTab.tsx",
  outcomes: "src/features/outcomes/OutcomesTab.tsx",
  playbooks: "src/features/playbooks/PlaybooksTab.tsx",
  "models-cost": "src/features/models-cost/ModelsCostTab.tsx",
  audit: "src/features/audit/AuditTab.tsx",
  settings: "src/features/settings/SettingsTab.tsx",
  system: "src/features/system/SystemTab.tsx",
  debug: "src/features/debug/DebugTab.tsx",
};

const ACTIVATED_FEATURES = {
  bottomPanel: "src/app/WorkbenchBottomPanel.tsx",
  terminal: "src/app/InteractiveTerminal.tsx",
  commandPalette: "src/shared/ui/CommandPalette.tsx",
  ideHealth: "src/features/health/IDEHealthPanel.tsx",
};

const SHELL_BUDGET = { jsGzip: 95_000, cssGzip: 18_000 };
const ROUTE_BUDGET = { jsGzip: 35_000, cssGzip: 12_000 };
const CODE_JS_BUDGET = 45_000;
// Vite links shared first-party hooks/providers back to the shell entry, so its
// direct JS/CSS are expected in activated graphs. React, React Query and Tauri
// are named platform chunks already required by the shell. Only these direct
// assets are allowed; their dependencies are not transitively allowlisted.
const ACTIVATION_GRAPH_ALLOWLIST = {
  shellRootKey: "index.html",
  allowedShellRootKeys: ["index.html"],
  allowedSharedChunkNames: ["vendor-react", "vendor-query", "vendor-tauri"],
};

function formatBytes(bytes) {
  return `${(bytes / 1_000).toFixed(1)} kB`;
}

function assertAtMost(label, actual, budget) {
  if (actual > budget) {
    throw new Error(`${label} exceeded: ${actual} > ${budget} bytes`);
  }
}

function measureFiles(files) {
  const size = (file) => gzipSync(readFileSync(join(distPath, file))).byteLength;
  return {
    jsGzip: files.filter((file) => file.endsWith(".js")).reduce((sum, file) => sum + size(file), 0),
    cssGzip: files.filter((file) => file.endsWith(".css")).reduce((sum, file) => sum + size(file), 0),
  };
}

function report(label, measurement) {
  console.log(`${label}: ${formatBytes(measurement.jsGzip)} JavaScript, ${formatBytes(measurement.cssGzip)} CSS`);
}

function assertAbsentFromShell(label, files, shellFiles) {
  const eagerFiles = overlappingFiles(files, shellFiles);
  if (eagerFiles.length > 0) {
    throw new Error(`${label} is eagerly loaded: ${eagerFiles.join(", ")}`);
  }
}

const shell = measureGraph({ manifest, rootKey: "index.html", distPath });
report("Shell", shell);
assertAtMost("Shell JavaScript gzip budget", shell.jsGzip, SHELL_BUDGET.jsGzip);
assertAtMost("Shell CSS gzip budget", shell.cssGzip, SHELL_BUDGET.cssGzip);

const shellFiles = new Set(shell.files);
const terminalVendorFiles = namedGraphFiles(manifest, "vendor-terminal");
const editorVendorFiles = namedGraphFiles(manifest, "vendor-editor-core");
assertAbsentFromShell("Terminal vendor graph", terminalVendorFiles, shellFiles);
assertAbsentFromShell("Editor vendor graph", editorVendorFiles, shellFiles);
report("Editor vendor graph", measureFiles(editorVendorFiles));

for (const [route, source] of Object.entries(ROUTES)) {
  const rootKey = findChunkBySource(manifest, source);
  const routeGraph = measureGraph({ manifest, rootKey, distPath });
  const incrementFiles = routeGraph.files.filter((file) => !shellFiles.has(file));
  const budgetFiles = route === "code" ? excludeFiles(incrementFiles, editorVendorFiles) : incrementFiles;
  const increment = measureFiles(budgetFiles);
  report(`Route ${route} increment`, increment);

  assertAtMost(`Route ${route} CSS gzip budget`, increment.cssGzip, ROUTE_BUDGET.cssGzip);
  assertAtMost(
    `Route ${route} JavaScript gzip budget`,
    increment.jsGzip,
    route === "code" ? CODE_JS_BUDGET : ROUTE_BUDGET.jsGzip,
  );
}

for (const [feature, source] of Object.entries(ACTIVATED_FEATURES)) {
  const rootKey = findChunkBySource(manifest, source);
  const featureGraph = measureGraph({ manifest, rootKey, distPath });
  const eagerFiles = activatedFeatureEagerFiles(
    manifest,
    rootKey,
    ACTIVATION_GRAPH_ALLOWLIST,
  );
  if (eagerFiles.length > 0) {
    throw new Error(`Activated feature ${feature} is eagerly loaded: ${eagerFiles.join(", ")}`);
  }
  report(`Activated feature ${feature}`, featureGraph);
}
