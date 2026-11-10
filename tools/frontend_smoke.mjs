// Headless lifecycle smoke for the original frontend + live-game preservation.
//
//   node web/tools/frontend_smoke.mjs
//
// Flows:
//   A  fresh startup -> menu -> New Game -> New Hero -> confirm -> gameplay
//   B  gameplay -> Escape -> menu (Resume/Save available) -> Resume -> same game
//   C  menu -> Settings -> Escape -> menu (fresh and in-game)
//   D  in-game menu -> New Game -> New Hero -> Cancel -> menu -> Resume (game intact)
//   E  host size preserved across game/frontend, 640<->800 and sub-screens
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');

const KIND = {
  game: 0, graph: 1, objects: 2, exe: 3, names: 4,
  menu640: 5, menu800: 6, newh640: 7, newh800: 8, save640: 9, save800: 10,
};

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

function expect(cond, msg) { if (!cond) throw new Error('FAIL: ' + msg); }
function approx(a, b) { return Math.abs(a - b) < 1e-4; }

function frameHash() {
  const ptr = wasm.frame_ptr();
  const len = wasm.frame_w() * wasm.frame_h() * 4;
  const view = new Uint8Array(mem.buffer, ptr, len);
  let h = 2166136261 >>> 0;
  for (let i = 0; i < len; i += 97) {
    h ^= view[i];
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h;
}

// Map logical framebuffer coords -> host (canvas) coords, matching the Rust
// letterbox transform, so hit testing is exercised at non-native host sizes.
function toHost(lx, ly, hostW, hostH, logicalW, logicalH) {
  const scale = Math.min(hostW / logicalW, hostH / logicalH);
  const w = Math.round(logicalW * scale);
  const h = Math.round(logicalH * scale);
  const rx = Math.floor((hostW - w) / 2);
  const ry = Math.floor((hostH - h) / 2);
  return [rx + lx * scale, ry + ly * scale];
}

function click(x, y) {
  wasm.pointer_down(1, x, y);
  wasm.pointer_up(1, x, y);
  wasm.step(0.016);
}

await loadFile(manifest.resources.game, (p, l) => wasm.load_resource(KIND.game, p, l));
await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(KIND.graph, p, l));
await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(KIND.objects, p, l));
await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(KIND.exe, p, l));
if (manifest.resources.names) {
  await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(KIND.names, p, l));
}
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (id2, ptr, len) => wasm.load_profile(id2, ptr, len), id);
}
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}
for (const [res, slot, kind] of [
  ['640', 'menu', KIND.menu640], ['640', 'newh', KIND.newh640], ['640', 'save', KIND.save640],
  ['800', 'menu', KIND.menu800], ['800', 'newh', KIND.newh800], ['800', 'save', KIND.save800],
]) {
  const url = manifest.menu[res] && manifest.menu[res][slot];
  if (url) await loadFile(url, (p, l) => wasm.load_resource(kind, p, l));
}
console.log('loaded resources + menu RES (menu/newh/save)');

const NEWGAME_800 = [280, 303];
const SETTINGS_800 = [268, 353];
const LOAD_800 = [265, 250];
const RESUME_800 = [290, 161];
const HERO2_800 = [265, 108];
const OK_800 = [85, 527];       // 800 region 20 (left, Ok) rect (85,527,100,33)
const CANCEL_800 = [195, 527];   // 800 region 21 (right, Отмена) rect (195,527,100,33)
const CLASS_CHIEF_800 = [464, 49]; // 800 region 5 (Chief, entity_class+1) rect (464,49,105,20)
const CHA_MINUS_800 = [515, 195];     // 800 region 25 (Charisma '-') rect (515,195,30,15)
const MARKSMANSHIP_800 = [428, 436];  // 800 region 16 (Marksmanship) rect (428,434,134,20)

const decoder = new TextDecoder('utf-8');
function playerBlock() {
  if (!wasm.debug_player_state()) return '';
  const ptr = wasm.debug_player_state_ptr();
  const len = wasm.debug_player_state_len();
  return decoder.decode(new Uint8Array(mem.buffer, ptr, len));
}

// ---- Flow A: fresh startup -> main menu -> New Game -> New Hero -> confirm ----
expect(wasm.frontend_boot(800) === 1, 'frontend_boot(800)');
wasm.on_resize(800, 600);
wasm.step(0.016);
expect(wasm.frontend_state() === 0, 'A: MainMenu');
expect(wasm.frontend_is_active() === 1, 'A: frontend active');
expect(wasm.debug_game_exists() === 0, 'A: no game at startup');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'A: 800x600 menu');

// Resume must be absent on the fresh menu.
wasm.pointer_move(RESUME_800[0] + 5, RESUME_800[1] + 5);
expect(wasm.frontend_hover() === -1, 'A: Resume hidden at startup, got ' + wasm.frontend_hover());
wasm.pointer_move(0, 0);

