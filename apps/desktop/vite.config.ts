import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

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
  if (
    normalized.includes('/node_modules/@codemirror/') ||
    normalized.includes('/node_modules/@lezer/')
  ) {
    return 'vendor-editor';
  }
  if (normalized.includes('/node_modules/@xterm/')) return 'vendor-terminal';
  return undefined;
}

export default defineConfig({
  plugins: [react()],
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
