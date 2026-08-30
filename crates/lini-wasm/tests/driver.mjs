// Compile and highlight every sample through the browser artifact and write
// the results where `tests/wasm.rs` can diff them against the binary's own.
//
//   node driver.mjs <pkg-dir> <out-dir> <sample.lini>…
//
// Deliberately the *same* entry points a web page calls — `compile(src)` and
// `highlight(src)` — so the test exercises the shipped path rather than a
// test-only shortcut.

import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { pathToFileURL } from "node:url";

const [pkgDir, outDir, ...samples] = process.argv.slice(2);

const { default: init, compile, highlight } = await import(
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
    const src = readFileSync(path, "utf8");
    // Highlighting first: it never throws, so a sample the compiler rejects
    // still reports its listing rather than nothing.
    writeFileSync(`${outDir}/${name}.html`, highlight(src));
    writeFileSync(`${outDir}/${name}.svg`, compile(src));
  } catch (e) {
    // A sample the browser build rejects is a real difference — record it as
    // the output so the Rust side reports a diff rather than a missing file.
    writeFileSync(`${outDir}/${name}.svg`, `WASM ERROR: ${e.message ?? e}\n`);
    failed++;
  }
}
if (failed) console.error(`${failed} sample(s) threw in wasm`);
