// Diagnostic: decode **every** occupied INTR interface sprite (0..=274) from
// both resolution variants to PNGs, with dimensions and the verified role.
//
//   node web/tools/intr_sheet.mjs
//
// Uses the runtime decoder via the wasm ABI (`debug_decode_intr_image`,
// `debug_intr_role`). PNG output is diagnostic only.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const outDir = join(here, 'intr_dump', 'all');
await mkdir(outDir, { recursive: true });

const ROLE = [
  'HudOverlay', 'CharacterWindow', 'InventorySlot', 'InventoryArrow', 'AutoButton',
  'Portrait', 'ItemIcon', 'WindowFrame', 'Panel', 'Button', 'InterfaceControl',
  'SmallUi', 'Unclassified', 'SelectionMarker', 'HealthBar',
];

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

await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(1, p, l));
const intr = manifest.intr || {};
await loadFile(intr['640'], (k, p, l) => wasm.load_resource(k, p, l), 11);
await loadFile(intr['800'], (k, p, l) => wasm.load_resource(k, p, l), 12);

const rows = [];
for (const variant of [800, 640]) {
  let count = 0;
  for (let id = 0; id <= 274; id++) {
    if (!wasm.debug_decode_intr_image(variant, id)) {
      rows.push([variant, id, '-', '-', 'MISSING']);
      continue;
    }
    const w = wasm.debug_image_w();
    const h = wasm.debug_image_h();
    const role = ROLE[wasm.debug_intr_role(id)] || 'Unclassified';
    if (variant === 800) rows.push([variant, id, w, h, role]);
    const rgba = Buffer.from(new Uint8Array(mem.buffer, wasm.debug_image_ptr(), w * h * 4));
    await writeFile(join(outDir, `intr${variant}_${String(id).padStart(3, '0')}.png`), encodePng(w, h, rgba));
    count++;
  }
  console.log(`decoded ${count} sprites for ${variant}`);
}

console.log('\nidx | 800 w x h | role');
for (const [, id, w, h, role] of rows) {
  console.log(`${String(id).padStart(3)} | ${w}x${h} | ${role}`);
}
console.log('\nwrote INTR contact sheet to', outDir);
