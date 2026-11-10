// Headless save/load smoke test.
//
//   node web/tools/save_smoke.mjs
//
// Proves: compressed snapshot creation, state restoration into a live game,
// and loading a snapshot into a fresh wasm instance (no prior boot()).
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const KIND = { game: 0, graph: 1, objects: 2, exe: 3, names: 4 };

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
  async function loadMenu(res) {
    const m = manifest.menu[res];
    const kinds = {
      menu: res === "640" ? 5 : 6,
      newh: res === "640" ? 7 : 8,
      save: res === "640" ? 9 : 10,
    };
    for (const key of Object.keys(kinds)) {
      if (m[key]) await loadFile(m[key], (p, l) => wasm.load_resource(kinds[key], p, l));
    }
  }
  return { wasm, mem, loadMenu };
}

function snapshot(wasm, mem) {
  if (wasm.save_snapshot() !== 1) throw new Error('save_snapshot failed');
  const ptr = wasm.save_snapshot_ptr();
  const len = wasm.save_snapshot_len();
  if (!ptr || !len) throw new Error('empty snapshot');
  return new Uint8Array(mem.buffer, ptr, len).slice();
}

function load(wasm, mem, bytes) {
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(mem.buffer, ptr, bytes.length).set(bytes);
  const ok = wasm.load_snapshot(ptr, bytes.length);
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, bytes.length);
  return ok;
}

const a = await makeRuntime();
if (a.wasm.boot(0, -1) !== 1) throw new Error('boot failed');
a.wasm.on_resize(1600, 1200);
for (let i = 0; i < 4; i++) a.wasm.step(0.016);
const beforeX = a.wasm.debug_player_x();
const beforeY = a.wasm.debug_player_y();
console.log('before save player', beforeX.toFixed(2), beforeY.toFixed(2));

const bytes = snapshot(a.wasm, a.mem);
console.log('snapshot bytes', bytes.length, 'header', new TextDecoder().decode(bytes.subarray(0, 4)));
if (bytes.length >= 0x20000) throw new Error('snapshot was not compressed');

// Move the hero, then restore.
a.wasm.pointer_down(1, 1200, 700);
a.wasm.pointer_up(1, 1200, 700);
for (let i = 0; i < 40; i++) a.wasm.step(0.016);
const movedX = a.wasm.debug_player_x();
const movedY = a.wasm.debug_player_y();
console.log('after move player', movedX.toFixed(2), movedY.toFixed(2));

if (load(a.wasm, a.mem, bytes) !== 1) throw new Error('load_snapshot failed (live)');
for (let i = 0; i < 2; i++) a.wasm.step(0.016);
const restoredX = a.wasm.debug_player_x();
const restoredY = a.wasm.debug_player_y();
console.log('restored player', restoredX.toFixed(2), restoredY.toFixed(2));
if (Math.abs(restoredX - beforeX) > 0.01 || Math.abs(restoredY - beforeY) > 0.01) {
  throw new Error('live load did not restore the player cell');
}
if (restoredX === movedX && restoredY === movedY) {
  throw new Error('player did not move before load');
}

// Frontend request plumbing: Save menu -> slot 0, Load menu -> slot 1.
await a.loadMenu('800');
if (a.wasm.frontend_boot(800) !== 1) throw new Error('frontend_boot failed');
a.wasm.on_resize(1600, 1200);
const host = (x, y) => [x * 2, y * 2];
let [sx, sy] = host(396, 227);
a.wasm.pointer_down(1, sx, sy);
a.wasm.pointer_up(1, sx, sy);
[sx, sy] = host(406, 105);
a.wasm.pointer_down(1, sx, sy);
a.wasm.pointer_up(1, sx, sy);
if (a.wasm.take_save_request() !== 0) throw new Error('save slot request not queued');
a.wasm.key_down(27); // Escape: back to the main menu
let [lx, ly] = host(376, 276);
a.wasm.pointer_down(1, lx, ly);
a.wasm.pointer_up(1, lx, ly);
[lx, ly] = host(406, 143);
a.wasm.pointer_down(1, lx, ly);
a.wasm.pointer_up(1, lx, ly);
if (a.wasm.take_load_request() !== 1) throw new Error('load slot request not queued');
console.log('frontend slot requests OK');
// Fresh runtime: no boot(), load directly.
const b = await makeRuntime();
if (load(b.wasm, b.mem, bytes) !== 1) throw new Error('load_snapshot failed (fresh)');
for (let i = 0; i < 3; i++) b.wasm.step(0.016);
if (b.wasm.debug_game_exists() !== 1) throw new Error('fresh load did not create a game');
const freshX = b.wasm.debug_player_x();
const freshY = b.wasm.debug_player_y();
console.log('fresh load player', freshX.toFixed(2), freshY.toFixed(2));
if (Math.abs(freshX - beforeX) > 0.01 || Math.abs(freshY - beforeY) > 0.01) {
  throw new Error('fresh load did not restore the player cell');
}
if (b.wasm.frame_ptr() === 0) throw new Error('fresh load produced no frame');

console.log('SAVE SMOKE OK');
