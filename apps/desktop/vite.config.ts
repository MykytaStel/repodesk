import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';

const FRONTEND_CHUNK_BUDGET_BYTES = 500_000;
const CODEMIRROR_CORE_PACKAGES = [
  '/node_modules/@codemirror/state/',
  '/node_modules/@codemirror/view/',
  '/node_modules/@codemirror/commands/',
  '/node_modules/@codemirror/search/',
  '/node_modules/@codemirror/autocomplete/',
  '/node_modules/@codemirror/lint/',
  '/node_modules/@codemirror/language/',
];

function vendorChunk(id: string): string | undefined {
  const normalized = id.replace(/\\/g, '/');
  if (!normalized.includes('/node_modules/')) return undefined;

  if (
    normalized.includes('/node_modules/react/') ||
    normalized.includes('/node_modules/react-dom/') ||
    normalized.includes('/node_modules/scheduler/')
  ) {
    return 'vendor-react';
  }
  if (normalized.includes('/node_modules/@tanstack/')) return 'vendor-query';
  if (normalized.includes('/node_modules/@tauri-apps/')) return 'vendor-tauri';
  if (CODEMIRROR_CORE_PACKAGES.some((segment) => normalized.includes(segment))) {
    return 'vendor-editor-core';
  }
  // Language parsers (`@codemirror/lang-*`, `@lezer/*`) are intentionally not
  // grouped here. `SemanticCodeEditor` imports them lazily per opened language;
  // allowing Rollup to keep those dynamic graphs separate prevents one large
  // editor vendor chunk from defeating the lazy-loading boundary.
  if (normalized.includes('/node_modules/@xterm/')) return 'vendor-terminal';
  return undefined;
}

function chunkBudget(): Plugin {
  return {
    name: 'repodesk-chunk-budget',
    generateBundle(_options, bundle) {
      const oversized = Object.values(bundle)
        .filter((output) => output.type === 'chunk')
        .map((output) => ({
          fileName: output.fileName,
          bytes: new TextEncoder().encode(output.code).byteLength,
        }))
        .filter(({ bytes }) => bytes > FRONTEND_CHUNK_BUDGET_BYTES)
        .sort((a, b) => b.bytes - a.bytes);

      if (oversized.length === 0) return;

      const details = oversized
        .map(({ fileName, bytes }) => `${fileName}: ${(bytes / 1_000).toFixed(1)} kB`)
        .join(', ');
      this.error(
        `RepoDesk frontend chunk budget exceeded (${FRONTEND_CHUNK_BUDGET_BYTES / 1_000} kB): ${details}`,
      );
    },
  };
}

export default defineConfig({
  plugins: [react(), chunkBudget()],
  clearScreen: false,
  build: {
    rollupOptions: {
      output: {
        // Keep the application shell and feature routes small while preserving
        // stable cache boundaries for the few heavy third-party subsystems.
        manualChunks: vendorChunk,
      },
    },
  },
  server: {
    host: '127.0.0.1',
    port: 5177,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
});
