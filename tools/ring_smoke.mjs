// Headless selection-ellipse smoke (`FUN_0042532C` party rings).
//
//   node web/tools/ring_smoke.mjs
//
// Boots Dragomir from the packed resources and checks:
//   * exactly one ellipse is emitted while the leader is selected;
//   * the ellipse is INTR[178] (green) at full health, INTR[126] (yellow) below
//     50 HP and INTR[172] (red) below 25 HP - the `Entity+0x5C` thresholds
//     0x321 / 0x191, a green/yellow/red health tier;
//   * the drawn pixels carry the sprite's own base-palette color inside the
//     56x32 rectangle at the projected feet;
//   * deselecting the cell (`0x81A814`) removes it and re-selecting restores it;
//   * the ellipse keeps that exact color at night (the original draws the ring
//     with the base palette, not the ambient `FUN_0043D971` version).
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
const pptr = wasm.alloc(pack.length);
new Uint8Array(mem.buffer, pptr, pack.length).set(pack);
if (wasm.load_pack(pptr, pack.length) !== 1) throw new Error('load_pack failed');
if (wasm.boot(0, -1) !== 1) throw new Error('boot failed');
wasm.on_resize(800, 600);
for (let i = 0; i < 5; i++) wasm.step(0.016);

function frame() {
  const p = wasm.frame_ptr();
  const fw = wasm.frame_w();
  const fh = wasm.frame_h();
  return { fw, fh, data: new Uint8Array(mem.buffer, p, fw * fh * 4) };
}

/** Most common color inside the 56x32 ellipse rect that satisfies `pred`. */
function dominantColor(rect, pred) {
  const { fw, data } = frame();
  const counts = new Map();
  for (let y = rect.y; y < rect.y + 32; y++) {
    for (let x = rect.x; x < rect.x + 56; x++) {
      if (x < 0 || y < 0 || x >= fw) continue;
      const o = (y * fw + x) * 4;
      const c = [data[o], data[o + 1], data[o + 2]];
      if (!pred(c)) continue;
      const key = c.join(',');
      counts.set(key, (counts.get(key) || 0) + 1);
    }
  }
  let best = null;
  for (const [key, n] of counts) {
    if (!best || n > best[3]) {
      const c = key.split(',').map(Number);
      best = [c[0], c[1], c[2], n];
    }
  }
  return best;
}

/** The ring colors are single solid palette entries (172 red, 126 yellow, 178 green). */
const ringColor = {
  172: (c) => c[0] >= 200 && c[1] <= 90 && c[2] <= 90,
  126: (c) => c[0] >= 200 && c[1] >= 180 && c[2] <= 90,
  178: (c) => c[1] >= 200 && c[0] <= 60 && c[2] <= 60,
};

function ring(i) {
  return {
    sprite: wasm.debug_selection_ring(i, 0),
    x: wasm.debug_selection_ring(i, 1),
    y: wasm.debug_selection_ring(i, 2),
  };
}

function check(label, hp, spriteId) {
  wasm.debug_set_player_hp(hp);
  wasm.step(0.016);
  const count = wasm.debug_selection_ring_count();
  if (count !== 1) throw new Error(`${label}: expected one ring, got ${count}`);
  const r = ring(0);
  if (r.sprite !== spriteId) {
    throw new Error(`${label}: sprite ${r.sprite}, expected ${spriteId}`);
  }
  const best = dominantColor(r, ringColor[spriteId]);
  if (!best || best[3] < 20) {
    throw new Error(`${label}: no ${spriteId} ellipse pixels at ${r.x},${r.y}`);
  }
  return { r, best };
}

const full = check('full health', 1600, 178);
console.log(`hp 1600 -> INTR[178] ${full.best.slice(0, 3)} (${full.best[3]} px) at ${full.r.x},${full.r.y}`);

const mid = check('mid health', 500, 126);
console.log(`hp 500  -> INTR[126] ${mid.best.slice(0, 3)} (${mid.best[3]} px)`);

const low = check('low health', 300, 172);
console.log(`hp 300  -> INTR[172] ${low.best.slice(0, 3)} (${low.best[3]} px)`);

// Deselect: the selection array drives the loop, so no ring is emitted.
if (wasm.debug_set_party_selected(0, 0) !== 1) throw new Error('could not deselect');
wasm.step(0.016);
if (wasm.debug_selection_ring_count() !== 0) {
  throw new Error('deselected leader still emits a ring');
}
if (dominantColor(low.r, ringColor[172]) !== null) {
  throw new Error('ring pixels survived deselection');
}
console.log('deselect -> no ring');

if (wasm.debug_set_party_selected(0, 1) !== 1) throw new Error('could not reselect');
wasm.step(0.016);
if (wasm.debug_selection_ring_count() !== 1) throw new Error('reselect did not restore the ring');

// Night phase: the ring uses the base palette, so its color is unchanged while
// the ambient delta (frame 12000 = -60/-70) shades the world.
wasm.debug_set_player_hp(1600);
wasm.debug_set_world_frame(12000, 0);
wasm.step(0.016);
if (wasm.debug_selection_ring_count() !== 1) throw new Error('ring missing at night');
const night = ring(0);
const nightBest = dominantColor(night, ringColor[178]);
if (!nightBest || nightBest[3] < 20) {
  throw new Error('night ring is not the unshaded base-palette color');
}
console.log(`night -> ring still unshaded ${nightBest.slice(0, 3)}`);

console.log('RING SMOKE OK');
