// Run the playground's tokenizer over every sample and write the markup where
// `tests/playground.rs` can diff it against `lini::highlight_html`.
//
//   node playground_driver.mjs <tokenizer.mjs> <out-dir> <sample.lini>…
//
// The tokenizer module is lifted verbatim from the marked region of
// `src/serve/playground.html` by the Rust side — nothing is re-typed here, so
// what runs is exactly what the page runs.

import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { pathToFileURL } from "node:url";

const [modulePath, outDir, ...samples] = process.argv.slice(2);
const { tokenize } = await import(pathToFileURL(modulePath).href);

for (const [i, path] of samples.entries()) {
  // Keyed by position, mirroring the Rust side: the corpus carries two
  // different files named links_hard.lini, and a flat name would let one
  // silently overwrite the other.
  const name = `${i}-${basename(path).replace(/\.lini$/, "")}`;
  writeFileSync(`${outDir}/${name}.html`, tokenize(readFileSync(path, "utf8")));
}
