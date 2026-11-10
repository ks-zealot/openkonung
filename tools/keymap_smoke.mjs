// Layout-independent hotkey mapping smoke test.
//
// Verifies that the browser shell maps physical keys (`event.code`), so a
// Russian/Cyrillic keyboard layout still produces the original VK codes.
//
//   node web/tools/keymap_smoke.mjs
import { readFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = join(here, '..');
const { keyCodeForEvent, isOneShotCommand } = await import(
  pathToFileURL(join(webDir, 'www', 'keymap.js')).href
);

function fail(msg) {
  console.error('FAIL:', msg);
  process.exit(1);
}

// Cyrillic layout: the physical M/Q keys produce Cyrillic `key` values.
const cases = [
  [{ code: 'KeyM', key: 'ь' }, 77, 'M on Russian layout'],
  [{ code: 'KeyQ', key: 'й' }, 81, 'Q on Russian layout'],
  [{ code: 'KeyC', key: 'с' }, 67, 'C on Russian layout'],
  [{ code: 'KeyI', key: 'ш' }, 73, 'I on Russian layout'],
  [{ code: 'KeyA', key: 'ф' }, 65, 'A on Russian layout'],
  [{ code: 'KeyF', key: 'а' }, 70, 'F on Russian layout'],
  [{ code: 'Space', key: ' ' }, 32, 'Space'],
  [{ code: 'Tab', key: 'Tab' }, 9, 'Tab'],
  [{ code: 'Escape', key: 'Escape' }, 27, 'Escape'],
  [{ code: 'Backspace', key: 'Backspace' }, 8, 'Backspace'],
  [{ code: 'Home', key: 'Home' }, 36, 'Home'],
  [{ code: 'Digit1', key: '1' }, 49, 'Digit1'],
  [{ code: 'Digit2', key: '2' }, 50, 'Digit2'],
  [{ code: 'Digit3', key: '3' }, 51, 'Digit3'],
];
for (const [event, expected, label] of cases) {
  const got = keyCodeForEvent(event);
  if (got !== expected) fail(`${label}: got ${got}, expected ${expected}`);
}

// Fallback for environments without `event.code`.
if (keyCodeForEvent({ key: 'M' }) !== 77) fail('fallback KeyM');
if (keyCodeForEvent({ key: 'q' }) !== 81) fail('fallback KeyQ');

// One-shot command filter.
if (!isOneShotCommand({ repeat: false })) fail('non-repeat must be one-shot');
if (isOneShotCommand({ repeat: true })) fail('repeat must be filtered');

// The shell must load the module rather than dispatching on `event.key`.
const app = await readFile(join(webDir, 'www', 'app.js'), 'utf8');
if (!app.includes('keymap.js')) fail('app.js does not import keymap.js');
if (!app.includes('keyCodeForEvent')) fail('app.js does not use the physical-key mapper');

console.log('KEYMAP SMOKE OK');
