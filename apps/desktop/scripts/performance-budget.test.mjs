import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import {
  activatedFeatureEagerFiles,
  collectStaticGraph,
  excludeFiles,
  findChunkBySource,
  graphFiles,
  namedGraphFiles,
  overlappingFiles,
} from "./performance-budget.mjs";

const manifest = {
  "index.html": {
    file: "assets/index.js",
    isEntry: true,
    imports: ["_react.js"],
    dynamicImports: ["src/features/work/WorkSurface.tsx", "src/features/code/CodeTab.tsx"],
    css: ["assets/shell.css"],
  },
  "_react.js": { file: "assets/react.js", css: ["assets/shared.css"] },
  "src/features/work/WorkSurface.tsx": {
    src: "src/features/work/WorkSurface.tsx",
    file: "assets/work.js",
    imports: ["_react.js"],
    css: ["assets/work.css", "assets/shared.css"],
  },
  "src/features/code/CodeTab.tsx": {
    src: "src/features/code/CodeTab.tsx",
    file: "assets/code.js",
    imports: ["_react.js"],
    css: ["assets/code.css"],
  },
};

test("static graph excludes dynamic routes", () => {
  assert.deepEqual([...collectStaticGraph(manifest, "index.html")].sort(), ["_react.js", "index.html"]);
});

test("graph files deduplicate shared CSS", () => {
  assert.deepEqual(graphFiles(manifest, new Set(["_react.js", "src/features/work/WorkSurface.tsx"])).sort(), [
    "assets/react.js",
    "assets/shared.css",
    "assets/work.css",
    "assets/work.js",
  ]);
});

const isolationManifest = {
  "index.html": {
    file: "assets/index.js",
    imports: ["_react.js", "_terminal.js", "_editor.js"],
  },
  "_react.js": { file: "assets/react.js" },
  "_terminal.js": {
    file: "assets/vendor-terminal.js",
    name: "vendor-terminal",
    css: ["assets/vendor-terminal.css"],
  },
  "_editor.js": { file: "assets/vendor-editor-core.js", name: "vendor-editor-core" },
  "src/features/code/CodeTab.tsx": {
    file: "assets/code.js",
    imports: ["_react.js", "_editor.js"],
    css: ["assets/code.css"],
  },
};

test("Code budget excludes the separately measured editor vendor graph", () => {
  const codeFiles = graphFiles(
    isolationManifest,
    collectStaticGraph(isolationManifest, "src/features/code/CodeTab.tsx"),
  );
  const editorFiles = namedGraphFiles(isolationManifest, "vendor-editor-core");

  assert.deepEqual(excludeFiles(codeFiles, editorFiles).sort(), [
    "assets/code.css",
    "assets/code.js",
    "assets/react.js",
  ]);
});

test("terminal vendor JS and CSS are detected when eager in the shell", () => {
  const shellFiles = graphFiles(isolationManifest, collectStaticGraph(isolationManifest, "index.html"));
  const terminalFiles = namedGraphFiles(isolationManifest, "vendor-terminal");

  assert.deepEqual(overlappingFiles(shellFiles, terminalFiles).sort(), [
    "assets/vendor-terminal.css",
    "assets/vendor-terminal.js",
  ]);
});

test("editor core graph is detected when eager in the shell", () => {
  const shellFiles = graphFiles(isolationManifest, collectStaticGraph(isolationManifest, "index.html"));
  const editorFiles = namedGraphFiles(isolationManifest, "vendor-editor-core");

  assert.deepEqual(overlappingFiles(shellFiles, editorFiles), ["assets/vendor-editor-core.js"]);
});

test("activation graph audit rejects an eager child while allowing only explicit shell platform assets", () => {
  const activationManifest = {
    "index.html": {
      file: "assets/shell.js",
      isEntry: true,
      imports: ["_react.js"],
      dynamicImports: ["src/OptionalFeature.tsx"],
      css: ["assets/shell.css"],
    },
    "_react.js": {
      file: "assets/vendor-react.js",
      name: "vendor-react",
      imports: ["_optional-child.js"],
    },
    "_optional-child.js": {
      file: "assets/optional-child.js",
      css: ["assets/optional-child.css"],
    },
    "src/OptionalFeature.tsx": {
      file: "assets/optional-feature.js",
      src: "src/OptionalFeature.tsx",
      imports: ["index.html"],
      css: ["assets/optional-feature.css"],
    },
  };
  assert.deepEqual(activatedFeatureEagerFiles(activationManifest, "src/OptionalFeature.tsx", {
    shellRootKey: "index.html",
    allowedShellRootKeys: ["index.html"],
    allowedSharedChunkNames: ["vendor-react"],
  }), ["assets/optional-child.js", "assets/optional-child.css"]);
});

