// Diagnostic: decode the original 11 GRAPH.RES mouse-cursor frames and render
// each to PNG (16-bit color, transparent via the RLE skip run). Uses the exact
// runtime decoder via the wasm ABI; PNG is diagnostic output only.
//
//   node web/tools/cursor_png.mjs
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const outDir = join(here, 'cursor_dump');
await mkdir(outDir, { recursive: true });

const KIND_GRAPH = 1;

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

// Compose over a magenta background so transparent pixels are visible.
function encodePng(w, h, rgba) {
  const sig = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6;
  const stride = w * 4;
  const raw = Buffer.alloc((stride + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride);
  }
  return Buffer.concat([
    sig, chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw, { level: 6 })), chunk('IEND', Buffer.alloc(0)),
  ]);
}

const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

const buf = await readFile(join(baseDir, manifest.resources.graph));
const ptr = wasm.alloc(buf.length);
new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
wasm.load_resource(KIND_GRAPH, ptr, buf.length);

const count = wasm.debug_cursor_count();
console.log('original cursor frames:', count);
for (let i = 0; i < count; i++) {
  if (!wasm.debug_decode_cursor_image(i)) {
    console.log(`  cursor[${i}]: (not present)`);
    continue;
  }
  const w = wasm.debug_image_w();
  const h = wasm.debug_image_h();
  const raw = Buffer.from(new Uint8Array(mem.buffer, wasm.debug_image_ptr(), w * h * 4));
  // Composite over magenta.
  const rgba = Buffer.alloc(w * h * 4);
  for (let p = 0; p < w * h; p++) {
    const a = raw[p * 4 + 3];
    if (a === 0) {
      rgba[p * 4] = 255; rgba[p * 4 + 1] = 0; rgba[p * 4 + 2] = 255; rgba[p * 4 + 3] = 255;
    } else {
      rgba[p * 4] = raw[p * 4]; rgba[p * 4 + 1] = raw[p * 4 + 1]; rgba[p * 4 + 2] = raw[p * 4 + 2]; rgba[p * 4 + 3] = 255;
    }
  }
  const name = `cursor_${String(i).padStart(2, '0')}.png`;
  await writeFile(join(outDir, name), encodePng(w, h, rgba));
  console.log(`  cursor[${i}] = ${w}x${h} -> ${name}`);
}
console.log('wrote cursor PNGs to', outDir);
