// Headless regression test for host-size preservation across runtime swaps.
//
//   A. frontend -> game: the gameplay runtime must inherit the real canvas
//      backing-store size that the frontend already knew via on_resize().
//   B. frontend 640 <-> 800 debug switch: the rebuilt frontend must keep the
//      real host size instead of falling back to its logical resolution.
//
//   node tools/host_resize_smoke.mjs
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');

const KIND = { game: 0, graph: 1, objects: 2, exe: 3, names: 4, menu640: 5, menu800: 6, newh640: 7, newh800: 8 };

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

function loadFile(file, fn, id) {
  return readFile(join(baseDir, file)).then((buf) => {
    const ptr = wasm.alloc(buf.length);
    new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
    if (id === undefined) fn(ptr, buf.length);
    else fn(id, ptr, buf.length);
  });
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
for (const [res, slot, kind] of [
  ['640', 'menu', KIND.menu640], ['640', 'newh', KIND.newh640],
  ['800', 'menu', KIND.menu800], ['800', 'newh', KIND.newh800],
]) {
  await loadFile(manifest.menu[res][slot], (p, l) => wasm.load_resource(kind, p, l));
}
console.log('loaded resources + menu RES');

function fail(msg) { throw new Error(msg); }
function expect(cond, msg) { if (!cond) fail(msg); }
function expectHost(w, h, where) {
  const gw = wasm.host_w();
  const gh = wasm.host_h();
  expect(gw === w && gh === h, `${where}: host ${gw}x${gh}, expected ${w}x${h}`);
}

// --- A. frontend -> game preserves the canvas backing store ---
expect(wasm.frontend_boot(800) === 1, 'frontend_boot(800) failed');
// Simulate a large real canvas (e.g. 1920x1080 window at dpr 1, or 960x540 at dpr 2).
wasm.on_resize(1600, 1200);
expectHost(1600, 1200, 'frontend before transition');

// Drive the original menu with host coords (host = logical * 2 here).
const H = (x, y) => [x * 2, y * 2];
wasm.pointer_down(1, ...H(285, 308)); // New Game
wasm.pointer_up(1, ...H(285, 308));
wasm.step(0.016);
expect(wasm.frontend_state() === 1, 'expected NewHero after New Game');

wasm.pointer_down(1, ...H(270, 113)); // select hero 2
wasm.pointer_up(1, ...H(270, 113));
expect(wasm.frontend_selected_hero() === 2, 'expected hero 2 selected');

wasm.pointer_down(1, ...H(95, 540)); // Ok (region 20, left)
wasm.pointer_up(1, ...H(95, 540));
wasm.step(0.016);
expect(wasm.frontend_is_active() === 0, 'expected game runtime after Ok confirm');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'expected 800x600 game frame');
expectHost(1600, 1200, 'game after frontend -> game transition');
console.log('A. frontend -> game host preserved: 1600x1200');

// --- B. frontend resolution switch preserves the canvas backing store ---
expect(wasm.frontend_boot(800) === 1, 'frontend_boot(800) failed (B)');
wasm.on_resize(1000, 700);
expectHost(1000, 700, 'frontend before resolution switch');

expect(wasm.frontend_set_resolution(640) === 1, 'frontend_set_resolution(640) failed');
expect(wasm.frontend_is_active() === 1, 'expected frontend after resolution switch');
expect(wasm.frame_w() === 640 && wasm.frame_h() === 480, 'expected 640x480 frontend frame');
expectHost(1000, 700, 'frontend after 800 -> 640 switch');

expect(wasm.frontend_set_resolution(800) === 1, 'frontend_set_resolution(800) failed');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'expected 800x600 frontend frame');
expectHost(1000, 700, 'frontend after 640 -> 800 switch');
console.log('B. frontend resolution switch host preserved: 1000x700');

console.log('HOST RESIZE SMOKE OK');
