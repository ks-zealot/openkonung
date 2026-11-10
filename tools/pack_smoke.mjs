// Pack-only acceptance smoke test.
//
// Serves ONLY web/www over HTTP (no original .res/.exe/Game.N/Konung.N/music
// files are reachable), then boots the wasm runtime from manifest.json +
// openkonung.wasm + konung.zip.pack exactly like the browser shell.
//
// Proves:
//   * load_pack populates the loader and the pack build id matches the manifest,
//   * all 10 music tracks are present in wasm memory,
//   * the game boots and renders a non-empty frame,
//   * the only HTTP requests are the shell files and the pack.
//
//   node web/tools/pack_smoke.mjs
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join, normalize, sep } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const wwwDir = join(webDir, 'www');

const requests = [];

const server = http.createServer(async (req, res) => {
  const path = decodeURIComponent((req.url || '/').split('?')[0]);
  requests.push(path);
  const rel = path === '/' ? 'index.html' : path.replace(/^\/+/, '');
  const file = normalize(join(wwwDir, rel));
  if (!file.startsWith(wwwDir + sep) && file !== join(wwwDir, 'index.html')) {
    res.writeHead(403).end('forbidden');
    return;
  }
  try {
    const data = await readFile(file);
    res.writeHead(200, { 'Cache-Control': 'no-store' });
    res.end(data);
  } catch {
    res.writeHead(404).end('not found');
  }
});

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const port = server.address().port;
const base = `http://127.0.0.1:${port}/`;

function fail(message) {
  console.error('FAIL:', message);
  server.close();
  process.exit(1);
}

try {
  const manifest = JSON.parse(
    (await (await fetch(base + 'manifest.json')).text()).replace(/^\uFEFF/, ''),
  );
  const build = manifest.build || 'dev';
  if (!manifest.pack || !manifest.pack.file) fail('manifest has no pack entry');
  console.log('build =', build);
  console.log('pack  =', manifest.pack.file, manifest.pack.size, 'bytes');

  const wasmBytes = new Uint8Array(await (await fetch(base + 'openkonung.wasm')).arrayBuffer());
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const wasm = instance.exports;
  const mem = wasm.memory;
  console.log('wasm  =', wasmBytes.length, 'bytes');

  const packBytes = new Uint8Array(
    await (await fetch(base + manifest.pack.file)).arrayBuffer(),
  );
  console.log('fetch =', packBytes.length, 'bytes');
  if (packBytes.length !== manifest.pack.size) {
    fail(`pack size ${packBytes.length} != manifest ${manifest.pack.size}`);
  }

  const ptr = wasm.alloc(packBytes.length);
  new Uint8Array(mem.buffer, ptr, packBytes.length).set(packBytes);
  const loaded = wasm.load_pack(ptr, packBytes.length);
  if (loaded !== 1) fail('load_pack returned 0');
  if (wasm.debug_pack_loaded() !== 1) fail('debug_pack_loaded != 1');

  const buildLen = wasm.debug_pack_build_len();
  const buildPtr = wasm.debug_pack_build_ptr();
  const packBuild = new TextDecoder().decode(new Uint8Array(mem.buffer, buildPtr, buildLen));
  if (manifest.pack.build && packBuild !== manifest.pack.build) {
    fail(`pack build ${packBuild} != manifest ${manifest.pack.build}`);
  }
  console.log('loaded pack build =', packBuild);

  const music = wasm.pack_music_count();
  if (music !== 10) fail(`expected 10 music tracks, got ${music}`);
  for (let i = 0; i < music; i++) {
    const p = wasm.pack_music_ptr(i);
    const n = wasm.pack_music_len(i);
    if (!p || !n) fail(`music track ${i} empty`);
    const head = new TextDecoder().decode(new Uint8Array(mem.buffer, p, 4));
    if (head !== 'OggS') fail(`music track ${i} is not Ogg (head=${head})`);
    const np = wasm.pack_music_name_ptr(i);
    const nl = wasm.pack_music_name_len(i);
    const name = nl ? new TextDecoder().decode(new Uint8Array(mem.buffer, np, nl)) : `#${i}`;
    console.log(`  music[${i}] ${name} ${n} bytes`);
  }

  const t0 = Date.now();
  const ok = wasm.boot(0, -1);
  if (ok !== 1) fail('boot() failed');
  console.log('boot  = 1 in', Date.now() - t0, 'ms');
  wasm.on_resize(1600, 1200);
  for (let i = 0; i < 5; i++) wasm.step(0.016);

  const fptr = wasm.frame_ptr();
  const fw = wasm.frame_w();
  const fh = wasm.frame_h();
  const frame = new Uint8Array(mem.buffer, fptr, fw * fh * 4);
  let nonDark = 0;
  for (let i = 0; i < frame.length; i += 4) {
    if (frame[i] > 20 || frame[i + 1] > 20 || frame[i + 2] > 20) nonDark++;
  }
  console.log('frame =', fw + 'x' + fh, 'nonDark=' + nonDark);
  if (nonDark < 1000) fail(`frame looks empty (nonDark=${nonDark})`);

  const allowed = new Set([
    '/manifest.json',
    '/openkonung.wasm',
    '/' + manifest.pack.file,
  ]);
  const bad = requests.filter((p) => !allowed.has(p));
  if (bad.length) fail('unexpected requests: ' + bad.join(', '));
  if (!requests.includes('/' + manifest.pack.file)) fail('pack was not fetched');
  if (requests.some((p) => /\.(res|exe)$/i.test(p))) fail('a .res/.exe was fetched');
  if (requests.some((p) => /\/music\//i.test(p))) fail('external music was fetched');

  console.log('PASS: pack-only boot, no original resource downloads');
  server.close();
  process.exit(0);
} catch (e) {
  fail(e && e.stack ? e.stack : String(e));
}