test("primary route styles are emitted outside the eager shell graph", async () => {
  const desktopRoot = fileURLToPath(new URL("../", import.meta.url));
  const outDir = mkdtempSync(join(tmpdir(), "repodesk-primary-route-css-"));

  try {
    await build({
      root: desktopRoot,
      logLevel: "silent",
      build: { manifest: true, outDir, emptyOutDir: true },
    });

    const emittedManifest = JSON.parse(readFileSync(join(outDir, ".vite/manifest.json"), "utf8"));
    const shellCss = graphFiles(
      emittedManifest,
      collectStaticGraph(emittedManifest, "index.html"),
    ).filter((file) => file.endsWith(".css"));

    for (const source of [
      "src/features/work/WorkSurface.tsx",
      "src/features/changes/ChangesTab.tsx",
      "src/features/history/HistoryTab.tsx",
      "src/features/projects/ProjectsTab.tsx",
    ]) {
      const routeKey = findChunkBySource(emittedManifest, source);
      const routeCss = emittedManifest[routeKey].css ?? [];

      assert.ok(routeCss.length > 0, `${source} must emit CSS with its lazy route chunk`);
      assert.deepEqual(
        overlappingFiles(routeCss, shellCss),
        [],
        `${source} CSS must not be eagerly loaded by the shell`,
      );
    }
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
});

test("secondary and mixed route styles are emitted only with their owning lazy routes", async () => {
  const desktopRoot = fileURLToPath(new URL("../", import.meta.url));
  const outDir = mkdtempSync(join(tmpdir(), "repodesk-secondary-route-css-"));

  try {
    await build({
      root: desktopRoot,
      logLevel: "silent",
      build: { manifest: true, outDir, emptyOutDir: true },
    });

    const emittedManifest = JSON.parse(readFileSync(join(outDir, ".vite/manifest.json"), "utf8"));
    const cssTextForGraph = (rootKey) =>
      graphFiles(emittedManifest, collectStaticGraph(emittedManifest, rootKey))
        .filter((file) => file.endsWith(".css"))
        .map((file) => readFileSync(join(outDir, file), "utf8"))
        .join("\n");

    const shellCss = cssTextForGraph("index.html");
    const routeContracts = [
      ["src/features/work/WorkSurface.tsx", [".work-focus-layout", ".task-switcher"]],
      ["src/features/changes/ChangesTab.tsx", [".changes-focus-layout", ".findings-list"]],
      ["src/features/history/HistoryTab.tsx", [".run-evidence-detail"]],
      ["src/features/dashboard/DashboardTab.tsx", [".dashboard-grid", ".route-panel"]],
      ["src/features/knowledge/KnowledgeTab.tsx", [".knowledge-workspace", ".knowledge-page-header"]],
      ["src/features/orchestrate/OrchestrateTab.tsx", [".orchestrate-control-panel", ".task-row-main"]],
      ["src/features/debug/DebugTab.tsx", [".debug-event", ".debug-runtime-row"]],
      ["src/features/code/CodeTab.tsx", [".health-trend"]],
      ["src/features/models/ModelsTab.tsx", [".provider-panel"]],
      ["src/features/git/GitTab.tsx", [".file-group-stack"]],
      ["src/features/system/SystemTab.tsx", [".route-list"]],
      ["src/features/audit/AuditTab.tsx", [".route-summary-grid"]],
    ];

    for (const [source, selectors] of routeContracts) {
      const routeKey = findChunkBySource(emittedManifest, source);
      const routeCss = cssTextForGraph(routeKey);

      for (const selector of selectors) {
        assert.ok(routeCss.includes(selector), `${selector} must load with ${source}`);
        assert.ok(!shellCss.includes(selector), `${selector} must not be eagerly loaded by the shell`);
      }
    }

    const forbiddenShellSelectors = [
      ...shellCss.matchAll(/\.(?:work|changes|runs|orchestrate|knowledge|route|debug)-[\w-]+/g),
    ]
      .map(([selector]) => selector)
      .filter((selector) => selector !== ".debug-list");
    assert.deepEqual(
      [...new Set(forbiddenShellSelectors)].sort(),
      [],
      "route-exclusive selector prefixes must not be emitted in the eager shell CSS",
    );
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
});

test("shared secondary-route primitives follow every consumer without leaking to unrelated routes", async () => {
  const desktopRoot = fileURLToPath(new URL("../", import.meta.url));
  const outDir = mkdtempSync(join(tmpdir(), "repodesk-shared-secondary-css-"));

  try {
    await build({
      root: desktopRoot,
      logLevel: "silent",
      build: { manifest: true, outDir, emptyOutDir: true },
    });

    const emittedManifest = JSON.parse(readFileSync(join(outDir, ".vite/manifest.json"), "utf8"));
    const routeCssFiles = (source) => {
      const routeKey = findChunkBySource(emittedManifest, source);
      return graphFiles(emittedManifest, collectStaticGraph(emittedManifest, routeKey)).filter((file) =>
        file.endsWith(".css"),
      );
    };
    const containsSelector = (files, selector) =>
      files.some((file) => readFileSync(join(outDir, file), "utf8").includes(selector));
    const emittedCssFiles = [
      ...new Set(Object.values(emittedManifest).flatMap((chunk) => chunk.css ?? [])),
    ];
    const cssFilesContaining = (selector) =>
      emittedCssFiles.filter((file) => readFileSync(join(outDir, file), "utf8").includes(selector));

    const shellCss = routeCssFiles("index.html");
    const routeSources = [
      "src/features/work/WorkSurface.tsx",
      "src/features/code/CodeTab.tsx",
      "src/features/changes/ChangesTab.tsx",
      "src/features/history/HistoryTab.tsx",
      "src/features/projects/ProjectsTab.tsx",
      "src/features/dashboard/DashboardTab.tsx",
      "src/features/tokens/TokensTab.tsx",
      "src/features/models/ModelsTab.tsx",
      "src/features/git/GitTab.tsx",
      "src/features/knowledge/KnowledgeTab.tsx",
      "src/features/orchestrate/OrchestrateTab.tsx",
      "src/features/outcomes/OutcomesTab.tsx",
      "src/features/playbooks/PlaybooksTab.tsx",
      "src/features/models-cost/ModelsCostTab.tsx",
      "src/features/audit/AuditTab.tsx",
      "src/features/settings/SettingsTab.tsx",
      "src/features/system/SystemTab.tsx",
      "src/features/debug/DebugTab.tsx",
    ];
    const routeCssBySource = new Map(routeSources.map((source) => [source, routeCssFiles(source)]));
    const subnavOwners = new Set([
      "src/features/history/HistoryTab.tsx",
      "src/features/models-cost/ModelsCostTab.tsx",
    ]);
    const manualImportOwners = new Set([
      "src/features/work/WorkSurface.tsx",
      "src/features/playbooks/PlaybooksTab.tsx",
    ]);

    for (const selector of [".subnav-host .subnav", ".subnav-host .subnav button.selected"]) {
      for (const [source, cssFiles] of routeCssBySource) {
        assert.equal(
          containsSelector(cssFiles, selector),
          subnavOwners.has(source),
          `${selector} ownership mismatch for ${source}`,
        );
      }
      assert.ok(!containsSelector(shellCss, selector), `${selector} must remain lazy`);
      assert.equal(cssFilesContaining(selector).length, 1, `${selector} must be emitted once`);
    }

    for (const [source, cssFiles] of routeCssBySource) {
      assert.equal(
        containsSelector(cssFiles, ".manual-import-input"),
        manualImportOwners.has(source),
        `.manual-import-input ownership mismatch for ${source}`,
      );
    }
    assert.ok(!containsSelector(shellCss, ".manual-import-input"), ".manual-import-input must remain lazy");
    assert.equal(cssFilesContaining(".manual-import-input").length, 1, ".manual-import-input must be emitted once");
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
});

test("routing feature CSS is one shared lazy asset owned by every actual consumer", async () => {
  const desktopRoot = fileURLToPath(new URL("../", import.meta.url));
  const outDir = mkdtempSync(join(tmpdir(), "repodesk-shared-routing-css-"));

  try {
    await build({
      root: desktopRoot,
      logLevel: "silent",
      build: { manifest: true, outDir, emptyOutDir: true },
    });

    const emittedManifest = JSON.parse(readFileSync(join(outDir, ".vite/manifest.json"), "utf8"));
    const routeCssFiles = (source) => {
      const routeKey = findChunkBySource(emittedManifest, source);
      return graphFiles(emittedManifest, collectStaticGraph(emittedManifest, routeKey)).filter((file) =>
        file.endsWith(".css"),
      );
    };
    const emittedCssFiles = [
      ...new Set(Object.values(emittedManifest).flatMap((chunk) => chunk.css ?? [])),
    ];
    const routingAssets = emittedCssFiles.filter((file) =>
      readFileSync(join(outDir, file), "utf8").includes(".route-panel"),
    );

    assert.equal(routingAssets.length, 1, `routing primitives must be emitted once, found: ${routingAssets.join(", ")}`);
    const [routingAsset] = routingAssets;

    for (const source of [
      "src/features/work/WorkSurface.tsx",
      "src/features/changes/ChangesTab.tsx",
      "src/features/dashboard/DashboardTab.tsx",
      "src/features/debug/DebugTab.tsx",
      "src/features/orchestrate/OrchestrateTab.tsx",
      "src/features/audit/AuditTab.tsx",
      "src/features/code/CodeTab.tsx",
      "src/features/git/GitTab.tsx",
      "src/features/models/ModelsTab.tsx",
      "src/features/system/SystemTab.tsx",
    ]) {
      assert.ok(routeCssFiles(source).includes(routingAsset), `${source} must load the shared routing asset`);
    }

    for (const source of [
      "src/features/history/HistoryTab.tsx",
      "src/features/projects/ProjectsTab.tsx",
      "src/features/knowledge/KnowledgeTab.tsx",
      "src/features/playbooks/PlaybooksTab.tsx",
      "src/features/models-cost/ModelsCostTab.tsx",
      "src/features/settings/SettingsTab.tsx",
      "src/features/tokens/TokensTab.tsx",
      "src/features/outcomes/OutcomesTab.tsx",
    ]) {
      assert.ok(!routeCssFiles(source).includes(routingAsset), `${source} must not load unrelated routing CSS`);
    }
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
});

test("optional workspace features are standalone activation roots with isolated CSS", async () => {
  const desktopRoot = fileURLToPath(new URL("../", import.meta.url));
  const outDir = mkdtempSync(join(tmpdir(), "repodesk-optional-features-"));

  try {
    await build({
      root: desktopRoot,
      logLevel: "silent",
      build: { manifest: true, outDir, emptyOutDir: true },
    });

    const emittedManifest = JSON.parse(readFileSync(join(outDir, ".vite/manifest.json"), "utf8"));
    const shellFiles = graphFiles(
      emittedManifest,
      collectStaticGraph(emittedManifest, "index.html"),
    );
    const cssText = (files) => files
      .filter((file) => file.endsWith(".css"))
      .map((file) => readFileSync(join(outDir, file), "utf8"))
      .join("\n");
    const shellCss = cssText(shellFiles);
    const contracts = [
      ["src/shared/ui/CommandPalette.tsx", [".cmdk-panel-v2"]],
      ["src/app/WorkbenchBottomPanel.tsx", [".interactive-terminal", ".task-runner-panel"]],
      ["src/app/InteractiveTerminal.tsx", [".xterm"]],
      ["src/features/health/IDEHealthPanel.tsx", [".ide-health-overlay"]],
    ];
    const activationGraphAllowlist = {
      shellRootKey: "index.html",
      allowedShellRootKeys: ["index.html"],
      allowedSharedChunkNames: ["vendor-react", "vendor-query", "vendor-tauri"],
    };

    for (const [source, selectors] of contracts) {
      const rootKey = findChunkBySource(emittedManifest, source);
      const featureFiles = graphFiles(emittedManifest, collectStaticGraph(emittedManifest, rootKey));
      const featureCss = cssText(featureFiles);

      assert.deepEqual(
        activatedFeatureEagerFiles(emittedManifest, rootKey, activationGraphAllowlist),
        [],
        `${source} implementation graph must not overlap the eager shell graph`,
      );
      for (const selector of selectors) {
        assert.ok(featureCss.includes(selector), `${selector} must load with ${source}`);
        assert.ok(!shellCss.includes(selector), `${selector} must not be emitted in eager shell CSS`);
      }
    }

    assert.ok(shellCss.includes(".ide-health-indicator"), "the IDE Health trigger must remain in shell CSS");
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
});

test("entry audit treats Code as the editor activation boundary", async () => {
  const desktopRoot = fileURLToPath(new URL("../", import.meta.url));
  const outDir = join(desktopRoot, "dist");

  await build({
    root: desktopRoot,
    logLevel: "silent",
    build: { manifest: true, outDir, emptyOutDir: true },
  });

  const audit = spawnSync(process.execPath, ["scripts/check-entry-budget.mjs"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });
  const output = `${audit.stdout}\n${audit.stderr}`;

  assert.match(output, /Editor vendor graph: \d+\.\d+ kB JavaScript, 0\.0 kB CSS/);
  assert.doesNotMatch(
    output,
    /Missing manifest source: src\/features\/code\/SemanticCodeEditor\.tsx/,
    output,
  );
});
