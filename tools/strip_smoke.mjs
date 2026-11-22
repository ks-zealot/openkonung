// Headless inventory-strip visibility smoke (`_DAT_0081D600`).
//
//   node web/tools/strip_smoke.mjs
//
// Boots Dragomir from the packed resources and checks the original's strip
// ownership rule:
//   * a fresh game draws no strip (the strip row shows the world);
//   * clicking the leader's portrait shows the leader's strip (INTR[17] slot
//     stone appears), clicking the same portrait again hides it;
//   * opening and closing the character screen shows the edited member's
//     strip (`0x424376`: `0x81D600 = 0x81D630`).
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

const pack = await readFile(join(webDir, 'www', manifest.pack.file));
const pptr = wasm.alloc(pack.length);
new Uint8Array(mem.buffer, pptr, pack.length).set(pack);
if (wasm.load_pack(pptr, pack.length) !== 1) throw new Error('load_pack failed');
if (wasm.boot(0, -1) !== 1) throw new Error('boot failed');
wasm.on_resize(800, 600);
for (let i = 0; i < 5; i++) wasm.step(0.016);

/** Most common colour inside the strip slot band (800x600 layout). */
function dominantStripColor() {
  const p = wasm.frame_ptr();
  const fw = wasm.frame_w();
  const frame = new Uint8Array(mem.buffer, p, fw * wasm.frame_h() * 4);
  const counts = new Map();
  for (let y = 486; y < 556; y++) {
    for (let x = 159; x < 159 + 9 * 69; x++) {
      const o = (y * fw + x) * 4;
      const key = `${frame[o]},${frame[o + 1]},${frame[o + 2]}`;
      counts.set(key, (counts.get(key) || 0) + 1);
    }
  }
  let best = null;
  for (const [key, n] of counts) {
    if (!best || n > best[1]) best = [key, n];
  }
  const rgb = best[0].split(',').map(Number);
  return { rgb, count: best[1] };
}

/** Count `rgb` pixels in the strip slot band. */
function countStripColor(rgb) {
  const p = wasm.frame_ptr();
  const fw = wasm.frame_w();
  const frame = new Uint8Array(mem.buffer, p, fw * wasm.frame_h() * 4);
  let n = 0;
  for (let y = 486; y < 556; y++) {
    for (let x = 159; x < 159 + 9 * 69; x++) {
      const o = (y * fw + x) * 4;
      if (frame[o] === rgb[0] && frame[o + 1] === rgb[1] && frame[o + 2] === rgb[2]) n++;
    }
  }
  return n;
}

function expectVisible(expected, label) {
  const state = wasm.debug_inventory_strip_visible();
  if (state !== (expected ? 1 : 0)) {
    throw new Error(`${label}: strip visible flag ${state}, expected ${expected ? 1 : 0}`);
  }
  return state;
}

expectVisible(false, 'fresh game');
console.log('fresh game -> no strip');

// Leader portrait cell (cell 0) toggles the strip (`0x422D4E`).
wasm.pointer_down(1, 36, 36);
wasm.pointer_up(1, 36, 36);
wasm.step(0.016);
expectVisible(true, 'portrait click');
const shown = dominantStripColor();
if (shown.count < 1000) {
  throw new Error(`portrait click: slot band dominant colour only ${shown.count} px`);
}
const shownCount = countStripColor(shown.rgb);
console.log(`portrait click -> strip shown (${shown.rgb} x${shownCount})`);

wasm.pointer_down(1, 36, 36);
wasm.pointer_up(1, 36, 36);
wasm.step(0.016);
expectVisible(false, 'second portrait click');
const hiddenCount = countStripColor(shown.rgb);
if (hiddenCount > shownCount / 20) {
  throw new Error(
    `second portrait click: slot colour still covers ${hiddenCount}/${shownCount} px`,
  );
}
console.log(`second portrait click -> strip hidden (${hiddenCount} residual px)`);

// Character screen close shows the edited member's strip (`0x424376`).
wasm.key_down(67);
wasm.step(0.016);
if (!wasm.debug_character_screen()) throw new Error('character screen did not open');
wasm.key_down(27);
wasm.step(0.016);
if (wasm.debug_character_screen()) throw new Error('character screen did not close');
expectVisible(true, 'character screen close');
const afterSheet = countStripColor(shown.rgb);
if (afterSheet < shownCount / 2) {
  throw new Error(`character screen close: strip only ${afterSheet}/${shownCount} px`);
}
console.log(`character screen close -> strip shown (${afterSheet} px)`);

console.log('STRIP SMOKE OK');
