// Headless trade smoke test for the OpenKonung wasm ABI.
// Exercises role-2 restock -> open exchange -> buy -> sell (destroyed) -> cancel.
//
//   node web/tools/trade_smoke.mjs
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
if (manifest.quest) await loadFile(manifest.quest, (p, l) => wasm.load_resource(13, p, l));
if (manifest.intr?.["640"]) await loadFile(manifest.intr["640"], (p, l) => wasm.load_resource(11, p, l));
if (manifest.intr?.["800"]) await loadFile(manifest.intr["800"], (p, l) => wasm.load_resource(12, p, l));

if (wasm.boot(0, -1) !== 1) {
  console.error('boot failed');
  process.exit(1);
}
wasm.on_resize(1600, 1200);

function fail(msg) {
  console.error('FAIL:', msg);
  console.error(
    'state active=%d player_money=%d table=%d partner=%d',
    wasm.debug_trade_active(),
    wasm.debug_trade_money(0),
    wasm.debug_trade_money(2),
    wasm.debug_trade_money(1),
  );
  process.exit(1);
}

// `FUN_0041D048` runs the settlement pass once per 16 frames
// (`(frame + 7) & 0xF == 0`, i.e. frame 9), so advance past the first phase.
for (let frame = 0; frame < 12; frame++) wasm.step(0.06);
if (wasm.debug_trade_stock(2, 2) === 0) {
  // Give the restock loop one more settlement pass if it only armed the timer.
  for (let frame = 0; frame < 16; frame++) wasm.step(0.06);
}
let item = 0;
let stockSlot = -1;
for (let slot = 0; slot < 24; slot++) {
  const id = wasm.debug_trade_stock(2, slot);
  if (id !== 0) {
    item = id;
    stockSlot = slot;
    break;
  }
}
if (item === 0) fail('role-2 merchant stock was not restocked');

if (wasm.debug_trade_open(278) !== 1) fail('trade did not open for entity 278');
const money0 = wasm.debug_trade_money(0);
const cell = wasm.debug_trade_cell(2, 0);
if (cell !== item) fail(`stock cell mismatch (got ${cell}, expected ${item})`);
if (wasm.debug_trade_move(2, 0) !== 1) fail('could not move a merchant item to the offer row');
if (wasm.debug_trade_money(2) <= 0) fail('player table money was not auto-balanced');
if (wasm.debug_trade_confirm() !== 1) fail('buy confirm closed the exchange');

const money1 = wasm.debug_trade_money(0);
if (money1 >= money0) fail('buy did not deduct money');
if (wasm.debug_trade_stock(2, stockSlot) !== 0) fail('bought item stayed in merchant stock');

// Sell the received item back: the settlement merchant destroys it.
const inventory = [];
for (let col = 0; col < 5; col++) inventory.push(wasm.debug_trade_cell(0, col));
const bag_col = inventory.indexOf(item);
if (bag_col < 0) fail('bought item not found in the player bag row');
const beforeSell = wasm.debug_trade_money(0);
if (wasm.debug_trade_move(0, bag_col) !== 1) fail('could not offer the item back');
if (wasm.debug_trade_confirm() !== 1) fail('sell confirm failed');
const afterSell = wasm.debug_trade_money(0);
if (afterSell <= beforeSell) fail('sale did not pay the player');
if (wasm.debug_trade_stock(2, stockSlot) !== 0) fail('sold item was added to stock (must be destroyed)');

if (wasm.debug_trade_cancel() !== 1) fail('cancel did not close the exchange');
console.log('trade smoke OK: bought item', item, 'money', money0, '->', afterSell);
