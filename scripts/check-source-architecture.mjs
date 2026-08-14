import { execFileSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const HARD_SOURCE_LIMIT_BYTES = 28 * 1024;
const SOURCE_EXTENSIONS = new Set([".rs", ".ts", ".tsx", ".mjs"]);

function extensionOf(path) {
  const index = path.lastIndexOf(".");
  return index >= 0 ? path.slice(index) : "";
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

function changedSourcePaths(baseSha) {
  const output = execFileSync("git", ["diff", "--name-only", "-z", `${baseSha}...HEAD`, "--"], {
    cwd: process.cwd(),
  });

  return output
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .filter((path) => SOURCE_EXTENSIONS.has(extensionOf(path)));
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

export function runArchitectureRatchet() {
  const baseSha = resolveBaseSha();
  const paths = changedSourcePaths(baseSha);
  const failures = [];

  for (const path of paths) {
    const currentSize = existsSync(path) ? statSync(path).size : null;
    const baseSize = baseFileSize(baseSha, path);
    const failure = evaluateSourceChange({ path, baseSize, currentSize });
    if (failure) failures.push(failure);
  }

  console.log(`Architecture ratchet: ${paths.length} changed source file(s), base ${baseSha.slice(0, 12)}.`);
  console.log(`Hard limit for new/crossing files: ${HARD_SOURCE_LIMIT_BYTES} bytes (28 KiB).`);

  if (failures.length > 0) {
    console.error("\nSource architecture budget failed:");
    for (const failure of failures) console.error(`- ${failure}`);
    console.error("\nSplit responsibilities or reduce the touched god-file before merging; do not raise the limit to make the gate green.");
    process.exitCode = 1;
  }
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : null;
if (invokedPath && fileURLToPath(import.meta.url) === invokedPath) runArchitectureRatchet();
