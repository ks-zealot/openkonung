// Headless shadow-pass smoke test (`FUN_0043C040` + `FUN_0043D247`).
//
// Boots the game, renders one frame with the `shadows` config word (`0x81D620`)
// enabled and one with it disabled, and proves the two frames differ only by a
// bounded set of darkened pixels (the actor / equipment ground shadows).
//
//   node web/tools/shadow_smoke.mjs
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const KIND = { game: 0, graph: 1, objects: 2, exe: 3 };

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

async function loadFile(file, fn, id) {
  const buf = await readFile(join(baseDir, file));
  const ptr = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
  if (id === undefined) fn(ptr, buf.length);
  else fn(id, ptr, buf.length);
}

await loadFile(manifest.resources.game, (p, l) => wasm.load_resource(KIND.game, p, l));
await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(KIND.graph, p, l));
await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(KIND.objects, p, l));
await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(KIND.exe, p, l));
if (manifest.resources.names) {
  await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
}
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (id2, ptr, len) => wasm.load_profile(id2, ptr, len), id);
}
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}

if (wasm.boot(0, -1) !== 1) {
  console.error('boot failed');
  process.exit(1);
}
wasm.on_resize(1600, 1200);

function snapshot() {
  const ptr = wasm.frame_ptr();
  const len = wasm.frame_w() * wasm.frame_h() * 4;
  return new Uint8Array(mem.buffer, ptr, len).slice();
}

/** Load a 28-byte KONUNG.CFG with the given `shadows` word. */
function setShadows(on) {
  const cfg = new Uint8Array(28);
  const word = (i, v) => cfg.set(new Uint32Array([v]).buffer, i * 4) && 0;
  const dv = new DataView(cfg.buffer);
  dv.setUint32(0, 0x114, true); // 800x600
  for (const i of [1, 2, 3, 4, 6]) dv.setUint32(i * 4, 1, true);
  dv.setUint32(5 * 4, on ? 1 : 0, true);
  const ptr = wasm.alloc(cfg.length);
  new Uint8Array(mem.buffer, ptr, cfg.length).set(cfg);
  return wasm.config_load(ptr, cfg.length) === 1;
}

// Settle on the starting map, then render two frames without advancing time.
for (let i = 0; i < 8; i++) wasm.step(0.016);
setShadows(true);
wasm.step(0.0);
const onSpans = wasm.debug_shadow_spans();
const withShadows = snapshot();
wasm.step(0.0);
const onAgain = snapshot();
setShadows(false);
wasm.step(0.0);
const offSpans = wasm.debug_shadow_spans();
const without = snapshot();

function diff(a, b) {
  let changed = 0;
  let darkened = 0;
  let brightened = 0;
  for (let i = 0; i < a.length; i += 4) {
    const sa = a[i] + a[i + 1] + a[i + 2];
    const sb = b[i] + b[i + 1] + b[i + 2];
    if (sa === sb) continue;
    changed++;
    if (sa < sb) darkened++;
    else brightened++;
  }
  return { changed, darkened, brightened };
}

const stable = diff(withShadows, onAgain);
const removed = diff(withShadows, without);
const total = withShadows.length / 4;
console.log(`shadow spans: on=${onSpans} off=${offSpans}`);
console.log(`idle redraw stability: ${stable.changed} changed pixels (want 0)`);
console.log(`pixels changed by removing the shadow pass: ${removed.changed} / ${total}`);
console.log(`  shadow was darker: ${removed.darkened}   shadow was brighter: ${removed.brightened}`);

const changed = removed.changed;
const brightened = removed.brightened;

if (changed === 0) {
  console.error('SHADOW SMOKE FAILED: shadows changed no pixels');
  process.exit(1);
}
if (brightened !== 0) {
  console.error('SHADOW SMOKE FAILED: the pass must only darken');
  process.exit(1);
}
if (changed > total / 4) {
  console.error('SHADOW SMOKE FAILED: implausibly large shadow area');
  process.exit(1);
}
console.log('SHADOW SMOKE OK');
