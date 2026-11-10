// Player-feedback visual/regression smoke.
//
// Covers the eight reported issues against the current wasm build:
//   1 hotkeys, 2 item tooltip, 3 attack animation, 4 merchant stock,
//   5 cursor, 6 sound queue, 7 engage/retaliation, 8 monster aggro.
//
// Writes screenshots to web/tools/feedback/ for manual review.
//
//   node web/tools/feedback_smoke.mjs
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');
const outDir = join(here, 'feedback');
await mkdir(outDir, { recursive: true });

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
function encodePng(w, h, rgba) {
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
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
    sig,
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 6 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

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

await loadFile(manifest.resources.game, (p, l) => wasm.load_resource(0, p, l));
await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(1, p, l));
await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(2, p, l));
await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(3, p, l));
await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
if (manifest.sounds) await loadFile(manifest.sounds, (p, l) => wasm.load_resource(14, p, l));
if (manifest.quest) await loadFile(manifest.quest, (p, l) => wasm.load_resource(13, p, l));
if (manifest.intr?.['640']) await loadFile(manifest.intr['640'], (p, l) => wasm.load_resource(11, p, l));
if (manifest.intr?.['800']) await loadFile(manifest.intr['800'], (p, l) => wasm.load_resource(12, p, l));
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (a, ptr, len) => wasm.load_profile(a, ptr, len), id);
}
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}