click(NEWGAME_800[0] + 5, NEWGAME_800[1] + 5);
expect(wasm.frontend_state() === 1, 'A: NewHero');
expect(wasm.frontend_selected_hero() === 0, 'A: default hero Dragomir');
expect(wasm.frontend_selected_class() === 5, 'A: default Warrior');
click(HERO2_800[0] + 5, HERO2_800[1] + 5);
expect(wasm.frontend_selected_hero() === 2, 'A: hero 2 selected');
// Select Chief (region 5, "Купец"; region = entity_class+1).
click(CLASS_CHIEF_800[0] + 5, CLASS_CHIEF_800[1] + 5);
expect(wasm.frontend_selected_class() === 4, 'A: Chief class (entity +04 = 4)');
// Stepper: lower Charisma 25 -> 24 (region 25 = CHA '-') => refund 11 points.
click(CHA_MINUS_800[0] + 5, CHA_MINUS_800[1] + 5);
// Secondary-skill selector: Marksmanship (region 16).
click(MARKSMANSHIP_800[0] + 5, MARKSMANSHIP_800[1] + 5);
// Left bottom control is "Ok" (region 20 at 85,527).
click(OK_800[0] + 5, OK_800[1] + 5);
expect(wasm.frontend_is_active() === 0, 'A: gameplay active after Ok');
expect(wasm.debug_game_exists() === 1, 'A: game exists');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'A: 800x600 game');
const blockA = playerBlock();
expect(blockA.includes('HERO Wolf'), 'A: hero identity persisted: ' + blockA);
expect(blockA.includes('CLASS Chief (entity +04 = 4)'), 'A: class persisted: ' + blockA);
expect(blockA.includes('CHA base=24 current=24'), 'A: stepper edit persisted: ' + blockA);
expect(blockA.includes('points=11'), 'A: refunded points persisted: ' + blockA);
expect(blockA.includes('max_load=12.3'), 'A: derived max_load persisted: ' + blockA);
// Marksmanship selection (region 16) persisted; civil (Identification) untouched.
expect(blockA.includes('melee=0'), 'A: Marksmanship melee: ' + blockA);
expect(blockA.includes('archery=100'), 'A: Marksmanship archery: ' + blockA);
expect(blockA.includes('building=2'), 'A: Marksmanship building: ' + blockA);
expect(blockA.includes('smithing=5'), 'A: Marksmanship smithing: ' + blockA);
expect(blockA.includes('trade=10') && blockA.includes('healing=50') && blockA.includes('identify=20'),
  'A: civil fields untouched: ' + blockA);
console.log('Flow A OK');

// ---- Flow B: gameplay -> Escape -> menu -> Resume -> same game ----
click(520, 360); // set a walk target
for (let i = 0; i < 20; i++) wasm.step(0.016);
const before = { x: wasm.debug_player_x(), y: wasm.debug_player_y() };
wasm.key_down(27);
expect(wasm.frontend_is_active() === 1, 'B: Escape opens menu');
expect(wasm.frontend_state() === 0, 'B: menu is MainMenu');
expect(wasm.debug_game_exists() === 1, 'B: game preserved while in menu');
// Resume is now visible (hover index 0).
wasm.pointer_move(RESUME_800[0] + 5, RESUME_800[1] + 5);
expect(wasm.frontend_hover() === 0, 'B: Resume visible in-game, got ' + wasm.frontend_hover());
wasm.key_down(27);
expect(wasm.frontend_is_active() === 0, 'B: Resume returns to game');
const after = { x: wasm.debug_player_x(), y: wasm.debug_player_y() };
expect(approx(before.x, after.x) && approx(before.y, after.y),
  `B: player state preserved (${before.x},${before.y}) != (${after.x},${after.y})`);
expect(playerBlock().includes('CLASS Chief (entity +04 = 4)'),
  'B: character class preserved after Resume');
console.log('Flow B OK (player preserved)');

// ---- Flow C: Settings entry/back (in-game) ----
wasm.key_down(27);
expect(wasm.frontend_is_active() === 1, 'C: menu open');
click(SETTINGS_800[0] + 5, SETTINGS_800[1] + 5);
expect(wasm.frontend_state() === 2, 'C: Settings screen');
expect(wasm.debug_game_exists() === 1, 'C: game preserved in Settings');
wasm.key_down(27);
expect(wasm.frontend_state() === 0, 'C: back to MainMenu');

// Save and Load render distinct original SAVE screens (#1 vs #0).
const SAVE_800 = [288, 205];
click(SAVE_800[0] + 5, SAVE_800[1] + 5);
expect(wasm.frontend_state() === 4, 'C: SaveGame screen');
const saveHash = frameHash();
wasm.key_down(27);
click(LOAD_800[0] + 5, LOAD_800[1] + 5);
expect(wasm.frontend_state() === 3, 'C: LoadGame screen');
const loadHash = frameHash();
expect(saveHash !== loadHash, 'C: Save and Load screens must differ');
wasm.key_down(27);
expect(wasm.frontend_state() === 0, 'C: back to MainMenu');
console.log('Flow C OK');

