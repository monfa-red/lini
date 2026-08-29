// Compile every sample through the browser artifact and write the SVGs where
// `tests/wasm.rs` can diff them against the binary's own output.
//
//   node driver.mjs <pkg-dir> <out-dir> <sample.lini>…
//
// Deliberately the *same* entry point a web page calls — `compile(src)` — so
// the test exercises the shipped path rather than a test-only shortcut.

import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { pathToFileURL } from "node:url";

const [pkgDir, outDir, ...samples] = process.argv.slice(2);

const { default: init, compile } = await import(
  pathToFileURL(`${pkgDir}/lini_wasm.js`).href
);
await init({ module_or_path: readFileSync(`${pkgDir}/lini_wasm_bg.wasm`) });

let failed = 0;
for (const [i, path] of samples.entries()) {
  // Keyed by position in the argument list — the one fact the Rust side
  // shares — because basenames collide: the corpus carries a samples/ sheet
  // and a tests/fixtures/routing/ fixture both named links_hard.lini, and a
  // flat name would let the later compile silently overwrite the earlier.
  const name = `${i}-${basename(path).replace(/\.lini$/, "")}`;
  try {
    writeFileSync(`${outDir}/${name}.svg`, compile(readFileSync(path, "utf8")));
  } catch (e) {
    // A sample the browser build rejects is a real difference — record it as
    // the output so the Rust side reports a diff rather than a missing file.
    writeFileSync(`${outDir}/${name}.svg`, `WASM ERROR: ${e.message ?? e}\n`);
    failed++;
  }
}
if (failed) console.error(`${failed} sample(s) threw in wasm`);