function fail(msg) {
  console.error('FAIL:', msg);
  process.exit(1);
}
function boot() {
  const ok = wasm.boot(0, -1);
  if (ok !== 1) fail(`boot failed (${ok})`);
  wasm.on_resize(800, 600);
  wasm.step(0.06);
}
function steps(n, dt = 0.06) {
  for (let i = 0; i < n; i++) wasm.step(dt);
}
function save(name) {
  const w = wasm.frame_w();
  const h = wasm.frame_h();
  const ptr = wasm.frame_ptr();
  const rgba = Buffer.from(new Uint8Array(mem.buffer, ptr, w * h * 4));
  return writeFile(join(outDir, name), encodePng(w, h, rgba));
}
function hashRegion(cx, cy, w, h) {
  const fw = wasm.frame_w();
  const fh = wasm.frame_h();
  const ptr = wasm.frame_ptr();
  const v = new Uint8Array(mem.buffer, ptr, fw * fh * 4);
  let acc = 2166136261;
  for (let y = cy; y < cy + h; y++) {
    for (let x = cx; x < cx + w; x++) {
      const i = (y * fw + x) * 4;
      acc ^= v[i]; acc = Math.imul(acc, 16777619);
      acc ^= v[i + 1]; acc = Math.imul(acc, 16777619);
      acc ^= v[i + 2]; acc = Math.imul(acc, 16777619);
    }
  }
  return (acc >>> 0).toString(16);
}
function combatState() {
  if (wasm.debug_combat_state() !== 1) return null;
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
function playerHp() {
  const s = combatState();
  return s ? Number(s.player_hp) : null;
}

// ---------------------------------------------------------------- 1 hotkeys
boot();
wasm.key_down(77);
if (wasm.debug_map_open() !== 1) fail('#1 M did not open the world map');
wasm.key_down(77);
if (wasm.debug_map_open() !== 0) fail('#1 M did not close the world map');
wasm.key_down(81);
if (wasm.debug_quest_open() !== 1) fail('#1 Q did not open the quest log');
wasm.key_down(81);
if (wasm.debug_quest_open() !== 0) fail('#1 Q did not close the quest log');
wasm.key_down(73);
if (wasm.debug_character_screen() !== 1) fail('#1 I did not open the character screen');
wasm.key_down(27);
if (wasm.debug_character_screen() !== 0) fail('#1 Escape did not close the character screen');
wasm.key_down(49);
if (wasm.debug_player_weapon_slot() !== 2) fail('#1 key 1 did not select weapon slot 2');
wasm.key_down(50);
if (wasm.debug_player_weapon_slot() !== 0) fail('#1 key 2 did not select weapon slot 0');
wasm.key_down(51);
if (wasm.debug_player_weapon_slot() !== 1) fail('#1 key 3 did not select weapon slot 1');
const autoBefore = wasm.debug_player_flags_b() & 2;
wasm.pointer_down(1, 650, 385);
wasm.pointer_up(1, 650, 385);
const autoAfter = wasm.debug_player_flags_b() & 2;
if (autoBefore === autoAfter) fail('#7 AUTO button click did not toggle the auto-XP flag');

// ---------------------------------------------------------------- 6 sounds
wasm.key_down(67); // C -> character screen + UI click
const soundCount = wasm.sound_count();
if (soundCount < 1) fail('#6 UI action queued no sound');
const soundIds = [];
for (let i = 0; i < soundCount; i++) soundIds.push(wasm.sound_id(i));
if (!soundIds.includes(4)) fail(`#6 UI click id 4 missing (got ${soundIds})`);
wasm.key_down(27);

// ---------------------------------------------------------------- 5 cursor
wasm.pointer_move(20, 20);
if (wasm.debug_cursor_index() !== 4) fail('#5 HUD cursor is not the neutral arrow');
wasm.step(0.016);
await save('cursor_hud_arrow.png');
const groundCursor = (wasm.pointer_move(400, 300), wasm.debug_cursor_index());
if (groundCursor !== 4) fail(`#5 ground cursor is not the neutral arrow (${groundCursor})`);
// Ground WorldObject cell (`FUN_00428740` 0x2000 branch): the original shows
// the hand badge over active collectable piles, even off the solid pixels.
let handChecked = false;
for (let i = 0; i < wasm.debug_world_object_count(); i++) {
  const cell = wasm.debug_world_object_cell(i);
  if (cell === 0xFFFFFFFF) continue;
  const pt = wasm.debug_cell_screen(cell & 0xFFFF, cell >>> 16);
  const sx = pt >>> 16, sy = pt & 0xFFFF;
  if (sx < 140 || sx >= 800 || sy < 0 || sy >= 486) continue;
  wasm.pointer_move(sx, sy);
  if (wasm.debug_cursor_index() === 7) { handChecked = true; break; }
}
if (!handChecked) fail('#5 no on-screen ground object showed the hand cursor');
if (handChecked) {
  wasm.step(0.016); // redraw the frame with the cursor over the marker
  await save('cursor_ground_hand.png');
}
wasm.pointer_move(159 + 35, 486 + 35); // inventory potion
if (wasm.debug_cursor_index() !== 4) fail('#5 inventory item cursor is not the neutral arrow');

// ---------------------------------------------------------------- 2 tooltip
wasm.key_down(67); // C
wasm.pointer_move(217, 138); // MAIN_800 slot 2 centre
if (wasm.debug_hover_item() !== 1) fail('#2 character screen hover did not resolve the knife');
wasm.step(0.016);
wasm.step(0.016);
await save('tooltip_character.png');
wasm.key_down(27);

// ---------------------------------------------------------------- 7 engage
const staged = wasm.debug_ai_setup();
if (staged === 0) fail('#7 AI staging found no enemy');
const enemy = staged - 1;
wasm.key_down(65); // A
if (wasm.debug_player_target() !== enemy) {
  fail(`#7 A/engage did not target actor ${enemy} (got ${wasm.debug_player_target()})`);
}
steps(80);
await save('engage.png');

// ---------------------------------------------------------------- 3 attack
wasm.debug_combat_set_seed(1);
if (wasm.debug_combat_setup() === 0) fail('#3 combat staging failed');
const hashes = new Set();
let sawMeleeRow = false;
let saved = 0;
for (let i = 0; i < 80; i++) {
  wasm.step(0.06);
  const s = combatState();
  if (!s) continue;
  if (s.target_action === '5') sawMeleeRow = true;
  if (s.player_attack_frame !== '0') hashes.add(hashRegion(280, 170, 260, 260));
  if (saved < 4 && s.player_attack_frame !== '0' && i % 3 === 0) {
    await save(`attack_${String(saved).padStart(2, '0')}.png`);
    saved++;
  }
  if (s.target_dead === '1') break;
}
if (!sawMeleeRow) fail('#3 target never entered melee action row 5');
if (hashes.size < 10) fail(`#3 attack animation rendered only ${hashes.size} distinct frames`);

// ---------------------------------------------------------------- 4 trade
steps(6);
if (wasm.debug_trade_open(278) !== 1) fail('#4 role-2 merchant did not open');
if (wasm.debug_trade_cell(2, 0) === 0) fail('#4 role-2 merchant has no stock');
wasm.step(0.016);
wasm.step(0.016);
await save('trade_merchant.png');
wasm.debug_trade_cancel();
if (wasm.debug_trade_open(279) !== 1) fail('#4 role-3 merchant did not open');
if (wasm.debug_trade_cell(2, 0) !== 0) fail('#4 role-3 merchant unexpectedly has stock');
wasm.step(0.016);
wasm.step(0.016);
await save('trade_empty.png');
wasm.debug_trade_cancel();

// ---------------------------------------------------------------- 8 aggro
wasm.debug_clear_combat_targets();
wasm.debug_set_player_hp(1600);
wasm.debug_set_group_activity(4, 0);
const hpBefore = playerHp();
if (wasm.debug_set_player_cell(20, 16) !== 1) fail('#8 teleport failed');
steps(20);
if (wasm.debug_group_activity(4) < 1) fail('#8 ant group did not wake inside rectangle A');
const targets = [37, 38, 39, 40, 41].map((id) => wasm.debug_world_target(id));
if (!targets.some((t) => t >= 0)) fail(`#8 ant group acquired no target (${targets})`);
steps(160);
const hp = playerHp();
if (hp === null || hpBefore === null || hp >= hpBefore) {
  fail(`#8 ants did not damage the player (hp ${hpBefore} -> ${hp})`);
}
await save('aggro.png');

console.log('FEEDBACK SMOKE OK');
console.log('wrote', outDir);
