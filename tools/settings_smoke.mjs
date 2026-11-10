// Headless KONUNG.CFG + Settings smoke.
//
//   node web/tools/settings_smoke.mjs
//
// Proves the original 28-byte configuration round-trips through the wasm ABI,
// `frontend_boot(0)` honours the stored video mode, the exact 0x46257C
// settings rectangles toggle live values, Ok persists and Cancel restores.
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');

const KIND = {
  game: 0, graph: 1, objects: 2, exe: 3, names: 4,
  menu640: 5, menu800: 6, newh640: 7, newh800: 8, save640: 9, save800: 10,
};

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

function expect(cond, msg) { if (!cond) throw new Error('FAIL: ' + msg); }

function loadFile(file, fn, id) {
  return readFile(join(baseDir, file)).then((buf) => {
    const ptr = wasm.alloc(buf.length);
    new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
    if (id === undefined) fn(ptr, buf.length);
    else fn(id, ptr, buf.length);
  });
}

function click(x, y) {
  wasm.pointer_down(1, x, y);
  wasm.pointer_up(1, x, y);
  wasm.step(0.016);
}

function cfgBytes(video = 0x111, transparency = 1) {
  const out = new Uint8Array(28);
  const dv = new DataView(out.buffer);
  [video, transparency, 1, 1, 1, 1, 1].forEach((v, i) => dv.setUint32(i * 4, v, true));
  return out;
}
function loadCfg(bytes) {
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(mem.buffer, ptr, bytes.length).set(bytes);
  return wasm.config_load(ptr, bytes.length);
}
function writeCfg() {
  const ptr = wasm.alloc(28);
  const n = wasm.config_write(ptr, 28);
  const out = new Uint8Array(mem.buffer, ptr, n).slice();
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, 28);
  return out;
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
for (const [res, slot, kind] of [
  ['640', 'menu', KIND.menu640], ['640', 'newh', KIND.newh640], ['640', 'save', KIND.save640],
  ['800', 'menu', KIND.menu800], ['800', 'newh', KIND.newh800], ['800', 'save', KIND.save800],
]) {
  const url = manifest.menu[res] && manifest.menu[res][slot];
  if (url) await loadFile(url, (p, l) => wasm.load_resource(kind, p, l));
}

// 640 configuration, 640x480 frontend, then open Settings (main-menu action 4).
expect(loadCfg(cfgBytes(0x111, 1)) === 1, 'config_load');
expect(wasm.frontend_boot(0) === 1, 'frontend_boot(0)');
wasm.on_resize(640, 480);
wasm.step(0.016);
expect(wasm.frame_w() === 640 && wasm.frame_h() === 480, 'config video mode 0x111');
expect(wasm.frontend_state() === 0, 'main menu');
click(216, 283); // MAIN_MENU Settings rect (215,282,118,39)
expect(wasm.frontend_state() === 2, 'settings open');

// Toggle sound off (640 rect 413,342,31,31).
click(428, 357);
expect((wasm.debug_config_word(6) >>> 0) === 0, 'sound toggled off');
const revBefore = wasm.config_revision() >>> 0;
expect(revBefore > 0, 'config revision bumped');

// Escape cancels the live toggle.
wasm.key_down(27);
wasm.step(0.016);
expect((wasm.debug_config_word(6) >>> 0) === 1, 'escape restores sound');

// Re-open, disable transparency, then Ok persists (640 Cancel/Ok at y=450).
click(216, 283);
click(428, 181); // transparency row (413,166,31,31)
click(285, 450); // OK (250,420,70,61)
expect(wasm.frontend_state() === 0, 'Ok returns to menu');
const persisted = writeCfg();
const dv = new DataView(persisted.buffer);
expect(dv.getUint32(0, true) === 0x111, 'persisted video mode 0x111');
expect(dv.getUint32(4, true) === 0, 'persisted transparency off');
expect(persisted.length === 28, 'persisted exactly 28 bytes');

// Cancel restores the snapshot taken on entry (transparency still off).
click(216, 283);
click(428, 181);
click(355, 450); // Cancel (320,420,71,61)
expect(wasm.frontend_state() === 0, 'Cancel returns to menu');
expect((wasm.debug_config_word(1) >>> 0) === 0, 'cancel restores snapshot');

// A new config payload is accepted and reflected by frontend_boot.
expect(loadCfg(cfgBytes(0x114, 1)) === 1, 'reload 800 config');
expect(wasm.frontend_boot(800) === 1, 'explicit frontend_boot(800)');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, '800x600 frontend');

console.log('PASS: KONUNG.CFG 28-byte round-trip, settings toggles, Ok/Cancel, video mode');
