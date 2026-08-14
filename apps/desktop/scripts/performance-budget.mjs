import { readFileSync } from "node:fs";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

export function collectStaticGraph(manifest, rootKey) {
  const visited = new Set();
  const visit = (key) => {
    if (visited.has(key)) return;
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Missing manifest chunk: ${key}`);
    visited.add(key);
    for (const dependency of chunk.imports ?? []) visit(dependency);
  };
  visit(rootKey);
  return visited;
}

export function graphFiles(manifest, keys) {
  const files = new Set();
  for (const key of keys) {
    const chunk = manifest[key];
    if (chunk.file) files.add(chunk.file);
    for (const css of chunk.css ?? []) files.add(css);
  }
  return [...files];
}

export function namedGraphFiles(manifest, name) {
  const roots = Object.entries(manifest)
    .filter(([, chunk]) => chunk.name === name)
    .map(([key]) => key);
  if (roots.length === 0) throw new Error(`Missing manifest named chunk: ${name}`);

  const keys = new Set();
  for (const root of roots) {
    for (const key of collectStaticGraph(manifest, root)) keys.add(key);
  }
  return graphFiles(manifest, keys);
}

export function excludeFiles(files, excludedFiles) {
  const excluded = new Set(excludedFiles);
  return files.filter((file) => !excluded.has(file));
}

export function overlappingFiles(files, otherFiles) {
  const other = new Set(otherFiles);
  return files.filter((file) => other.has(file));
}

function directChunkFiles(manifest, key) {
  const chunk = manifest[key];
  if (!chunk) throw new Error(`Missing manifest chunk: ${key}`);
  return [chunk.file, ...(chunk.css ?? [])].filter(Boolean);
}

/**
 * Return feature-graph files that are also eager shell files, excluding only
 * explicitly named platform assets. Allowed roots and named chunks contribute
 * their direct files only: their static dependencies are never transitively
 * allowlisted, so an arbitrary feature child shared with the shell still fails.
 */
export function activatedFeatureEagerFiles(
  manifest,
  rootKey,
  {
    shellRootKey = "index.html",
    allowedShellRootKeys = [],
    allowedSharedChunkNames = [],
  } = {},
) {
  const featureFiles = graphFiles(manifest, collectStaticGraph(manifest, rootKey));
  const shellFiles = graphFiles(manifest, collectStaticGraph(manifest, shellRootKey));
  const allowedFiles = new Set();

  for (const key of allowedShellRootKeys) {
    for (const file of directChunkFiles(manifest, key)) allowedFiles.add(file);
  }
  for (const name of allowedSharedChunkNames) {
    const keys = Object.entries(manifest)
      .filter(([, chunk]) => chunk.name === name)
      .map(([key]) => key);
    if (keys.length === 0) throw new Error(`Missing manifest named chunk: ${name}`);
    for (const key of keys) {
      for (const file of directChunkFiles(manifest, key)) allowedFiles.add(file);
    }
  }

  return overlappingFiles(featureFiles, shellFiles).filter((file) => !allowedFiles.has(file));
}

export function findChunkBySource(manifest, source) {
  const match = Object.entries(manifest).find(([key, chunk]) => key === source || chunk.src === source);
  if (!match) throw new Error(`Missing manifest source: ${source}`);
  return match[0];
}

export function measureGraph({ manifest, rootKey, distPath }) {
  const files = graphFiles(manifest, collectStaticGraph(manifest, rootKey));
  const size = (file) => gzipSync(readFileSync(join(distPath, file))).byteLength;
  return {
    files,
    jsGzip: files.filter((file) => file.endsWith(".js")).reduce((sum, file) => sum + size(file), 0),
    cssGzip: files.filter((file) => file.endsWith(".css")).reduce((sum, file) => sum + size(file), 0),
  };
}
