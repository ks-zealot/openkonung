// Headless melee combat smoke test for the OpenKonung wasm ABI.
// Exercises attack order -> attack/death animation -> HP loss -> corpse loot
// -> pickup in one deterministic run (seeded original LCG).
//
//   node web/tools/combat_smoke.mjs
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

const ok = wasm.boot(0, -1);
if (ok !== 1) {
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

const started = wasm.debug_combat_setup();
if (started === 0) {
  console.error('combat setup failed (no object-family actor beside the player)');
  process.exit(1);
}
const before = report();
console.log('before', before);
if (Number(before.target_hp) !== 120) {
  console.error('target HP was not staged to 120');
  process.exit(1);
}
if (Number(before.player_attack_frames) === 0 && Number(before.player_attack_frame) === 0) {
  // The cycle length resolves on the first adjacent combat tick.
}

let sawAttack = false;
let after = before;
for (let i = 0; i < 240; i++) {
  wasm.step(0.06);
  after = report();
  if (Number(after.player_attack_frames) > 1) sawAttack = true;
  if (Number(after.target_dead) === 1) break;
}
console.log('after', after);

function fail(msg) {
  console.error('FAIL:', msg);
  process.exit(1);
}
if (!sawAttack) fail('attack animation cycle never started');
// `FUN_0041EB48`/`FUN_004123BC` case 5: melee is action row 5. For every
// shipped human base/direction that is a 12-frame cycle (720 ms at 60 ms).
if (Number(after.player_attack_frames) !== 12) {
  fail(`melee row 5 cycle length should be 12 frames, got ${after.player_attack_frames}`);
}
if (Number(after.target_dead) !== 1) fail('test target did not die');
const action = Number(after.target_action);
if (action !== 3 && action !== 9) fail(`death action row not 3/9 (got ${action})`);
if (Number(after.loot) < Number(before.loot) + 1) fail('no corpse loot marker spawned');
if (!(Number(after.weapon_durability) < Number(before.weapon_durability))) {
  fail('weapon durability did not decrease');
}
if (Number(after.player_dead) === 1) fail('protagonist died during the deterministic dummy fight');

const inventoryBefore = Number(after.inventory);
const moneyBefore = Number(after.money);
if (wasm.debug_combat_collect() !== 1) fail('corpse pickup did not run');
const picked = report();
console.log('picked', picked);
if (!(Number(picked.inventory) > inventoryBefore || Number(picked.money) > moneyBefore)) {
  fail('corpse money/inventory were not transferred');
}
if (Number(picked.loot) >= Number(after.loot)) fail('corpse marker was not consumed');

if (wasm.debug_combat_kill_player() !== 1) fail('player death transition failed');
for (let i = 0; i < 20; i++) wasm.step(0.06);
const deadPlayer = report();
console.log('dead player', deadPlayer);
if (Number(deadPlayer.player_dead) !== 1) fail('player dead flag not set');
if (Number(deadPlayer.player_hp) !== 0) fail('player HP not zero');

console.log('COMBAT SMOKE OK');
