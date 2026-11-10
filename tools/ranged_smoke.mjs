// Headless ranged-combat smoke test for the OpenKonung wasm ABI.
// Covers arrow flight, ammo decrement, bow wear, hit/dodge, kill rewards,
// corpse arrow-stack loot and the prop settlement target branch.
//
//   node web/tools/ranged_smoke.mjs
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
const { instance } = await WebAssembly.instantiate(
  await readFile(join(webDir, 'www', 'openkonung.wasm')),
  {},
);
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

function report() {
  if (wasm.debug_ranged_state() !== 1) return {};
  const ptr = wasm.debug_ranged_state_ptr();
  const len = wasm.debug_ranged_state_len();
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

function stepTicks(n) {
  for (let i = 0; i < n; i++) wasm.step(0.06);
}

function waitFor(fn, maxTicks) {
  for (let i = 0; i < maxTicks; i++) {
    wasm.step(0.06);
    if (fn(report())) return i + 1;
  }
  return 0;
}

// --- 1. normal shot: spawn, flight, kill reward, bow wear ------------------
wasm.debug_combat_set_seed(1);
if (wasm.debug_ranged_setup() === 0) fail('ranged setup failed');
const before = report();
console.log('ranged before', before);
if (Number(before.ammo_qty) !== 30) fail('ammo was not staged to 30');
if (Number(before.bow_durability) !== 56) fail('bow durability was not staged to 56');

let sawProjectile = false;
let spawned = null;
for (let i = 0; i < 40; i++) {
  wasm.step(0.06);
  const r = report();
  if (Number(r.projectiles) > 0) {
    sawProjectile = true;
    spawned = r;
    break;
  }
}
if (!sawProjectile) fail('no projectile spawned');
console.log('spawned', spawned);
if (!(Number(spawned.ammo_qty) < 30)) fail('ammo did not decrement at spawn');
wasm.step(0.06);
const moved = report();
if (
  Number(moved.projectiles) > 0 &&
  Number(moved.proj0_x) === Number(spawned.proj0_x) &&
  Number(moved.proj0_y) === Number(spawned.proj0_y)
) {
  fail('projectile did not move');
}

if (!waitFor((r) => Number(r.target_dead) === 1, 220)) fail('target did not die');
const killed = report();
console.log('killed', killed);
if (!(Number(killed.player_archery) > Number(before.player_archery))) {
  fail('archery reward not applied');
}
if (!(Number(killed.bow_durability) < Number(before.bow_durability))) {
  fail('bow durability did not decrease');
}
if (!(Number(killed.projectiles) === 0)) fail('projectile was not removed on hit');
const lootBeforePickup = Number(killed.projectiles);
if (wasm.debug_combat_collect() !== 1) fail('corpse pickup failed');

// --- 2. corpse contains the victim's authored 30-arrow stack ----------------
wasm.debug_combat_set_seed(2);
if (wasm.debug_ranged_setup() === 0) fail('second ranged setup failed');
if (wasm.debug_ranged_target_arrows() !== 1) fail('could not stage victim arrows');
const invBefore = Number(report().inv_arrow_qty);
if (!waitFor((r) => Number(r.target_dead) === 1, 220)) fail('arrow-carrier did not die');
if (wasm.debug_combat_collect() !== 1) fail('arrow-carrier pickup failed');
const afterLoot = report();
console.log('arrow loot', afterLoot);
if (!(Number(afterLoot.inv_arrow_qty) >= invBefore + 30)) {
  fail('victim 30-arrow stack was not looted');
}

// --- 3. dodge: arrow consumed, no HP loss ----------------------------------
wasm.debug_combat_set_seed(3);
if (wasm.debug_ranged_setup() === 0) fail('dodge setup failed');
if (wasm.debug_ranged_make_dodge() !== 1) fail('could not set dodge target');
const dodgeBefore = report();
if (!waitFor((r) => Number(r.projectiles) > 0, 40)) fail('dodge shot never spawned');
if (!waitFor((r) => Number(r.projectiles) === 0, 80)) fail('dodge shot did not expire');
const dodged = report();
console.log('dodged', dodged);
if (Number(dodged.target_hp) !== Number(dodgeBefore.target_hp)) {
  fail('dodged arrow removed HP');
}
if (!(Number(dodged.ammo_qty) < Number(dodgeBefore.ammo_qty))) {
  fail('dodged shot did not consume ammo');
}

// --- 4. prop target: blocked-cell pixel hit reaches the settlement branch ---
let propOk = false;
let propReport = null;
let propSetups = 0;
for (let seed = 1; seed <= 60 && !propOk; seed++) {
  wasm.debug_combat_set_seed(seed);
  const setup = wasm.debug_prop_setup(33);
  if (setup === 0) continue;
  propSetups++;
  if (seed === 1) console.log('prop setup ok', report());
  const start = report();
  if ((Number(start.prop_flags) & 0x15) === 0x15) {
    propOk = true;
    propReport = start;
    break;
  }
  for (let i = 0; i < 40; i++) {
    wasm.step(0.06);
    const r = report();
    if ((Number(r.prop_flags) & 0x15) === 0x15) {
      propOk = true;
      propReport = r;
      break;
    }
    if (Number(r.projectiles) === 0 && i > 4) break;
  }
}
if (!propOk) fail(`prop settlement target branch never fired (setups=${propSetups})`);
console.log('prop', propReport);

console.log('RANGED SMOKE OK');
