// Headless legacy save-interop smoke test.
//
//   node web/tools/legacy_smoke.mjs
//
// Proves: `legacy_export_size/ptr` produces an exact 0xB07A4 KiB-free
// `KONUNG.SAn` payload, `legacy_import` loads it into a live game, and the
// `KONUNG.SAV` name table round-trips through the CP866 ABI.
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const KIND = { game: 0, graph: 1, objects: 2, exe: 3, names: 4 };
const SNAPSHOT_LEN = 0x0b07a4;

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);

async function makeRuntime() {
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
    await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(KIND.names, p, l));
  }
  for (const p of manifest.profiles) {
    const id = parseInt(p.replace(/\D/g, ''), 10);
    await loadFile(p, (id2, ptr, len) => wasm.load_profile(id2, ptr, len), id);
  }
  for (const kon of manifest.konungs) {
    await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
  }
  return { wasm, mem };
}

function exportLegacy(wasm, mem) {
  const len = wasm.legacy_export_size() >>> 0;
  if (!len) throw new Error('legacy_export_size returned 0');
  const ptr = wasm.legacy_export_ptr();
  return new Uint8Array(mem.buffer, ptr, len).slice();
}

function importLegacy(wasm, mem, bytes) {
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(mem.buffer, ptr, bytes.length).set(bytes);
  const ok = wasm.legacy_import(ptr, bytes.length);
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, bytes.length);
  return ok;
}

const a = await makeRuntime();
if (a.wasm.boot(0, -1) !== 1) throw new Error('boot failed');
a.wasm.on_resize(1600, 1200);
for (let i = 0; i < 4; i++) a.wasm.step(0.016);

const x0 = a.wasm.debug_player_x();
const y0 = a.wasm.debug_player_y();
const snap = exportLegacy(a.wasm, a.mem);
console.log('legacy snapshot', snap.length, 'bytes');
if (snap.length !== SNAPSHOT_LEN) {
  throw new Error(`expected ${SNAPSHOT_LEN} bytes, got ${snap.length}`);
}

// Import into a fresh instance (no boot()).
const b = await makeRuntime();
if (importLegacy(b.wasm, b.mem, snap) !== 1) throw new Error('legacy_import failed');
b.wasm.on_resize(1600, 1200);
for (let i = 0; i < 4; i++) b.wasm.step(0.016);
const x1 = b.wasm.debug_player_x();
const y1 = b.wasm.debug_player_y();
console.log('imported player', x1.toFixed(2), y1.toFixed(2));
if (Math.abs(x1 - x0) > 0.01 || Math.abs(y1 - y0) > 0.01) {
  throw new Error('legacy import did not restore the leader cell');
}

// Import into the live game too (mode-1 path on an existing session).
if (importLegacy(a.wasm, a.mem, snap) !== 1) throw new Error('live legacy_import failed');

// KONUNG.SAV: build 500-byte CP866 names, parse them back through the ABI.
const names = new Array(10).fill('');
names[0] = 'Драконий остров';
names[9] = 'Save 10';
const text = new TextEncoder().encode(names.join('\0'));
let ptr = a.wasm.alloc(text.length);
new Uint8Array(a.mem.buffer, ptr, text.length).set(text);
const nameLen = a.wasm.legacy_names_build(ptr, text.length) >>> 0;
if (typeof a.wasm.dealloc === 'function') a.wasm.dealloc(ptr, text.length);
if (nameLen !== 500) throw new Error(`KONUNG.SAV length ${nameLen}`);
const filePtr = a.wasm.legacy_export_ptr();
const file = new Uint8Array(a.mem.buffer, filePtr, nameLen).slice();
ptr = a.wasm.alloc(file.length);
new Uint8Array(a.mem.buffer, ptr, file.length).set(file);
if (a.wasm.legacy_names_parse(ptr, file.length) !== 1) throw new Error('legacy_names_parse failed');
if (typeof a.wasm.dealloc === 'function') a.wasm.dealloc(ptr, file.length);
const tp = a.wasm.legacy_names_text_ptr();
const tl = a.wasm.legacy_names_text_len();
const decoded = new TextDecoder().decode(new Uint8Array(a.mem.buffer, tp, tl)).split('\0');
if (decoded[0] !== 'Драконий остров' || decoded[9] !== 'Save 10') {
  throw new Error('KONUNG.SAV names did not round-trip: ' + JSON.stringify(decoded));
}
console.log('KONUNG.SAV names OK:', decoded[0], '/', decoded[9]);

console.log('LEGACY SMOKE OK');
