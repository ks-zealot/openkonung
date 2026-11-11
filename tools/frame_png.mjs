// Render wasm frames to PNG for visual verification.
//   node web/tools/frame_png.mjs
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { deflateSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const baseDir = join(webDir, '..');

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

async function loadFile(file, fn, id) {
  const buf = await readFile(join(baseDir, file));
  const ptr = wasm.alloc(buf.length);
  new Uint8Array(mem.buffer, ptr, buf.length).set(buf);
  if (id === undefined) fn(ptr, buf.length);
  else fn(id, ptr, buf.length);
}

await loadFile(manifest.resources.game, (p, l) => wasm.load_resource(0, p, l));
await loadFile(manifest.resources.graph, (p, l) => wasm.load_resource(1, p, l));
await loadFile(manifest.resources.objects, (p, l) => wasm.load_resource(2, p, l));
await loadFile(manifest.resources.exe, (p, l) => wasm.load_resource(3, p, l));
if (manifest.resources.names) {
  await loadFile(manifest.resources.names, (p, l) => wasm.load_resource(4, p, l));
}
if (manifest.quest) {
  await loadFile(manifest.quest, (p, l) => wasm.load_resource(13, p, l));
}
for (const p of manifest.profiles) {
  const id = parseInt(p.replace(/\D/g, ''), 10);
  await loadFile(p, (a, ptr, len) => wasm.load_profile(a, ptr, len), id);
}
for (const kon of manifest.konungs) {
  await loadFile(kon.file, (id, ptr, len) => wasm.load_konung(id, ptr, len), kon.id);
}

// Original INTR interface archives (kinds 11/12) so the HUD renders.
const intr = manifest.intr || {};
if (intr['640']) await loadFile(intr['640'], (k, p, l) => wasm.load_resource(k, p, l), 11);
if (intr['800']) await loadFile(intr['800'], (k, p, l) => wasm.load_resource(k, p, l), 12);
console.log('loaded INTR', Object.keys(intr).join(',') || '(none)');

// Isolated 640x480 HUD preview (the live game is fixed at 800x600). Must run
// before boot(), which moves GRAPH.RES out of the loader.
if (wasm.debug_render_intr_hud(640)) {
  const w = wasm.debug_hud_w();
  const h = wasm.debug_hud_h();
  const view = Buffer.from(new Uint8Array(mem.buffer, wasm.debug_hud_ptr(), w * h * 4));
  await writeFile(join(here, 'hud_640.png'), encodePng(w, h, view)).then(() => console.log('wrote hud_640.png'));
}

// hero_index 0 = Dragomir (default).
if (wasm.boot(0, -1) !== 1) throw new Error('boot failed');
wasm.on_resize(800, 600);

function save(name) {
  const ptr = wasm.frame_ptr();
  const len = wasm.frame_w() * wasm.frame_h() * 4;
  const view = Buffer.from(new Uint8Array(mem.buffer, ptr, len));
  const png = encodePng(wasm.frame_w(), wasm.frame_h(), view);
  return writeFile(join(here, name), png).then(() => console.log('wrote', name));
}

// Crop a region out of the live 800x600 framebuffer.
function saveRegion(name, x, y, w, h) {
  const fw = wasm.frame_w();
  const fh = wasm.frame_h();
  const full = Buffer.from(new Uint8Array(mem.buffer, wasm.frame_ptr(), fw * fh * 4));
  const crop = Buffer.alloc(w * h * 4);
  for (let row = 0; row < h; row++) {
    const src = ((y + row) * fw + x) * 4;
    full.copy(crop, row * w * 4, src, src + w * 4);
  }
  return writeFile(join(here, name), encodePng(w, h, crop)).then(() => console.log('wrote', name));
}

// Walk a bit so the player is mid-motion, then capture the integrated HUD.
wasm.pointer_down(1, 520, 360);
wasm.pointer_up(1, 520, 360);
for (let i = 0; i < 25; i++) wasm.step(0.016);
await save('detail.png');
await saveRegion('hud_inventory_strip.png', 100, 470, 700, 130);
await saveRegion('hud_party_panel.png', 0, 0, 160, 600);

// Hover the first inventory cell to show the item label in the bottom bar.
// Host == virtual here (on_resize(800,600), scale 1); read-only hover.
wasm.pointer_move(263, 521);
for (let i = 0; i < 2; i++) wasm.step(0.016);
await save('detail_hover.png');
await saveRegion('hud_item_label.png', 100, 545, 700, 55);
wasm.pointer_move(0, 0); // clear hover

// Hover the protagonist to show the original actor panel (`FUN_00430958`):
// name, class, level/health and the party stat line, left aligned.
{
  const px = wasm.debug_player_x();
  const py = wasm.debug_player_y();
  const packed = wasm.debug_cell_screen(px, py);
  const sx = (packed >>> 16) & 0xFFFF;
  const sy = packed & 0xFFFF;
  wasm.pointer_move(sx, Math.max(0, sy - 74));
  for (let i = 0; i < 2; i++) wasm.step(0.016);
  await save('detail_hover_actor.png');
  await saveRegion('hud_actor_label.png', 100, 545, 700, 55);
  wasm.pointer_move(0, 0); // clear hover
}

// Character screen: `C` (the original opens it via a UI command, not a mouse
// portrait click).
wasm.key_down(67);
for (let i = 0; i < 2; i++) wasm.step(0.016);
console.log('character_screen =', wasm.debug_character_screen());
await save('character_screen.png');
await saveRegion('character_equipment.png', 140, 30, 280, 240); // equipped slots
await saveRegion('character_doll.png', 540, 130, 220, 220);     // paper doll
await saveRegion('character_stats.png', 430, 40, 330, 250);     // name/class + stats
await saveRegion('character_inventory.png', 100, 470, 700, 130); // strip stays visible

// Escape closes and returns to normal gameplay.
wasm.key_down(27);
for (let i = 0; i < 2; i++) wasm.step(0.016);
console.log('character_screen after Escape =', wasm.debug_character_screen());
await save('after_close.png');
await saveRegion('player_peace.png', 300, 150, 200, 200);
await saveRegion('hud_buttons.png', 0, 330, 140, 270);

// World map (interface mode 5). Reveal all locations for a deterministic
// screenshot, then open/close with the original M key.
console.log('reveal all =', wasm.debug_reveal_all_locations());
wasm.key_down(77);
for (let i = 0; i < 2; i++) wasm.step(0.016);
console.log('map open =', wasm.debug_map_open());
await save('map_800.png');
await saveRegion('map_screen.png', 140, 0, 660, 556);
wasm.key_down(77);
for (let i = 0; i < 2; i++) wasm.step(0.016);

// Quest log (interface mode 2).
console.log('journal slots =', wasm.debug_activate_journal_slots(3));
wasm.key_down(81);
for (let i = 0; i < 2; i++) wasm.step(0.016);
await save('quest_log.png');
wasm.key_down(81);
for (let i = 0; i < 2; i++) wasm.step(0.016);

// TAB toggles the protagonist to battle stance: the equipped knife must switch
// from its sheathed overlay to the "in hand" variant (`FUN_00427e9c`).
wasm.key_down(9);
for (let i = 0; i < 3; i++) wasm.step(0.016);
await save('detail_battle.png');
await saveRegion('player_battle.png', 300, 150, 200, 200);

// Character sheet in battle stance: the paper doll must show the held knife.
wasm.key_down(67);
for (let i = 0; i < 2; i++) wasm.step(0.016);
await save('character_screen_battle.png');
await saveRegion('character_doll_battle.png', 540, 130, 220, 220);
wasm.key_down(27);
for (let i = 0; i < 2; i++) wasm.step(0.016);


// Trade window (interface mode 7): settle one logic frame so the role-2
// merchant restock seeds stock, then open entity 278's exchange.
for (let i = 0; i < 2; i++) wasm.step(0.06);
console.log('trade open =', wasm.debug_trade_open(278), 'stock =', wasm.debug_trade_stock(2, 2));
for (let i = 0; i < 2; i++) wasm.step(0.06);
await save('trade.png');
await saveRegion('trade_rows.png', 100, 0, 700, 600);
wasm.debug_trade_cancel();
for (let i = 0; i < 2; i++) wasm.step(0.016);
