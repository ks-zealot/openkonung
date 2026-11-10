// Headless day/night + LIGHTS map smoke.
//
//   node web/tools/lighting_smoke.mjs
//
// Loads the packed resources, boots Dragomir, then compares the rendered world
// at day (frame 0, ambient zero) and night (frame 12000, dark ambient) and
// checks that the per-map LIGHTS.N low/high data was attached.
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

if (!manifest.pack || !manifest.pack.file) throw new Error('manifest has no pack');
const pack = await readFile(join(webDir, 'www', manifest.pack.file));
const ptr = wasm.alloc(pack.length);
new Uint8Array(mem.buffer, ptr, pack.length).set(pack);
if (wasm.load_pack(ptr, pack.length) !== 1) throw new Error('load_pack failed');
if (wasm.boot(0, -1) !== 1) throw new Error('boot failed');
wasm.on_resize(800, 600);

function averageWorld() {
  wasm.step(0.016);
  const p = wasm.frame_ptr();
  const fw = wasm.frame_w();
  const fh = wasm.frame_h();
  const frame = new Uint8Array(mem.buffer, p, fw * fh * 4);
  let sum = 0;
  let n = 0;
  // World viewport above the HUD strip.
  for (let y = 0; y < 470; y += 3) {
    for (let x = 0; x < fw; x += 3) {
      const o = (y * fw + x) * 4;
      sum += frame[o] + frame[o + 1] + frame[o + 2];
      n += 3;
    }
  }
  return sum / n;
}

const lightMaps = wasm.debug_light_map_count();
if (lightMaps !== 40) throw new Error(`expected 40 LIGHTS.N maps, got ${lightMaps}`);

wasm.debug_set_world_frame(0, 0);
const day = averageWorld();
wasm.debug_set_world_frame(12000, 0);
const night = averageWorld();
if (!(night < day * 0.95)) {
  throw new Error(`night frame is not darker: day=${day.toFixed(1)} night=${night.toFixed(1)}`);
}
console.log(`day=${day.toFixed(1)} night=${night.toFixed(1)} maps=${lightMaps}`);
console.log('PASS: day/night ambient darkens the world; 40 LIGHTS.N maps attached');
