// Page-level smoke test: drives the *served* bundle exactly like the browser
// page does, with cache-busting, and proves which build produced the frame.
//
//   powershell -File web/serve.ps1      (or bash web/serve.sh)
//   node web/tools/page_smoke.mjs [baseUrl]   (default http://localhost:8000/)
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const base = (process.argv[2] || 'http://localhost:8000/').replace(/\/?$/, '/');

const KIND = { game: 0, graph: 1, objects: 2, exe: 3, names: 4, menu640: 5, menu800: 6, newh640: 7, newh800: 8 };

async function fetchBytes(url) {
  const r = await fetch(url, { cache: 'no-store' });
  if (!r.ok) throw new Error(url + ' -> HTTP ' + r.status);
  return new Uint8Array(await r.arrayBuffer());
}

function bust(path, build) {
  const url = base + path;
  return url + (url.includes('?') ? '&' : '?') + 'v=' + encodeURIComponent(build);
}

// 1) Bootstrap exactly like index.html.
const manifest = JSON.parse(new TextDecoder().decode(await fetchBytes(base + 'manifest.json')));
const build = manifest.build || 'dev';
console.log('build id =', build);
console.log('base     =', manifest.base || '/');

// 2) Load the wasm under the manifest build id.
const wasmBytes = await fetchBytes(bust('openkonung.wasm', build));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;
console.log('wasm bytes =', wasmBytes.length);

function load(buf, fn, id) {
  const ptr = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
  if (id === undefined) fn(ptr, buf.length);
  else fn(id, ptr, buf.length);
}

const res = manifest.resources;
load(await fetchBytes(bust(res.game, build)), (p, l) => wasm.load_resource(KIND.game, p, l));
load(await fetchBytes(bust(res.graph, build)), (p, l) => wasm.load_resource(KIND.graph, p, l));
load(await fetchBytes(bust(res.objects, build)), (p, l) => wasm.load_resource(KIND.objects, p, l));
load(await fetchBytes(bust(res.exe, build)), (p, l) => wasm.load_resource(KIND.exe, p, l));
if (res.names) load(await fetchBytes(bust(res.names, build)), (p, l) => wasm.load_resource(KIND.names, p, l));
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  load(await fetchBytes(bust(p, build)), (a, ptr, len) => wasm.load_profile(a, ptr, len), id);
}
for (const kon of manifest.konungs) {
  load(await fetchBytes(bust(kon.file, build)), (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}
for (const [r, slot, kind] of [
  ['640', 'menu', KIND.menu640], ['640', 'newh', KIND.newh640],
  ['800', 'menu', KIND.menu800], ['800', 'newh', KIND.newh800],
]) {
  const url = manifest.menu && manifest.menu[r] && manifest.menu[r][slot];
  if (url) load(await fetchBytes(bust(url, build)), (p, l) => wasm.load_resource(kind, p, l));
}

// 3) Boot the frontend at the 800 original layout and render one frame.
if (wasm.frontend_boot(800) !== 1) throw new Error('frontend_boot failed');
wasm.on_resize(800, 600);
wasm.step(0.016);

const fw = wasm.frame_w();
const fh = wasm.frame_h();
const ptr = wasm.frame_ptr();
if (!ptr) throw new Error('frame_ptr null');
const view = new Uint8Array(mem.buffer, ptr, fw * fh * 4);
let nonDark = 0;
for (let i = 0; i < view.length; i += 4) {
  if (view[i] > 20 || view[i + 1] > 20 || view[i + 2] > 20) nonDark++;
}
const pct = (100 * nonDark / (fw * fh)).toFixed(1);
console.log('booted; frame ' + fw + 'x' + fh, 'frontend_active=' + wasm.frontend_is_active(), 'state=' + wasm.frontend_state());
console.log('non-dark pixels =', nonDark, '(' + pct + '%)');
if (fw !== 800 || fh !== 600) throw new Error('expected an 800x600 menu framebuffer');
if (nonDark < fw * fh * 0.2) throw new Error('frame looks empty; wrong/stale bundle?');
console.log('PAGE SMOKE OK (build ' + build + ')');
