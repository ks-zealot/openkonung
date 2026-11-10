// Headless integration smoke test for the OpenKonung wasm ABI.
// Mirrors www/app.js resource loading, then drives the app and inspects frames.
//
//   node web/tools/smoke.mjs
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
console.log('loaded Game.res');
await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(KIND.graph, p, l));
console.log('loaded GRAPH.RES');
await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(KIND.objects, p, l));
console.log('loaded Objects.res');
await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(KIND.exe, p, l));
console.log('loaded CLIENT.EXE');
if (manifest.resources.names) {
  await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
  console.log('loaded', manifest.resources.names);
}

for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (id2, ptr, len) => wasm.load_profile(id2, ptr, len), id);
}
console.log('loaded', manifest.profiles.length, 'profiles');

let n = 0;
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
  n++;
}
console.log('loaded', n, 'Konung maps');

const t0 = Date.now();
// hero_index 0 = Dragomir (Game.1, start location 4 = "Старый лагерь").
const ok = wasm.boot(0, -1);
console.log('boot =', ok, 'in', Date.now() - t0, 'ms');
if (ok !== 1) process.exit(1);

wasm.on_resize(1600, 1200);

function frameStats() {
  const ptr = wasm.frame_ptr();
  const len = wasm.frame_w() * wasm.frame_h() * 4;
  const view = new Uint8Array(mem.buffer, ptr, len);
  let nonDark = 0;
  for (let i = 0; i < view.length; i += 4) {
    if (view[i] > 20 || view[i + 1] > 20 || view[i + 2] > 20) nonDark++;
  }
  return { len, nonDark, w: wasm.frame_w(), h: wasm.frame_h() };
}

for (let i = 0; i < 5; i++) wasm.step(0.016);
console.log('detail frame', frameStats());

const textCount = wasm.text_count();
const dec = new TextDecoder();
const lines = [];
for (let i = 0; i < textCount; i++) {
  const ptr = wasm.text_ptr(i);
  const len = wasm.text_len(i);
  if (ptr && len < 300) {
    lines.push(dec.decode(new Uint8Array(mem.buffer, ptr, len)));
  }
}
console.log('hud/text items:', textCount);
for (const l of lines.slice(0, 6)) console.log('  |', l);
const hud = lines.join(' | ');
console.log('HUD shows Dragomir start name "Старый лагерь":', hud.includes('Старый лагерь'));
console.log('HUD shows hero Dragomir / Game.1:', hud.includes('hero:Dragomir Game.1'));

// Click to walk (centre of a 1600x1200 host -> virtual 800x600).
wasm.pointer_down(1, 1100, 500);
wasm.pointer_up(1, 1100, 500);
for (let i = 0; i < 30; i++) wasm.step(0.016);
console.log('after-click frame', frameStats());

// Graph view.
wasm.key_down(72);
for (let i = 0; i < 2; i++) wasm.step(0.016);
console.log('graph frame', frameStats());
console.log('graph text items:', wasm.text_count());
const glabels = [];
for (let i = 0; i < wasm.text_count(); i++) {
  const ptr = wasm.text_ptr(i);
  const len = wasm.text_len(i);
  if (ptr && len && len < 300) glabels.push(dec.decode(new Uint8Array(mem.buffer, ptr, len)));
}
console.log('graph label sample:');
for (const l of glabels.slice(1, 6)) console.log('   ', l);

console.log('SMOKE OK');
