// Headless movie-trigger and packed-video smoke.
//
//   node web/tools/video_smoke.mjs
//
// Verifies the transcoded assets/manifest, the packed MP4 payloads, and the
// Rust-side request/finish handshake for the intro and game triggers.
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

function expect(cond, msg) {
  if (!cond) throw new Error('FAIL: ' + msg);
}
function decoder(bytes) {
  return new TextDecoder().decode(bytes);
}
function videoName() {
  const len = wasm.video_name_len() >>> 0;
  if (!len) return '';
  return decoder(new Uint8Array(mem.buffer, wasm.video_name_ptr(), len));
}

expect(manifest.videos && Object.keys(manifest.videos).length === 19, 'manifest lists 19 videos');
for (const [name, item] of Object.entries(manifest.videos)) {
  expect(typeof item.file === 'string' && item.file.endsWith('.mp4'), name + ' fallback URL');
  const bytes = await readFile(join(webDir, 'www', item.file));
  expect(bytes.length > 1000, name + ' non-empty fallback');
  expect(bytes.subarray(4, 8).toString('ascii') === 'ftyp', name + ' is MP4');
}

const pack = await readFile(join(webDir, 'www', manifest.pack.file));
const ptr = wasm.alloc(pack.length);
new Uint8Array(mem.buffer, ptr, pack.length).set(pack);
expect(wasm.load_pack(ptr, pack.length) === 1, 'load_pack');
expect(wasm.pack_video_count() === 19, 'packed video count 19');

const packed = [];
for (let i = 0; i < wasm.pack_video_count(); i++) {
  const np = wasm.pack_video_name_ptr(i);
  const nl = wasm.pack_video_name_len(i);
  const name = decoder(new Uint8Array(mem.buffer, np, nl));
  const p = wasm.pack_video_ptr(i);
  const len = wasm.pack_video_len(i) >>> 0;
  expect(len > 1000, name + ' packed size');
  expect(decoder(new Uint8Array(mem.buffer, p + 4, 4)) === 'ftyp', name + ' packed MP4');
  packed.push(name);
}
for (const name of ['poster', 'win', 'gameover', 'credits', 'defeat7', 'defeat39']) {
  expect(packed.includes(name), 'packed ' + name);
}

expect(wasm.frontend_boot(0) === 1, 'frontend_boot');
wasm.debug_video_reset();
expect(wasm.start_intro_video() === 1, 'start intro');
expect(wasm.video_pending() === 1, 'intro pending');
expect(videoName() === 'poster.mp4', 'intro clip name');
expect(wasm.video_finish() === 1, 'finish intro');
expect(wasm.video_pending() === 0, 'intro cleared');
expect(wasm.frontend_state() === 0, 'intro lands on menu');

// Debug trigger selectors mirror the four game paths.
for (const [kind, loc, expected] of [
  [1, 0, 'win.mp4'],
  [2, 0, 'gameover.mp4'],
  [3, 7, 'defeat7.mp4'],
  [4, 0, 'credits.mp4'],
]) {
  expect(wasm.debug_queue_video(kind, loc, 0) === 1, 'queue ' + expected);
  expect(videoName() === expected, 'clip name ' + expected);
  expect(wasm.video_finish() === 1, 'finish ' + expected);
}

console.log('PASS: 19 transcoded movies, packed/fallback MP4s, intro + trigger handshake');
