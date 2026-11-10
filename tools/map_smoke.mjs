// Headless world-map smoke test: open mode 5, verify a revealed region, then
// click it to travel through the transition record.
//
//   node web/tools/map_smoke.mjs
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
  await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
}
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (a, ptr, len) => wasm.load_profile(a, ptr, len), id);
}
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}

if (wasm.boot(0, -1) !== 1) throw new Error('boot failed');
wasm.on_resize(800, 600);
for (let i = 0; i < 5; i++) wasm.step(0.016);

const startMap = wasm.debug_current_map();
if (wasm.debug_reveal_all_locations() !== 1) throw new Error('reveal failed');
wasm.key_down(77); // M
for (let i = 0; i < 3; i++) wasm.step(0.016);
if (!wasm.debug_map_open()) throw new Error('map did not open');

const fw = wasm.frame_w();
const fh = wasm.frame_h();
const view = new Uint8Array(mem.buffer, wasm.frame_ptr(), fw * fh * 4);
let mapPixels = 0;
for (let y = 60; y < 420; y++) {
  for (let x = 220; x < 722; x++) {
    const i = (y * fw + x) * 4;
    if (view[i] > 40 || view[i + 1] > 40 || view[i + 2] > 40) mapPixels++;
  }
}
console.log('map frame', { fw, fh, mapPixels, startMap });
if (mapPixels < 10000) throw new Error('map overlay did not render');

// Location 2 must have a transition record in Game.1; click its first pixel.
const loc = 2;
const packed = wasm.debug_map_location_point(loc);
if (packed === 0xffffffff) throw new Error(`no map run for location ${loc}`);
const lx = packed >>> 16;
const ly = packed & 0xffff;
// Map origin at 800x600 is (220,60); host == virtual here.
wasm.pointer_down(1, 220 + lx + 2, 60 + ly + 2);
wasm.pointer_up(1, 220 + lx + 2, 60 + ly + 2);
for (let i = 0; i < 3; i++) wasm.step(0.016);
const endMap = wasm.debug_current_map();
console.log('travel', { loc, startMap, endMap, mapOpen: wasm.debug_map_open() });
if (endMap !== loc) throw new Error(`map travel did not reach ${loc}`);
if (wasm.debug_map_open()) throw new Error('map stayed open after travel');

console.log('MAP SMOKE OK');
