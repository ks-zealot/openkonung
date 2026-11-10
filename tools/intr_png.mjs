// Diagnostic: decode original INTR interface-image archives and render selected
// sprites to PNG for visual inspection. Uses the exact runtime decoder via the
// wasm ABI; PNG is diagnostic output only, never a runtime input.
//
//   node web/tools/intr_png.mjs
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const outDir = join(here, 'intr_dump');
await mkdir(outDir, { recursive: true });

const KIND_GRAPH = 1;
const KIND_INTR640 = 11;
const KIND_INTR800 = 12;

// Reference sprites identified from the original game (see task §6).
const REFERENCE = [0, 1, 2, 3, 17, 71, 150, 240];

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

async function loadFile(file, fn, kind) {
  const buf = await readFile(join(baseDir, file));
  const ptr = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
  fn(kind, ptr, buf.length);
}

// The INTR decoder needs GRAPH.RES palettes.
await loadFile(manifest.resources.graph, (k, p, l) => wasm.load_resource(k, p, l), KIND_GRAPH);

const intr = manifest.intr || {};
const variants = [];
if (intr['800'] || manifest.resources.intr800) {
  const file = intr['800'] || manifest.resources.intr800;
  await loadFile(file, (k, p, l) => wasm.load_resource(k, p, l), KIND_INTR800);
  variants.push({ label: '800', width: 800, variant: 800 });
}
if (intr['640'] || manifest.resources.intr640) {
  const file = intr['640'] || manifest.resources.intr640;
  await loadFile(file, (k, p, l) => wasm.load_resource(k, p, l), KIND_INTR640);
  variants.push({ label: '640', width: 640, variant: 640 });
}
if (variants.length === 0) throw new Error('no INTR archives found in manifest');

for (const v of variants) {
  for (const id of REFERENCE) {
    if (!wasm.debug_decode_intr_image(v.variant, id)) {
      console.log(`  INTR${v.label}[${id}]: (not present)`);
      continue;
    }
    const w = wasm.debug_image_w();
    const h = wasm.debug_image_h();
    const rgba = Buffer.from(new Uint8Array(mem.buffer, wasm.debug_image_ptr(), w * h * 4));
    const name = `intr${v.label}_${String(id).padStart(3, '0')}.png`;
    await writeFile(join(outDir, name), encodePng(w, h, rgba));
    console.log(`  INTR${v.label}[${id}] = ${w}x${h} -> ${name}`);
  }
}

console.log('wrote INTR PNGs to', outDir);
console.log('Check: INTR800[3] should be the left party panel HUD; INTR800[17] the inventory-slot frame.');
