// Headless autonomous mob-AI smoke:
// group activity 1 -> target acquisition -> row-5 attack -> player damage.
//
//   node web/tools/mob_ai_smoke.mjs
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');

const KIND = { game: 0, graph: 1, objects: 2, exe: 3, names: 4 };
const manifest = JSON.parse(
  (await readFile(join(webDir, 'www', 'manifest.json'), 'utf8')).replace(/^\uFEFF/, ''),
);
const wasmBytes = await readFile(join(webDir, 'www', 'openkonung.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const wasm = instance.exports;
const mem = wasm.memory;

function loadFile(file, fn, id) {
  return readFile(join(baseDir, file)).then((buf) => {
    const ptr = wasm.alloc(buf.length);
    new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
    if (id === undefined) fn(ptr, buf.length);
    else fn(id, ptr, buf.length);
  });
}

await loadFile(manifest.resources.game, (p, l) => wasm.load_resource(KIND.game, p, l));
await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(KIND.graph, p, l));
await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(KIND.objects, p, l));
await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(KIND.exe, p, l));
await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(KIND.names, p, l));
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (id2, ptr, len) => wasm.load_profile(id2, ptr, len), id);
}
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}

if (wasm.boot(0, -1) !== 1) {
  console.error('boot failed');
  process.exit(1);
}
wasm.on_resize(1600, 1200);
wasm.debug_combat_set_seed(1);

function report() {
  if (wasm.debug_combat_state() !== 1) return {};
  const ptr = wasm.debug_combat_state_ptr();
  const len = wasm.debug_combat_state_len();
  const text = new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len));
  const out = {};
  for (const part of text.trim().split(/\s+/)) {
    const eq = part.indexOf('=');
    if (eq > 0) out[part.slice(0, eq)] = part.slice(eq + 1);
  }
  return out;
}

function fail(msg) {
  console.error('FAIL:', msg);
  process.exit(1);
}

const started = wasm.debug_ai_setup();
if (started === 0) fail('AI staging found no object-family actor');
const actor = started - 1;
const group = wasm.debug_world_group(actor);
const activity = wasm.debug_group_activity(group);
console.log('staged', { actor, group, activity });
const before = report();
if (Number(before.player_hp) <= 0 || before.player_dead !== '0') {
  fail('player was not alive at AI staging');
}

// The group was explicitly set to activity 1; the first combat tick must
// acquire the adjacent leader without a player attack order.
wasm.step(0.07);
const acquired = wasm.debug_world_target(actor);
console.log('post activity', wasm.debug_group_activity(group), 'target', acquired);
console.log('after one tick', { actor, target: acquired, state: report() });
if (acquired !== 0) fail(`actor did not acquire the leader (target=${acquired})`);

let damaged = false;
let state = report();
for (let i = 0; i < 80; i++) {
  wasm.step(0.07);
  state = report();
  if (Number(state.player_hp) < Number(before.player_hp) || state.player_dead === '1') {
    damaged = true;
    break;
  }
}
if (!damaged) fail('autonomous row-5 attack did not damage the leader');
const action = Number(state.target_action);
console.log('attacking', { player_hp: state.player_hp, target_action: action });
if (action !== 5) fail(`world actor did not use melee row 5 (got ${action})`);

console.log('MOB AI SMOKE OK');
