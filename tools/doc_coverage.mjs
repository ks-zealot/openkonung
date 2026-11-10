// Rustdoc coverage check for the Rust sources in `web/`.
//
// `#![warn(missing_docs)]` (src/lib.rs) only sees items that are reachable from
// the crate root, and it cannot see private helpers at all on stable Rust. Most
// of the ported logic lives in private and `pub(super)` functions, so the lint
// alone cannot prove that "every function is documented".
//
// This tool parses the sources directly and reports:
//   - every `fn` (free function, inherent method, trait method) without a
//     preceding `///` doc comment,
//   - every file without a `//!` module header.
//
// Attributes between the doc comment and the item are allowed, matching rustc:
//
//     /// Summary.
//     #[test]
//     fn case() {}
//
// Usage:
//     node tools/doc_coverage.mjs            # list gaps, exit 1 when any
//     node tools/doc_coverage.mjs --summary  # counts only, still exit 1
//     node tools/doc_coverage.mjs --warn     # exit 0, for local exploration
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');

// Generated tables are written by `tools/gen_ui_tables.py` and
// `tools/gen_world_map.py`; their rows are data, not hand-written code. They
// only need a module header, which the generators emit.
const GENERATED = new Set(['src/hud/buttons_data.rs', 'src/world_map/data.rs']);

const args = new Set(process.argv.slice(2));
const summaryOnly = args.has('--summary');
const warnOnly = args.has('--warn');

// `fn name(`, optionally qualified by `pub`, `pub(crate)`, `const`, `async`,
// `unsafe` and `extern "C"`. Deliberately anchored at the start of a line so a
// `fn` mentioned inside prose or a string is not counted.
const FN = /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:default|const|async|unsafe|extern\s+"[^"]*")\s+)*fn\s+([A-Za-z0-9_]+)/;

function isDocLine(line) {
  return /^\s*\/\/[\/!]/.test(line);
}

function isAttrLine(line) {
  return /^\s*#!?\[/.test(line);
}

// True when the item at `index` is preceded by a `///` comment, allowing
// attributes (including multi-line ones) between the two.
function hasDoc(lines, index) {
  let i = index - 1;
  while (i >= 0) {
    const line = lines[i];
    if (isAttrLine(line)) {
      // Multi-line attribute: walk back to its opening line before continuing.
      if (!/\]\s*$/.test(line)) {
        let depth = 0;
        for (let j = i; j >= 0; j -= 1) {
          depth += (lines[j].match(/\[/g) || []).length;
          depth -= (lines[j].match(/\]/g) || []).length;
          if (depth <= 0) {
            i = j;
            break;
          }
        }
      }
      i -= 1;
      continue;
    }
    return isDocLine(line);
  }
  return false;
}

function hasModuleDoc(lines) {
  // The header must be the first non-blank, non-attribute content of the file.
  for (const line of lines) {
    if (line.trim() === '') continue;
    if (isAttrLine(line)) return false; // `#![...]` before any `//!`
    return /^\s*\/\/!/.test(line);
  }
  return false;
}

async function* walk(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(full);
    } else if (entry.name.endsWith('.rs')) {
      yield full;
    }
  }
}

const missingFns = [];
const missingHeaders = [];
let files = 0;
let fnCount = 0;
let generated = 0;

for (const dir of ['src', 'examples']) {
  for await (const full of walk(join(webDir, dir))) {
    const rel = relative(webDir, full).replaceAll('\\', '/');
    if (GENERATED.has(rel)) {
      generated += 1;
      continue;
    }
    files += 1;
    const lines = (await readFile(full, 'utf8')).split(/\r?\n/);
    if (!hasModuleDoc(lines)) missingHeaders.push(rel);

    for (let i = 0; i < lines.length; i += 1) {
      if (!FN.test(lines[i])) continue;
      fnCount += 1;
      if (!hasDoc(lines, i)) {
        missingFns.push(`${rel}:${i + 1}: ${lines[i].trim()}`);
      }
    }
  }
}

const scope = `${files} files, ${fnCount} functions (${generated} generated files skipped)`;
if (!summaryOnly) {
  for (const gap of missingHeaders) console.log(`module header  ${gap}`);
  for (const gap of missingFns) console.log(`fn doc         ${gap}`);
}
console.log(
  `doc coverage: ${fnCount - missingFns.length}/${fnCount} functions documented, ` +
    `${files - missingHeaders.length}/${files} module headers (${scope})`
);

const gaps = missingFns.length + missingHeaders.length;
if (gaps > 0) {
  console.log(`missing: ${missingFns.length} function docs, ${missingHeaders.length} module headers`);
  if (!warnOnly) process.exit(1);
}
