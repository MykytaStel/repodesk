import { readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { gzipSync } from "node:zlib";

const dist = new URL("../dist/", import.meta.url);
const html = readFileSync(new URL("index.html", dist), "utf8");
const assetPaths = [
  ...html.matchAll(/<script[^>]+src="([^"]+\.js)"/g),
  ...html.matchAll(/<link[^>]+rel="modulepreload"[^>]+href="([^"]+\.js)"/g),
].map((match) => match[1]);
const uniquePaths = [...new Set(assetPaths)];
const forbidden = uniquePaths.filter((path) => /vendor-(terminal|editor-core)/.test(basename(path)));

if (forbidden.length > 0) {
  throw new Error(`Heavy optional code is eagerly preloaded: ${forbidden.join(", ")}`);
}

const gzipBytes = uniquePaths.reduce((total, path) => {
  const relative = path.replace(/^\//, "");
  return total + gzipSync(readFileSync(join(dist.pathname, relative))).byteLength;
}, 0);
const budgetBytes = 110_000;

if (gzipBytes > budgetBytes) {
  throw new Error(`Initial JavaScript gzip budget exceeded: ${gzipBytes} > ${budgetBytes} bytes`);
}

console.log(`OK: initial JavaScript ${gzipBytes} gzip bytes across ${uniquePaths.length} files`);