// ---- Flow D: New Game -> Cancel preserves the suspended game ----
click(NEWGAME_800[0] + 5, NEWGAME_800[1] + 5);
expect(wasm.frontend_state() === 1, 'D: NewHero');
click(CANCEL_800[0] + 5, CANCEL_800[1] + 5);
expect(wasm.frontend_state() === 0, 'D: Cancel -> MainMenu');
expect(wasm.debug_game_exists() === 1, 'D: game still exists after Cancel');
wasm.key_down(27);
expect(wasm.frontend_is_active() === 0, 'D: Resume after cancel');
const afterCancel = { x: wasm.debug_player_x(), y: wasm.debug_player_y() };
expect(approx(after.x, afterCancel.x) && approx(after.y, afterCancel.y),
  'D: same game resumed after Cancel');
console.log('Flow D OK');

// ---- Flow E: host size preserved across all transitions at 1000x700 ----
wasm.on_resize(1000, 700);
expect(wasm.host_w() === 1000 && wasm.host_h() === 700, 'E: host recorded');

wasm.key_down(27); // game -> menu
expect(wasm.frontend_is_active() === 1, 'E: menu open');
expect(wasm.host_w() === 1000 && wasm.host_h() === 700, 'E: host preserved game->frontend');

// Settings hit test via letterboxed host coords.
const [sx, sy] = toHost(SETTINGS_800[0] + 5, SETTINGS_800[1] + 5, 1000, 700, 800, 600);
click(sx, sy);
expect(wasm.frontend_state() === 2, 'E: Settings hit test at 1000x700');
wasm.key_down(27);
expect(wasm.frontend_state() === 0, 'E: back');

// 640 <-> 800 frontend resolution switch preserves host and game.
expect(wasm.frontend_set_resolution(640) === 1, 'E: switch to 640');
expect(wasm.frame_w() === 640 && wasm.frame_h() === 480, 'E: 640x480 menu');
expect(wasm.host_w() === 1000 && wasm.host_h() === 700, 'E: host preserved across resolution switch');
const [lx, ly] = toHost(LOAD_800[0] * 640 / 800 + 5, LOAD_800[1] * 640 / 800 + 5, 1000, 700, 640, 480);
click(lx, ly);
expect(wasm.frontend_state() === 3, 'E: Load hit test at 640/1000x700');
wasm.key_down(27);
expect(wasm.frontend_state() === 0, 'E: back from Load');

expect(wasm.frontend_set_resolution(800) === 1, 'E: switch back to 800');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'E: 800x600 menu');

wasm.key_down(27); // menu -> Resume
expect(wasm.frontend_is_active() === 0, 'E: Resume to game');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'E: 800x600 game');
expect(wasm.host_w() === 1000 && wasm.host_h() === 700, 'E: host preserved frontend->game');

wasm.on_resize(1200, 800);
expect(wasm.host_w() === 1200 && wasm.host_h() === 800, 'E: resize updates host');
console.log('Flow E OK');

// ---- Flow F: game over -> New Game -> Ok restarts a fresh run ----
// Regression: the game-over movie discards the live game, so the raw resource
// buffers must be reclaimed by the loader or the next `start_game` fails and
// the New-Hero Ok button silently does nothing.
expect(wasm.debug_game_exists() === 1, 'F: live game before death');
expect(wasm.debug_combat_kill_player() === 1, 'F: protagonist killed');
wasm.step(0.016);
expect(wasm.video_pending() === 1, 'F: game-over movie queued');
wasm.video_finish();
wasm.step(0.016);
expect(wasm.frontend_is_active() === 1, 'F: game over returns to the menu');
expect(wasm.frontend_state() === 0, 'F: MainMenu after game over');
expect(wasm.debug_game_exists() === 0, 'F: finished run discarded');

const [ngx, ngy] = toHost(NEWGAME_800[0] + 5, NEWGAME_800[1] + 5, 1200, 800, 800, 600);
click(ngx, ngy);
expect(wasm.frontend_state() === 1, 'F: NewHero after death');
const [okx, oky] = toHost(OK_800[0] + 5, OK_800[1] + 5, 1200, 800, 800, 600);
click(okx, oky);
expect(wasm.frontend_is_active() === 0, 'F: Ok starts gameplay after death');
expect(wasm.debug_game_exists() === 1, 'F: fresh game exists after death');
expect(wasm.frame_w() === 800 && wasm.frame_h() === 600, 'F: 800x600 game after restart');
console.log('Flow F OK (New Game after death)');

console.log('FRONTEND SMOKE OK');
