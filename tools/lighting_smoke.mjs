// Headless day/night + LIGHTS map smoke.
//
//   node web/tools/lighting_smoke.mjs
//
// Loads the packed resources, boots Dragomir, then checks the live ambient /
// light-map behaviour against the original (`FUN_00428FE0` + `FUN_0042532C`):
//
//   * frame 0 is neutral, frame 12000 is the deep-night plateau
//     (`(-60, -50, -70)` -> the world darkens);
//   * the ambient is independent of `light_effects` (`FUN_0043D971` uses the
//     `0x5076BC` delta regardless of the option), so 12000 with the option off
//     is still dark and differs from 12000 with it on only where the `LIGHTS.N`
//     masks are;
//   * `day_night = 0` disables the cycle entirely (12000 looks like frame 0);
//   * the masks run only in the night window `[8100, 18900)`, so the dusk phase
//     6000 looks identical with and without `light_effects`;
//   * all 40 per-map `LIGHTS.N` maps were attached.
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

const CONFIG_LEN = 28;
/** `[shadows, day_night, light_effects]` -> 28-byte KONUNG.CFG payload. */
function configBytes([shadows, dayNight, lightEffects]) {
  const buf = Buffer.alloc(CONFIG_LEN);
  const words = [0x114, 1, dayNight, lightEffects, 0, shadows, 0];
  words.forEach((v, i) => buf.writeUInt32LE(v >>> 0, i * 4));
  return buf;
}

function applyConfig(words) {
  const buf = configBytes(words);
  const p = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, p, buf.length).set(buf);
  if (wasm.config_load(p, buf.length) !== 1) throw new Error('config_load failed');
}

/** Mean RGB of the world viewport at `frame` with the given config. */
function meanAt(frame, words = [1, 1, 1]) {
  applyConfig(words);
  wasm.debug_set_player_cell(72, 34);
  wasm.debug_set_world_frame(frame, 0);
  // dt 0: repaint without advancing the 60 ms frame clock.
  wasm.step(0);
  const p = wasm.frame_ptr();
  const fw = wasm.frame_w();
  const frameBuf = new Uint8Array(mem.buffer, p, fw * wasm.frame_h() * 4);
  let sum = 0;
  let n = 0;
  // World viewport above the HUD strip.
  for (let y = 0; y < 470; y += 3) {
    for (let x = 140; x < fw; x += 3) {
      const o = (y * fw + x) * 4;
      sum += frameBuf[o] + frameBuf[o + 1] + frameBuf[o + 2];
      n += 3;
    }
  }
  return sum / n;
}

const lightMaps = wasm.debug_light_map_count();
if (lightMaps !== 40) throw new Error(`expected 40 LIGHTS.N maps, got ${lightMaps}`);

const day = meanAt(0);
const night = meanAt(12000);
if (!(night < day * 0.95)) {
  throw new Error(`night frame is not darker: day=${day.toFixed(1)} night=${night.toFixed(1)}`);
}

// `light_effects = 0` must keep the ambient (only the masks go away).
const nightNoLight = meanAt(12000, [1, 1, 0]);
if (!(nightNoLight < day * 0.95)) {
  throw new Error(`ambient must survive light_effects=0: day=${day.toFixed(1)} ` +
    `night-nolight=${nightNoLight.toFixed(1)}`);
}

// Day/night off: the night frame renders exactly like the neutral one.
const nightDayOff = meanAt(12000, [1, 0, 1]);
if (Math.abs(nightDayOff - day) > 0.05) {
  throw new Error(`day_night=0 must disable the cycle: day=${day.toFixed(1)} ` +
    `night-dayoff=${nightDayOff.toFixed(1)}`);
}

// Dusk (6000) is outside the night window, so the masks are a no-op there.
const dusk = meanAt(6000, [1, 1, 1]);
const duskNoLight = meanAt(6000, [1, 1, 0]);
if (Math.abs(dusk - duskNoLight) > 0.05) {
  throw new Error(`dusk masks must be off: on=${dusk.toFixed(1)} off=${duskNoLight.toFixed(1)}`);
}

console.log(
  `day=${day.toFixed(1)} night=${night.toFixed(1)} night-nolight=${nightNoLight.toFixed(1)} ` +
  `night-dayoff=${nightDayOff.toFixed(1)} dusk=${dusk.toFixed(1)} maps=${lightMaps}`,
);
console.log('PASS: ambient independent of light_effects; LIGHTS masks night-only; 40 maps attached');
