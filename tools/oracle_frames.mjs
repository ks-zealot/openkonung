// Remake half of the original-vs-remake screenshot harness.
//
//   node web/tools/oracle_frames.mjs [--out DIR]
//
// Renders the boot state (Dragomir, loc 4, cell 72,34, only the leader selected)
// at the ambient phases and render-toggle combinations the original side
// captures with `original_cheat.py frame` + a window screenshot. The frame set
// is four phase plateaus (0 neutral, 6000 dusk ramp, 12000 night, 18000 neutral
// after sunrise); the configs are the three render toggles one at a time.
//
// The output names line up with `tools/oracle_capture.py`: remake-<frame>-<cfg>.png.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');

// --- tiny PNG encoder (RGBA8, no filtering) --------------------------------
function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xEDB88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

function encodePng(w, h, rgba) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const stride = w * 4;
  const raw = Buffer.alloc((stride + 1) * h);
  for (let y = 0; y < h; y++) {
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 6 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

// --- argument ---------------------------------------------------------------
let outDir = join(here, 'oracle');
for (let i = 2; i < process.argv.length; i++) {
  if (process.argv[i] === '--out' && process.argv[i + 1]) outDir = resolve(process.argv[++i]);
}

const FRAMES = [0, 6000, 12000, 18000];
// name -> [shadows, day_night, light_effects]
const CONFIGS = {
  all: [1, 1, 1],
  noshadow: [0, 1, 1],
  dayonly: [1, 0, 1],
  nolight: [1, 1, 0],
};
// The leader's start cell and the neutral pointer spot (free ground, right of
// the viewport) used by the original capture.
const CELL = [72, 34];
const POINTER = [780, 20];

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

const pack = await readFile(join(webDir, 'www', manifest.pack.file));
const pptr = wasm.alloc(pack.length);
new Uint8Array(mem.buffer, pptr, pack.length).set(pack);
if (wasm.load_pack(pptr, pack.length) !== 1) throw new Error('load_pack failed');
if (wasm.boot(0, -1) !== 1) throw new Error('boot failed');
wasm.on_resize(800, 600);

// The original capture clicks the leader's portrait first (the strip starts
// hidden and `_DAT_0081D600` is only set by a party-cell activation). Match it
// so both sides render the same HUD.
wasm.pointer_down(1, 36, 36);
wasm.pointer_up(1, 36, 36);
wasm.step(0.016);

const CONFIG_LEN = 28;
function configBytes([shadows, dayNight, lightEffects]) {
  const buf = Buffer.alloc(CONFIG_LEN);
  // Order from `config.rs`: video, transparency, day/night, light effects,
  // music, shadows, sound.
  const words = [0x114, 1, dayNight, lightEffects, 0, shadows, 0];
  words.forEach((v, i) => buf.writeUInt32LE(v >>> 0, i * 4));
  return buf;
}

function applyConfig(words) {
  const buf = configBytes(words);
  const ptr = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
  if (wasm.config_load(ptr, buf.length) !== 1) throw new Error('config_load failed');
}

function save(name) {
  const w = wasm.frame_w();
  const h = wasm.frame_h();
  const view = Buffer.from(new Uint8Array(mem.buffer, wasm.frame_ptr(), w * h * 4));
  return writeFile(join(outDir, name), encodePng(w, h, view));
}

await mkdir(outDir, { recursive: true });
for (const frame of FRAMES) {
  for (const [name, words] of Object.entries(CONFIGS)) {
    applyConfig(words);
    wasm.debug_set_player_cell(CELL[0], CELL[1]);
    // The original's New Game leaves the leader facing 4; the facing changes
    // the sprite anchor and therefore the camera target.
    wasm.debug_set_player_facing(4);
    wasm.debug_set_world_frame(frame, 0);
    // dt 0: no logic-frame advance, so the ambient phase stays exactly `frame`,
    // but `step` still repaints.
    wasm.step(0);
    wasm.pointer_move(POINTER[0], POINTER[1]);
    wasm.step(0);
    const actual = wasm.debug_world_frame_lo();
    if (actual !== frame) throw new Error(`frame ${frame} became ${actual}`);
    await save(`remake-${frame}-${name}.png`);
    console.log(`remake-${frame}-${name}.png  config=${words.join(',')}`);
  }
}
console.log(`wrote ${FRAMES.length * Object.keys(CONFIGS).length} PNGs to ${outDir}`);
