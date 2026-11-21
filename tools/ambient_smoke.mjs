// Headless ambient-sound smoke test.
//
// Verifies the original `FUN_00436214` `0x4365d4` behaviour:
//   * the loc-4 bank 280..287 exists in SOUNDS.RES and is queued;
//   * the roll runs once per 60 ms game frame at 1% (`0x62 < rand()%100`);
//   * no ambience is queued while an interface window is open.
//
//   node web/tools/ambient_smoke.mjs
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');

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

// Normally the page loads only the pack; exercise that path first and fall
// back to the per-file resources for a loose development install.
let fromPack = false;
try {
  const pack = await readFile(join(webDir, 'www', 'konung.zip.pack'));
  const ptr = wasm.alloc(pack.length);
  new Uint8Array(mem.buffer, ptr, pack.length).set(pack);
  fromPack = wasm.load_pack(ptr, pack.length) === 1;
} catch {}
if (!fromPack) {
  await loadFile(manifest.resources.game, (p, l) => wasm.load_resource(0, p, l));
  await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(1, p, l));
  await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(2, p, l));
  await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(3, p, l));
  if (manifest.resources.names) {
    await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
  }
  for (const p of manifest.profiles) {
    const id = parseInt(p.replace(/\D/g, ''), 10);
    await loadFile(p, (i, ptr, len) => wasm.load_profile(i, ptr, len), id);
  }
  for (const k of manifest.konungs) {
    await loadFile(k.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), k.id);
  }
  if (!manifest.sounds) fail('manifest has no sound archive');
  await loadFile(manifest.sounds, (p, l) => wasm.load_resource(14, p, l));
}

function fail(msg) {
  console.error('FAIL:', msg);
  process.exit(1);
}

if (wasm.boot(0, -1) !== 1) fail('boot failed');

// The original requires SOUNDS.RES; loc 4's eight ambient clips must exist.
const bank = [];
for (let i = 0; i < 8; i++) {
  const id = 0x100 + (4 - 1) * 8 + i;
  const len = wasm.debug_sound_clip_len(id);
  if (!len) fail(`ambient clip ${id} is missing from SOUNDS.RES`);
  const rate = wasm.debug_sound_rate(id);
  if (rate !== 22050) fail(`ambient clip ${id} base rate ${rate}, expected 22050`);
  bank.push(id);
}

// 6000 original frames (6 minutes of game time) -> ~60 ambient events at 1%.
const base = bank[0];
let ambient = 0;
for (let i = 0; i < 6000; i++) {
  wasm.step(0.06);
  const n = wasm.sound_count();
  for (let j = 0; j < n; j++) {
    const id = wasm.sound_id(j) >>> 0;
    if (id >= base && id < base + 8) ambient++;
  }
}
if (ambient < 30 || ambient > 100) {
  fail(`expected about 60 ambient events in 6000 frames (1%), got ${ambient}`);
}

// While the character screen is open the original is in case 3, not case 0.
wasm.key_down(67); // C
if (!wasm.debug_character_screen()) fail('character screen did not open');
let uiAmbient = 0;
for (let i = 0; i < 600; i++) {
  wasm.step(0.06);
  const n = wasm.sound_count();
  for (let j = 0; j < n; j++) {
    const id = wasm.sound_id(j) >>> 0;
    if (id >= base && id < base + 8) uiAmbient++;
  }
}
if (uiAmbient !== 0) fail(`ambient played with the character screen open (${uiAmbient})`);

console.log(`AMBIENT SMOKE OK (${ambient} ambient events / 6000 frames)`);
