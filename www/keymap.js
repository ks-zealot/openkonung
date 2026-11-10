// Physical-key → original VK-code mapping for the browser shell.
//
// The original game dispatches on Win32 virtual-key codes, which are
// layout-independent for A..Z and 0..9 (a Russian keyboard still reports
// VK_M when the physical M key is pressed). DOM `KeyboardEvent.key` is
// layout-dependent (`"ь"` on a Russian layout), so hotkeys must be mapped from
// `KeyboardEvent.code` instead.
//
// This module is imported by `app.js` and is also exercised directly by
// `tools/keymap_smoke.mjs`.

const SIMPLE = Object.freeze({
  Escape: 27,
  Enter: 13,
  Backspace: 8,
  Tab: 9,
  Space: 32,
  Home: 36,
  KeyA: 65,
  KeyC: 67,
  KeyD: 68,
  KeyE: 69,
  KeyF: 70,
  KeyH: 72,
  KeyI: 73,
  KeyK: 75,
  KeyL: 76,
  KeyM: 77,
  KeyP: 80,
  KeyQ: 81,
  KeyR: 82,
  KeyT: 84,
  KeyV: 86,
});

/// Original VK code for a DOM keyboard event, or 0 when not bound.
/// Uses `event.code` first; falls back to `event.key` only for non-letter
/// control keys that are layout independent anyway.
export function keyCodeForEvent(e) {
  if (!e) return 0;
  const code = typeof e.code === 'string' ? e.code : '';
  if (code && SIMPLE[code] !== undefined) return SIMPLE[code];
  if (/^Digit[0-9]$/.test(code)) return 48 + Number(code.slice(5));
  if (/^Numpad[0-9]$/.test(code)) return 96 + Number(code.slice(6));

  // Fallback for environments that do not provide `code`.
  const key = typeof e.key === 'string' ? e.key : '';
  switch (key) {
    case 'Escape': return 27;
    case 'Enter': return 13;
    case 'Backspace': return 8;
    case 'Tab': return 9;
    case ' ': return 32;
    case 'Home': return 36;
    default: break;
  }
  if (key.length === 1 && key >= '0' && key <= '9') return key.charCodeAt(0);
  if (key.length === 1) {
    const upper = key.toUpperCase();
    if (upper >= 'A' && upper <= 'Z') return upper.charCodeAt(0);
  }
  return 0;
}

/// True for a key-down event that should be treated as a one-shot command.
/// Held keys are ignored by the game shell to mirror the original 60 ms
/// one-command-per-frame throttle.
export function isOneShotCommand(e) {
  return !!(e && !e.repeat);
}
