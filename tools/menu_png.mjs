// Diagnostic: decode original menu RES images and render frontend frames to PNG.
// Uses the exact runtime decoder (via the wasm ABI); PNG is diagnostic output
// only, never a runtime input.
//
//   node web/tools/menu_png.mjs
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const outDir = join(here, 'menu_dump');
await mkdir(outDir, { recursive: true });

const KIND = {
  menu640: 5,
  menu800: 6,
  newh640: 7,
  newh800: 8,
  save640: 9,
  save800: 10,
};

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
  const t = Buffer.from(type, 'ascii');
  const body = Buffer.concat([t, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

function encodePng(w, h, rgba) {
  const sig = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const stride = w * 4;
  const raw = Buffer.alloc((stride + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride);
  }
  const idat = deflateSync(raw, { level: 6 });
  return Buffer.concat([sig, chunk('IHDR', ihdr), chunk('IDAT', idat), chunk('IEND', Buffer.alloc(0))]);
}

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

async function loadFile(file, fn, id) {
  const buf = await readFile(join(baseDir, file));
  const ptr = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
  if (id === undefined) fn(ptr, buf.length);
  else fn(id, ptr, buf.length);
}

const menu = manifest.menu || {};
for (const [res, slot, kind] of [
  ['640', 'menu', KIND.menu640], ['640', 'newh', KIND.newh640], ['640', 'save', KIND.save640],
  ['800', 'menu', KIND.menu800], ['800', 'newh', KIND.newh800], ['800', 'save', KIND.save800],
]) {
  const url = menu[res] && menu[res][slot];
  if (url) await loadFile(url, (p, l) => wasm.load_resource(kind, p, l));
}
// konung.res carries the original bitmap fonts used by the New-Hero screen.
if (manifest.resources && manifest.resources.names) {
  await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
}
// Core resources are needed for the New-Hero paper-doll preview (the runtime
// composes the 160x160 frames once at frontend_boot).
for (const [kind, key] of [[0, 'game'], [1, 'graph'], [2, 'objects'], [3, 'exe']]) {
  const file = manifest.resources && manifest.resources[key];
  if (file) await loadFile(file, (p, l) => wasm.load_resource(kind, p, l));
}

async function saveRgba(name, ptr, w, h) {
  const view = Buffer.from(new Uint8Array(mem.buffer, ptr, w * h * 4));
  await writeFile(join(outDir, name), encodePng(w, h, view));
}

async function saveFrame(name) {
  const ptr = wasm.frame_ptr();
  if (!ptr) return;
  await saveRgba(name, ptr, wasm.frame_w(), wasm.frame_h());
}

// 1) Decode-dump every image of each family.
for (const [label, kind, count] of [
  ['menu640', KIND.menu640, 15], ['newh640', KIND.newh640, 13], ['save640', KIND.save640, 2],
  ['menu800', KIND.menu800, 15], ['newh800', KIND.newh800, 13], ['save800', KIND.save800, 2],
]) {
  for (let i = 0; i < count; i++) {
    if (!wasm.debug_decode_menu_image(kind, i)) break;
    await saveRgba(
      `${label}_${String(i).padStart(3, '0')}.png`,
      wasm.debug_image_ptr(),
      wasm.debug_image_w(),
      wasm.debug_image_h(),
    );
  }
  console.log('decoded', label, count, 'images');
}

// 2) Render frontend states.
async function renderFrontend(width, height) {
  if (!wasm.frontend_boot(width)) throw new Error('frontend_boot failed');
  wasm.on_resize(width, height);
  wasm.step(0.016);
  await saveFrame(`frontend_${width}_mainmenu.png`);

  // Click New Game (rect depends on resolution).
  const newGame = width === 640 ? [224, 241] : [280, 303];
  wasm.pointer_down(1, newGame[0] + 5, newGame[1] + 5);
  wasm.pointer_up(1, newGame[0] + 5, newGame[1] + 5);
  wasm.step(0.016);
  await saveFrame(`frontend_${width}_newhero.png`);

  // Hover + select hero 1.
  const hero1 = width === 640 ? [137, 94] : [155, 108];
  wasm.pointer_move(hero1[0] + 5, hero1[1] + 5);
  wasm.pointer_down(1, hero1[0] + 5, hero1[1] + 5);
  wasm.pointer_up(1, hero1[0] + 5, hero1[1] + 5);
  wasm.step(0.016);
  await saveFrame(`frontend_${width}_newhero_hero1.png`);

  // Original class marker + small-font values for each class (hit-rect centers).
  const classBoxes = width === 640
    ? { Merchant: [481, 68], Hunter: [362, 68], Chief: [362, 36], Warrior: [481, 36] }
    : { Merchant: [582, 81], Hunter: [464, 81], Chief: [464, 49], Warrior: [582, 49] };
  for (const [name, box] of Object.entries(classBoxes)) {
    wasm.pointer_down(1, box[0] + 5, box[1] + 5);
    wasm.pointer_up(1, box[0] + 5, box[1] + 5);
    wasm.step(0.016);
    await saveFrame(`frontend_${width}_class_${name}.png`);
  }

  // Debug hitbox overlay (K).
  wasm.key_down(75);
  wasm.step(0.016);
  await saveFrame(`frontend_${width}_newhero_debug.png`);
  wasm.key_down(75); // debug off

  // Back to the menu, then the original Settings screen (MENU #1).
  wasm.key_down(27);
  const settings = width === 640 ? [215, 282] : [268, 353];
  wasm.pointer_down(1, settings[0] + 5, settings[1] + 5);
  wasm.pointer_up(1, settings[0] + 5, settings[1] + 5);
  wasm.step(0.016);
  await saveFrame(`frontend_${width}_settings.png`);
}

await renderFrontend(800, 600);
await renderFrontend(640, 480);
console.log('wrote frontend PNGs to', outDir);
