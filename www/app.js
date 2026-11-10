// OpenKonung browser shell.
//
// Thin host around the Rust/wasm core: fetch the original game resources and
// the original menu RES families, hand them to the module, forward input, and
// present the active framebuffer (original frontend 640x480 / 800x600, or the
// 800x600 game) plus the text overlay queue.
//
// The framebuffer dimensions are dynamic: the Rust side starts on the original
// menu (which may be 640x480 or 800x600) and switches to the 800x600 game once
// a hero is confirmed. JS re-reads frame_w()/frame_h() every frame.

const KIND_GAME = 0;
const KIND_GRAPH = 1;
const KIND_OBJECTS = 2;
const KIND_EXE = 3;
const KIND_NAMES = 4;
const KIND_MENU640 = 5;
const KIND_MENU800 = 6;
const KIND_NEWH640 = 7;
const KIND_NEWH800 = 8;
const KIND_SAVE640 = 9;
const KIND_SAVE800 = 10;
const KIND_INTR640 = 11;
const KIND_INTR800 = 12;
const KIND_QUEST = 13;
const KIND_SOUNDS = 14;
const KIND_FLAMES = 15;
const KIND_LIGHTS = 16;
const KIND_LIGHT_MAP = 17;

const canvas = document.getElementById('game');
const ctx = canvas.getContext('2d');
const overlay = document.getElementById('overlay');
const msgEl = document.getElementById('msg');

let wasm = null;
let mem = null;
let off = document.createElement('canvas');
let offctx = off.getContext('2d', { willReadFrequently: true });
let imageData = offctx.createImageData(800, 600);
// Framebuffer size is unknown until the Rust runtime reports it; 0 forces the
// first present() to size the offscreen canvas (the HTML default is 300x150,
// which must never be used as a clip for an 800x600/640x480 frame).
let fbW = 0;
let fbH = 0;
let presentedOnce = false;
const decoder = new TextDecoder('utf-8');

// Content build id (from manifest.json). Every asset URL is versioned with it
// so a stale JS/WASM/manifest combination cannot be mixed.
let build = (typeof window !== 'undefined' && window.__OPENKONUNG_BUILD__) || 'dev';

let dpr = 1;
let lastTime = 0;
let running = false;

// Physical-key mapping (layout independent). Loaded from `keymap.js` once the
// manifest build id is known so the module is cache-busted with the shell.
let keyCodeForEvent = () => 0;
let isOneShotCommand = () => true;

// Background music: the original install's `music/` folder is played as a
// sequential playlist, wrapping back to the first track after the last.
// Browsers block autoplay until the user interacts, so playback is attempted
// at boot and retried on the first pointer/key event.
let baseUrl = '/';
let musicEl = null;
let musicList = [];
let musicIndex = 0;
let musicStarted = false;
// KONUNG.CFG audio gates (music +0x10, sound +0x18 in the 28-byte payload).
let soundEnabled = true;
let musicEnabled = true;
// Original movies (`*.flm` = AVI/Indeo 5) are transcoded to browser MP4s.
let videoFiles = {};
let videoBase = '';
let videoActive = false;
let videoPrompt = false;
let videoName = null;
let videoObjectUrl = null;
let videoSkipped = false;
let musicWasPlayingBeforeVideo = false;
// The pack is fetched after the wasm module boots, which can be after the user
// has already clicked; remember that so the playlist can start immediately once
// it is configured.
let userGestured = false;
window.addEventListener('pointerdown', () => { userGestured = true; }, true);
window.addEventListener('keydown', () => { userGestured = true; }, true);

// In-game sound effects (SOUNDS.RES): the Rust core queues requested sound ids
// each frame; the shell decodes them to mono 16-bit AudioBuffers on first use
// (base rate 22050 Hz) and plays them. Created lazily on the first gesture
// because browsers block audio before user interaction.
let audioCtx = null;
const soundBuffers = new Map();

function ensureAudio() {
  const Ctor = window.AudioContext || window.webkitAudioContext;
  if (!Ctor) return;
  if (!audioCtx) {
    try { audioCtx = new Ctor(); } catch (e) { audioCtx = null; }
  }
  // Browsers create the context suspended until a user gesture. The context is
  // created at boot (so clips can be decoded), therefore every gesture and
  // every playback attempt must resume it; otherwise no effect is ever heard.
  if (audioCtx && (audioCtx.state === 'suspended' || audioCtx.state === 'interrupted')) {
    const p = audioCtx.resume();
    if (p && typeof p.catch === 'function') p.catch(() => {});
  }
}

function playSound(id, pan = 0, volume = 0, rate = 22050) {
  if (!wasm || !soundEnabled) return;
  ensureAudio();
  if (!audioCtx) return;
  const ptr = wasm.debug_sound_clip_ptr(id);
  const len = wasm.debug_sound_clip_len(id);
  if (!ptr || !len) return;
  const base = wasm.debug_sound_rate(id) || 22050;
  let buf = soundBuffers.get(id);
  if (!buf) {
    const frames = len >> 1; // s16 mono
    buf = audioCtx.createBuffer(1, frames, base);
    const dst = buf.getChannelData(0);
    // Re-derive the view: memory growth detaches old ArrayBuffers.
    const src = new Int16Array(mem.buffer, ptr, frames);
    for (let i = 0; i < frames; i++) dst[i] = src[i] / 32768;
    soundBuffers.set(id, buf);
  }
  // `FUN_0042A13C` copies the clip into one of three secondary buffers
  // (22050/19050/25050 Hz); in WebAudio that is a playback-rate shift.
  const node = audioCtx.createBufferSource();
  node.buffer = buf;
  node.playbackRate.value = (rate || base) / base;
  // Positional mix: `pan` uses DirectSound's -10000..10000 scale; `volume` is
  // hundredths of a decibel (0 = full, -10000 = silent).
  let out = node;
  if (audioCtx.createStereoPanner) {
    const panner = audioCtx.createStereoPanner();
    panner.pan.value = Math.max(-1, Math.min(1, pan / 10000));
    out.connect(panner);
    out = panner;
  }
  if (volume < 0) {
    const gain = audioCtx.createGain();
    gain.gain.value = Math.pow(10, volume / 2000);
    out.connect(gain);
    out = gain;
  }
  out.connect(audioCtx.destination);
  node.start();
}

function setStatus(html, cls) {
  msgEl.innerHTML = '<span class="' + (cls || '') + '">' + html + '</span>' +
    '<div style="margin-top:8px;color:#6f8">build: ' + build + '</div>';
}

// Append the build id and forbid caching of every fetched asset.
function bust(url) {
  return url + (url.includes('?') ? '&' : '?') + 'v=' + encodeURIComponent(build);
}

async function fetchBytes(url) {
  const r = await fetch(bust(url), { cache: 'no-store' });
  if (!r.ok) throw new Error(bust(url) + ' -> HTTP ' + r.status);
  const buf = await r.arrayBuffer();
  return { bytes: new Uint8Array(buf), len: buf.byteLength };
}

function copyInto(ptr, bytes, len) {
  // Re-derive the view after alloc: memory growth detaches old ArrayBuffers.
  new Uint8Array(mem.buffer, ptr, len).set(bytes);
}

async function loadResource(url, kind) {
  const { bytes, len } = await fetchBytes(url);
  const ptr = wasm.alloc(len);
  copyInto(ptr, bytes, len);
  wasm.load_resource(kind, ptr, len);
}

async function loadProfile(id, url) {
  const { bytes, len } = await fetchBytes(url);
  const ptr = wasm.alloc(len);
  copyInto(ptr, bytes, len);
  wasm.load_profile(id, ptr, len);
}

async function loadKonung(id, url) {
  const { bytes, len } = await fetchBytes(url);
  const ptr = wasm.alloc(len);
  copyInto(ptr, bytes, len);
  wasm.load_konung(id, ptr, len);
}

async function loadLightMap(id, url) {
  const { bytes, len } = await fetchBytes(url);
  const ptr = wasm.alloc(len);
  copyInto(ptr, bytes, len);
  wasm.load_light_map(id, ptr, len);
}

function resize() {
  dpr = window.devicePixelRatio || 1;
  const cw = Math.max(1, canvas.clientWidth);
  const ch = Math.max(1, canvas.clientHeight);
  canvas.width = Math.round(cw * dpr);
  canvas.height = Math.round(ch * dpr);
  if (wasm) wasm.on_resize(canvas.width, canvas.height);
}

function gameRect() {
  const sw = canvas.width;
  const sh = canvas.height;
  const scale = Math.min(sw / fbW, sh / fbH);
  const w = Math.max(1, Math.round(fbW * scale));
  const h = Math.max(1, Math.round(fbH * scale));
  return { x: Math.floor((sw - w) / 2), y: Math.floor((sh - h) / 2), w, h, scale };
}

function ensureOffscreen(w, h) {
  if (off.width === w && off.height === h) return;
  off.width = w;
  off.height = h;
  offctx = off.getContext('2d', { willReadFrequently: true });
  imageData = offctx.createImageData(w, h);
}

function mousePos(e) {
  const r = canvas.getBoundingClientRect();
  return {
    x: (e.clientX - r.left) * dpr,
    y: (e.clientY - r.top) * dpr,
  };
}

const BTN = { 0: 1, 1: 2, 2: 3 };

function wireInput() {
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());

  canvas.addEventListener('mousedown', (e) => {
    e.preventDefault();
    if (videoActive) return;
    const p = mousePos(e);
    // Additive party-selection modifier (original `_DAT_0081D624`) and
    // force-talk modifier (original `_DAT_0081D5E4`).
    wasm.set_shift(e.shiftKey ? 1 : 0);
    wasm.set_ctrl(e.ctrlKey ? 1 : 0);
    wasm.pointer_down(BTN[e.button] || 1, p.x, p.y);
  });

  window.addEventListener('mouseup', (e) => {
    const p = mousePos(e);
    wasm.pointer_up(BTN[e.button] || 1, p.x, p.y);
  });

  window.addEventListener('mousemove', (e) => {
    const p = mousePos(e);
    wasm.pointer_move(p.x, p.y);
  });

  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    wasm.wheel(e.deltaY < 0 ? 1 : -1);
  }, { passive: false });

  window.addEventListener('keyup', (e) => {
    if (e.code === 'ShiftLeft' || e.code === 'ShiftRight' || e.key === 'Shift') wasm.set_shift(0);
    if (e.code === 'ControlLeft' || e.code === 'ControlRight' || e.key === 'Control') wasm.set_ctrl(0);
  });

  window.addEventListener('keydown', (e) => {
    // Movie playback is modal; Escape or any click on the movie/skip control
    // ends it early (playavi2.dll's form closes on click/key).
    if (videoActive || videoPrompt) {
      e.preventDefault();
      if (e.code === 'Escape' || e.code === 'Space' || e.code === 'Enter') skipVideo();
      return;
    }
    // Keep the additive-selection and force-talk modifiers current between
    // clicks/commands.
    wasm.set_shift(e.shiftKey ? 1 : 0);
    wasm.set_ctrl(e.ctrlKey ? 1 : 0);
    if (e.code === 'ShiftLeft' || e.code === 'ShiftRight' || e.key === 'Shift') return;
    if (e.code === 'ControlLeft' || e.code === 'ControlRight' || e.key === 'Control') return;

    // Debug resolution toggle for the original menu layouts: F1/F2. The digit
    // keys are gameplay weapon slots now.
    if (e.code === 'F1') { e.preventDefault(); wasm.frontend_set_resolution(640); return; }
    if (e.code === 'F2') { e.preventDefault(); wasm.frontend_set_resolution(800); return; }

    // Legacy original-file interop (diagnostic): Ctrl+I / Ctrl+E import/export
    // a `KONUNG.SAn` snapshot; with Shift the `KONUNG.SAV` name table.
    if (e.ctrlKey || e.metaKey) {
      if (e.code === 'KeyI') {
        e.preventDefault();
        pickLegacyFile(e.shiftKey ? 'names' : 'snapshot');
        return;
      }
      if (e.code === 'KeyE') {
        e.preventDefault();
        if (e.shiftKey) legacyExportNames(); else legacyExportSnapshot();
        return;
      }
    }

    // Held keys are ignored for one-shot commands; this mirrors the original
    // one-command-per-frame ring buffer and prevents rapid window toggling.
    if (!isOneShotCommand(e)) return;

    const code = keyCodeForEvent(e);
    if (code) {
      e.preventDefault();
      wasm.key_down(code);
    }
  });

  window.addEventListener('resize', resize);
}

function drawTexts(rect) {
  const n = wasm.text_count();
  const factor = rect.w / fbW;
  for (let i = 0; i < n; i++) {
    const x = wasm.text_x(i);
    const y = wasm.text_y(i);
    const rgb = wasm.text_rgb(i) >>> 0;
    const scale = wasm.text_scale(i);
    const ptr = wasm.text_ptr(i);
    const len = wasm.text_len(i);
    if (!ptr || !len) continue;
    const str = decoder.decode(new Uint8Array(mem.buffer, ptr, len));
    const size = Math.max(7, Math.round(14 * scale * factor));
    ctx.font = size + 'px Consolas, "DejaVu Sans Mono", "Courier New", monospace';
    ctx.textBaseline = 'top';
    ctx.fillStyle = 'rgb(' + ((rgb >> 16) & 255) + ',' + ((rgb >> 8) & 255) + ',' + (rgb & 255) + ')';
    ctx.fillText(str, rect.x + x * factor, rect.y + y * factor);
  }
}

function present() {
  // The original game draws its own cursor (and hides the OS one via
  // ShowCursor(0)); the frontend uses the normal system arrow.
  canvas.style.cursor = (wasm.frontend_is_active && wasm.frontend_is_active()) ? 'default' : 'none';
  const w = wasm.frame_w();
  const h = wasm.frame_h();
  // Always size the offscreen canvas to the runtime framebuffer before
  // putImageData. ensureOffscreen() is a no-op when the size already matches,
  // so this is cheap. (The guard previously skipped the first resize because
  // fbW/fbH started at 800x600 while the canvas defaulted to 300x150.)
  fbW = w;
  fbH = h;
  ensureOffscreen(fbW, fbH);
  if (!presentedOnce) {
    presentedOnce = true;
    window.__OPENKONUNG_FRAME__ = { fbW, fbH, offW: off.width, offH: off.height };
    console.log('[OpenKonung] present fb ' + fbW + 'x' + fbH +
      ' offscreen ' + off.width + 'x' + off.height);
  }
  const rect = gameRect();
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  const ptr = wasm.frame_ptr();
  if (!ptr) return;
  const len = fbW * fbH * 4;
  const src = new Uint8ClampedArray(mem.buffer, ptr, len);
  imageData.data.set(src);
  offctx.putImageData(imageData, 0, 0);

  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(off, 0, 0, fbW, fbH, rect.x, rect.y, rect.w, rect.h);

  drawTexts(rect);
}

// --- movies (transcoded original AVI/Indeo-5 clips) ----------------------
function setupVideo(manifest, base) {
  videoBase = base;
  videoFiles = manifest.videos || {};
  const movie = document.getElementById('movie');
  if (!movie) return;
  movie.addEventListener('ended', () => finishVideo(true));
  movie.addEventListener('error', () => finishVideo(false));
  movie.addEventListener('click', () => {
    if (videoPrompt) {
      retryVideoPlay();
    } else {
      finishVideo(true);
    }
  });
  const skip = document.getElementById('video-skip');
  if (skip) skip.addEventListener('click', (e) => { e.stopPropagation(); finishVideo(true); });
}

function videoUrlFor(name) {
  if (!name) return null;
  const baseName = name.replace(/\.mp4$/i, '').toLowerCase();
  if (typeof wasm.pack_video_count === 'function') {
    const n = wasm.pack_video_count() >>> 0;
    for (let i = 0; i < n; i++) {
      const np = wasm.pack_video_name_ptr(i);
      const nl = wasm.pack_video_name_len(i);
      if (!np || !nl) continue;
      const packedName = decoder.decode(new Uint8Array(mem.buffer, np, nl)).replace(/\.mp4$/i, '').toLowerCase();
      if (packedName !== baseName) continue;
      const p = wasm.pack_video_ptr(i);
      const len = wasm.pack_video_len(i) >>> 0;
      if (!p || !len) return null;
      if (videoObjectUrl) URL.revokeObjectURL(videoObjectUrl);
      const bytes = new Uint8Array(mem.buffer, p, len).slice();
      videoObjectUrl = URL.createObjectURL(new Blob([bytes], { type: 'video/mp4' }));
      return videoObjectUrl;
    }
  }
  const item = videoFiles[baseName];
  if (!item) return null;
  const file = typeof item === 'string' ? item : item.file;
  if (!file) return null;
  return bust(videoBase + file);
}

function currentVideoName() {
  if (typeof wasm.video_name_len !== 'function') return '';
  const len = wasm.video_name_len() >>> 0;
  if (!len) return '';
  const ptr = wasm.video_name_ptr();
  if (!ptr) return '';
  return decoder.decode(new Uint8Array(mem.buffer, ptr, len));
}

function pauseMusicForVideo() {
  musicWasPlayingBeforeVideo = !!(musicEl && musicStarted && !musicEl.paused);
  if (musicWasPlayingBeforeVideo && musicEl) musicEl.pause();
}

function resumeMusicAfterVideo() {
  if (musicWasPlayingBeforeVideo && musicEnabled && musicEl) {
    musicStarted = true;
    playMusicTrack(musicIndex);
  }
  musicWasPlayingBeforeVideo = false;
}

function showVideoPrompt() {
  videoPrompt = true;
  const hint = document.getElementById('video-hint');
  if (hint) hint.textContent = 'Click to play · Esc or Skip to continue';
}

function retryVideoPlay() {
  const movie = document.getElementById('movie');
  if (!movie) return;
  // This runs from a user gesture, so unmuted playback is allowed.
  movie.muted = false;
  movie.play().then(() => {
    videoPrompt = false;
    const hint = document.getElementById('video-hint');
    if (hint) hint.textContent = 'Esc or Skip to continue';
  }).catch(() => showVideoPrompt());
}

// Start every movie muted so browsers permit autoplay; the first click/key
// unmutes it (and satisfies the autoplay-with-audio policy).
function unmuteActiveVideo() {
  if (!videoActive) return;
  const movie = document.getElementById('movie');
  if (movie && movie.muted) movie.muted = false;
}
window.addEventListener('pointerdown', unmuteActiveVideo, true);
window.addEventListener('keydown', unmuteActiveVideo, true);

function maybeStartVideo() {
  if (videoActive || videoPrompt) return;
  if (typeof wasm.video_pending !== 'function' || wasm.video_pending() !== 1) return;
  const name = currentVideoName();
  if (!name) {
    wasm.video_finish();
    return;
  }
  const src = videoUrlFor(name);
  if (!src) {
    console.warn('[OpenKonung] movie unavailable:', name);
    wasm.video_finish();
    return;
  }
  const overlay = document.getElementById('video-overlay');
  const movie = document.getElementById('movie');
  if (!overlay || !movie) {
    wasm.video_finish();
    return;
  }
  videoActive = true;
  videoPrompt = false;
  videoSkipped = false;
  videoName = name;
  overlay.classList.add('show');
  const hint = document.getElementById('video-hint');
  if (hint) hint.textContent = 'Click for sound • Esc or Skip to continue';
  pauseMusicForVideo();
  movie.src = src;
  movie.currentTime = 0;
  movie.muted = true;
  movie.play().catch(() => showVideoPrompt());
}

function finishVideo(success) {
  const overlay = document.getElementById('video-overlay');
  const movie = document.getElementById('movie');
  if (overlay) overlay.classList.remove('show');
  if (movie) {
    movie.pause();
    movie.removeAttribute('src');
    movie.load();
  }
  if (videoObjectUrl) {
    URL.revokeObjectURL(videoObjectUrl);
    videoObjectUrl = null;
  }
  videoActive = false;
  videoPrompt = false;
  videoSkipped = false;
  videoName = null;
  if (typeof wasm.video_finish === 'function') wasm.video_finish();
  resumeMusicAfterVideo();
  if (musicEnabled && userGestured && !musicStarted) startMusic();
  lastTime = performance.now();
  if (success === false) console.warn('[OpenKonung] movie failed to decode');
}

function skipVideo() {
  if (!videoActive) return false;
  videoSkipped = true;
  finishVideo(true);
  return true;
}

// --- KONUNG.CFG (exact 28 bytes, base64 localStorage) --------------------
const CONFIG_KEY = 'openkonung.cfg';
const CONFIG_LEN = 28;
let lastConfigRevision = -1;

function applyConfigBytes(bytes) {
  if (!bytes || bytes.length !== CONFIG_LEN) return;
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  musicEnabled = dv.getUint32(16, true) !== 0;
  soundEnabled = dv.getUint32(24, true) !== 0;
  if (!musicEnabled && musicEl) {
    musicEl.pause();
    musicStarted = false;
  } else if (musicEnabled && musicEl && userGestured && !musicStarted) {
    startMusic();
  }
}

function loadConfig() {
  if (typeof wasm.config_load !== 'function') return;
  try {
    const raw = localStorage.getItem(CONFIG_KEY);
    if (!raw) return;
    const bytes = base64ToBytes(raw);
    if (bytes.length !== CONFIG_LEN) {
      console.warn('[OpenKonung] ignoring KONUNG.CFG with', bytes.length, 'bytes');
      return;
    }
    applyConfigBytes(bytes);
    const ptr = wasm.alloc(bytes.length);
    copyInto(ptr, bytes, bytes.length);
    if (wasm.config_load(ptr, bytes.length) !== 1) {
      console.warn('[OpenKonung] KONUNG.CFG rejected; using defaults');
    }
  } catch (e) {
    console.warn('[OpenKonung] KONUNG.CFG load failed', e);
  }
}

function persistConfigIfChanged() {
  if (typeof wasm.config_revision !== 'function') return;
  const rev = wasm.config_revision() >>> 0;
  if (rev === lastConfigRevision) return;
  lastConfigRevision = rev;
  try {
    const ptr = wasm.alloc(CONFIG_LEN);
    const n = wasm.config_write(ptr, CONFIG_LEN) >>> 0;
    if (n === CONFIG_LEN) {
      const bytes = new Uint8Array(mem.buffer, ptr, n).slice();
      applyConfigBytes(bytes);
      localStorage.setItem(CONFIG_KEY, bytesToBase64(bytes));
    }
    if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, CONFIG_LEN);
  } catch (e) {
    console.warn('[OpenKonung] KONUNG.CFG save failed', e);
  }
}

// --- save slots (compressed localStorage) ---------------------------------
const SAVE_PREFIX = 'openkonung.save.';
const SAVE_NAMES_KEY = 'openkonung.sav';
let saveNames = new Array(10).fill('');

function bytesToBase64(bytes) {
  let s = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    s += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
  }
  return btoa(s);
}

function base64ToBytes(b64) {
  const s = atob(b64);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

function syncSaveName(slot) {
  if (!wasm || typeof wasm.set_save_name !== 'function') return;
  const bytes = new TextEncoder().encode(saveNames[slot] || '');
  if (!bytes.length) return;
  const ptr = wasm.alloc(bytes.length);
  copyInto(ptr, bytes, bytes.length);
  wasm.set_save_name(slot, ptr, bytes.length);
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, bytes.length);
}

function loadSaveNames() {
  try {
    const raw = localStorage.getItem(SAVE_NAMES_KEY);
    if (!raw) return;
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return;
    for (let i = 0; i < 10; i++) {
      saveNames[i] = typeof arr[i] === 'string' ? arr[i] : '';
      if (saveNames[i]) syncSaveName(i);
    }
  } catch (e) {
    console.warn('[OpenKonung] cannot read save names', e);
  }
}

function defaultSlotName() {
  let base = 'Сохранение';
  if (wasm && typeof wasm.save_default_name === 'function' && wasm.save_default_name() === 1) {
    const ptr = wasm.save_default_name_ptr();
    const len = wasm.save_default_name_len();
    if (ptr && len) base = decoder.decode(new Uint8Array(mem.buffer, ptr, len));
  }
  const d = new Date();
  const wd = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const mo = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const pad = (n) => String(n).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, ' ');
  return base + ' ' + wd[d.getDay()] + ' ' + mo[d.getMonth()] + ' ' + day +
    ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
}

function saveSlotKey(slot) { return SAVE_PREFIX + slot; }

function writeSaveSlot(slot) {
  if (!wasm || typeof wasm.save_snapshot !== 'function') return false;
  if (wasm.save_snapshot() !== 1) {
    console.warn('[OpenKonung] save_snapshot failed');
    return false;
  }
  const ptr = wasm.save_snapshot_ptr();
  const len = wasm.save_snapshot_len();
  if (!ptr || !len) return false;
  const data = bytesToBase64(new Uint8Array(mem.buffer, ptr, len).slice());
  const name = defaultSlotName();
  saveNames[slot] = name;
  syncSaveName(slot);
  try {
    localStorage.setItem(saveSlotKey(slot), JSON.stringify({
      v: 1,
      name,
      created: Date.now(),
      data,
    }));
    localStorage.setItem(SAVE_NAMES_KEY, JSON.stringify(saveNames));
    return true;
  } catch (e) {
    console.warn('[OpenKonung] localStorage save failed', e);
    return false;
  }
}

function readSaveSlot(slot) {
  try {
    const raw = localStorage.getItem(saveSlotKey(slot));
    if (!raw) return null;
    return JSON.parse(raw);
  } catch (e) {
    console.warn('[OpenKonung] cannot read save slot ' + slot, e);
    return null;
  }
}

function loadSaveSlot(slot) {
  const rec = readSaveSlot(slot);
  if (!rec || typeof rec.data !== 'string') return false;
  let bytes;
  try { bytes = base64ToBytes(rec.data); } catch (e) { return false; }
  const ptr = wasm.alloc(bytes.length);
  copyInto(ptr, bytes, bytes.length);
  const ok = wasm.load_snapshot(ptr, bytes.length) === 1;
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, bytes.length);
  if (ok) {
    if (typeof rec.name === 'string' && rec.name) {
      saveNames[slot] = rec.name;
      syncSaveName(slot);
    }
    console.log('[OpenKonung] loaded save slot ' + slot);
  } else {
    console.warn('[OpenKonung] load_snapshot rejected slot ' + slot);
  }
  return ok;
}

// --- original `KONUNG.SAV` / `KONUNG.SA0..9` files ---------------------------

function downloadBytes(name, bytes) {
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  setTimeout(() => { URL.revokeObjectURL(url); a.remove(); }, 0);
}

function legacyExportSnapshot() {
  if (!wasm || typeof wasm.legacy_export_size !== 'function') return false;
  const len = wasm.legacy_export_size() >>> 0;
  if (!len) return false;
  const ptr = wasm.legacy_export_ptr();
  const bytes = new Uint8Array(mem.buffer, ptr, len).slice();
  downloadBytes('KONUNG.SA0', bytes);
  return true;
}

function legacyImportSnapshotBytes(bytes) {
  if (!wasm || typeof wasm.legacy_import !== 'function') return false;
  const ptr = wasm.alloc(bytes.length);
  copyInto(ptr, bytes, bytes.length);
  const ok = wasm.legacy_import(ptr, bytes.length) === 1;
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, bytes.length);
  return ok;
}

function legacyExportNames() {
  if (!wasm || typeof wasm.legacy_names_build !== 'function') return false;
  const text = new TextEncoder().encode(saveNames.join('\0'));
  const ptr = wasm.alloc(text.length);
  copyInto(ptr, text, text.length);
  const len = wasm.legacy_names_build(ptr, text.length) >>> 0;
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, text.length);
  if (!len) return false;
  const outPtr = wasm.legacy_export_ptr();
  downloadBytes('KONUNG.SAV', new Uint8Array(mem.buffer, outPtr, len).slice());
  return true;
}

function legacyImportNamesBytes(bytes) {
  if (!wasm || typeof wasm.legacy_names_parse !== 'function') return false;
  const ptr = wasm.alloc(bytes.length);
  copyInto(ptr, bytes, bytes.length);
  const ok = wasm.legacy_names_parse(ptr, bytes.length) === 1;
  if (typeof wasm.dealloc === 'function') wasm.dealloc(ptr, bytes.length);
  if (!ok) return false;
  const textPtr = wasm.legacy_names_text_ptr();
  const textLen = wasm.legacy_names_text_len();
  if (!textPtr || !textLen) return false;
  const text = decoder.decode(new Uint8Array(mem.buffer, textPtr, textLen));
  const parts = text.split('\0');
  for (let i = 0; i < 10; i++) {
    saveNames[i] = parts[i] || '';
    if (saveNames[i]) syncSaveName(i);
  }
  try { localStorage.setItem(SAVE_NAMES_KEY, JSON.stringify(saveNames)); } catch (e) {}
  return true;
}

function pickLegacyFile(kind) {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = kind === 'names' ? '.sav,.SAV' : '.sa0,.sa1,.sa2,.sa3,.sa4,.sa5,.sa6,.sa7,.sa8,.sa9,.SA0,.SA1,.SA2,.SA3,.SA4,.SA5,.SA6,.SA7,.SA8,.SA9';
  input.addEventListener('change', async () => {
    const file = input.files && input.files[0];
    if (!file) return;
    const bytes = new Uint8Array(await file.arrayBuffer());
    const ok = kind === 'names'
      ? legacyImportNamesBytes(bytes)
      : legacyImportSnapshotBytes(bytes);
    console.log('[OpenKonung] legacy ' + kind + ' import ' + (ok ? 'ok' : 'rejected'));
  });
  input.click();
}

function processSaveRequests() {
  if (!wasm) return;
  const saveSlot = typeof wasm.take_save_request === 'function' ? wasm.take_save_request() : -1;
  if (saveSlot >= 0) writeSaveSlot(saveSlot);
  const loadSlot = typeof wasm.take_load_request === 'function' ? wasm.take_load_request() : -1;
  if (loadSlot >= 0) loadSaveSlot(loadSlot);
}
function frame(now) {
  if (!running) return;
  const dt = Math.min(0.05, (now - lastTime) / 1000 || 0);
  lastTime = now;
  maybeStartVideo();
  if (videoActive || videoPrompt ||
      (typeof wasm.video_pending === 'function' && wasm.video_pending() === 1)) {
    persistConfigIfChanged();
    requestAnimationFrame(frame);
    return;
  }
  // Drain the Rust sound queue before stepping: input events queue clip ids
  // between frames, and `step()` clears the queue at the start of its draw pass.
  processSaveRequests();
  const ns = wasm.sound_count ? wasm.sound_count() : 0;
  for (let i = 0; i < ns; i++) {
    playSound(
      wasm.sound_id(i) >>> 0,
      wasm.sound_pan ? wasm.sound_pan(i) | 0 : 0,
      wasm.sound_volume ? wasm.sound_volume(i) | 0 : 0,
      wasm.sound_rate ? wasm.sound_rate(i) >>> 0 : 22050
    );
  }
  wasm.step(dt);
  maybeStartVideo();
  persistConfigIfChanged();
  present();
  requestAnimationFrame(frame);
}

// Resolve one playlist entry to a URL. Packed tracks are exposed by the Rust
// runtime as raw Ogg bytes; copy them into a Blob immediately (wasm memory may
// grow later, detaching the view) and keep the object URL for the session.
function musicTrackUrl(track) {
  if (track.url) return track.url;
  if (track.packUrl) return track.packUrl;
  if (typeof track.pack === 'number' && wasm) {
    const ptr = wasm.pack_music_ptr(track.pack);
    const len = wasm.pack_music_len(track.pack);
    if (!ptr || !len) return null;
    const bytes = new Uint8Array(mem.buffer, ptr, len).slice();
    track.packUrl = URL.createObjectURL(new Blob([bytes], { type: 'audio/ogg' }));
    return track.packUrl;
  }
  return null;
}

function playMusicTrack(i) {
  if (!musicEl || musicList.length === 0) return;
  const n = musicList.length;
  musicIndex = ((i % n) + n) % n;
  const url = musicTrackUrl(musicList[musicIndex]);
  if (!url) return;
  musicEl.src = url;
  const p = musicEl.play();
  // If autoplay was blocked, clear the flag so the first gesture retries.
  if (p && typeof p.catch === 'function') p.catch(() => { musicStarted = false; });
}

function startMusic() {
  if (musicStarted || !musicEl || !musicEnabled || videoActive || videoPrompt) return;
  musicStarted = true;
  playMusicTrack(musicIndex);
}

function setupMusic(manifest, base) {
  baseUrl = base;
  const tracks = [];
  const packed = (wasm && typeof wasm.pack_music_count === 'function')
    ? wasm.pack_music_count()
    : 0;
  if (packed > 0) {
    for (let i = 0; i < packed; i++) {
      let name = 'track ' + (i + 1);
      if (typeof wasm.pack_music_name_ptr === 'function') {
        const ptr = wasm.pack_music_name_ptr(i);
        const len = wasm.pack_music_name_len(i);
        if (ptr && len) name = decoder.decode(new Uint8Array(mem.buffer, ptr, len));
      }
      tracks.push({ pack: i, name });
    }
  } else if (Array.isArray(manifest.music)) {
    for (const name of manifest.music) tracks.push({ url: bust(base + name), name });
  }
  musicList = tracks;
  if (musicList.length === 0) return;
  musicEl = new Audio();
  musicEl.preload = 'auto';
  musicEl.volume = 0.5;
  musicEl.addEventListener('ended', () => playMusicTrack(musicIndex + 1));
  window.addEventListener('pointerdown', startMusic);
  window.addEventListener('keydown', startMusic);
  // The user may have clicked while resources were still loading.
  if (userGestured) startMusic();
}

async function loadMenuRes(manifest, base) {
  const menu = manifest.menu || {};
  const kinds = [
    ['640', 'menu', KIND_MENU640],
    ['640', 'newh', KIND_NEWH640],
    ['640', 'save', KIND_SAVE640],
    ['800', 'menu', KIND_MENU800],
    ['800', 'newh', KIND_NEWH800],
    ['800', 'save', KIND_SAVE800],
  ];
  for (const [res, slot, kind] of kinds) {
    const url = menu[res] && menu[res][slot];
    if (url) await loadResource(base + url, kind);
  }
}

async function main() {
  try {
    setStatus('loading manifest…');
    const manifestText = (await (await fetch(bust('manifest.json'), { cache: 'no-store' })).text()).replace(/^\uFEFF/, '');
    const manifest = JSON.parse(manifestText);
    build = manifest.build || build;
    // Layout-independent physical-key mapping, cache-busted with the shell.
    try {
      const keymap = await import('./keymap.js?v=' + encodeURIComponent(build));
      if (typeof keymap.keyCodeForEvent === 'function') keyCodeForEvent = keymap.keyCodeForEvent;
      if (typeof keymap.isOneShotCommand === 'function') isOneShotCommand = keymap.isOneShotCommand;
    } catch (e) {
      console.warn('[OpenKonung] keymap module failed to load', e);
    }
    window.__OPENKONUNG_BUILD__ = build;
    console.log('[OpenKonung] build id', build);
    const base = manifest.base || '/';

    setStatus('loading wasm module…');
    const { bytes: wasmBytes } = await fetchBytes('openkonung.wasm');
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    wasm = instance.exports;
    mem = wasm.memory;
    // Diagnostic handle (does not hold any state): lets devtools/tests drive and
    // inspect the Rust runtime that owns all frontend/game state.
    window.__OPENKONUNG_WASM__ = wasm;
    loadSaveNames();

    // Pack-only runtime: one ZIP archive contains every original file and the
    // background music. When it loads, no .res, .exe, Game.N, Konung.N or
    // /music/ request is made. `?nopack=1` is the explicit development escape
    // hatch that loads the per-file fallback instead.
    const forceRaw = new URLSearchParams(location.search).get('nopack') === '1';
    const packInfo = manifest.pack;
    const packFile = typeof packInfo === 'string'
      ? packInfo
      : (packInfo && packInfo.file) || 'konung.zip.pack';
    let packLoaded = false;
    if (!forceRaw) {
      if (typeof wasm.load_pack !== 'function') {
        throw new Error('this wasm build has no load_pack; rebuild the wasm module');
      }
      setStatus('loading resource pack (' + packFile + ')…');
      try {
        const { bytes, len } = await fetchBytes(packFile);
        const ptr = wasm.alloc(len);
        copyInto(ptr, bytes, len);
        packLoaded = wasm.load_pack(ptr, len) === 1;
        if (!packLoaded) throw new Error('load_pack rejected ' + packFile);
      } catch (e) {
        console.error('[OpenKonung] resource pack could not be loaded', e);
        setStatus('resource pack ' + packFile + ' could not be loaded.\n' +
          'Rebuild it with build.ps1 / build.sh, or run with ?nopack=1 after placing the original files.');
        throw e;
      }
    }

    if (forceRaw) {
      if (!manifest.resources) {
        throw new Error('?nopack=1 requires a manifest generated with --base (per-file resources)');
      }
      const res = manifest.resources;
      setStatus('loading game resources…\n' + res.game);
      await loadResource(base + res.game, KIND_GAME);
      setStatus('loading GRAPH.RES (terrain + human sprites)…');
      await loadResource(base + res.graph, KIND_GRAPH);
      setStatus('loading Objects.res (props + monsters)…');
      await loadResource(base + res.objects, KIND_OBJECTS);
      setStatus('loading executable tables (' + res.exe + ')…');
      await loadResource(base + res.exe, KIND_EXE);

      if (res.names) {
        setStatus('loading location names (' + res.names + ')…');
        await loadResource(base + res.names, KIND_NAMES);
      }

      for (const p of manifest.profiles) {
        setStatus('loading ' + p + '…');
        const id = parseInt(p.replace(/[^0-9]/g, ''), 10);
        await loadProfile(id, base + p);
      }

      let k = 0;
      for (const kon of manifest.konungs) {
        k++;
        if (k % 8 === 0 || k === manifest.konungs.length) {
          setStatus('loading Konung maps… ' + k + '/' + manifest.konungs.length);
        }
        await loadKonung(kon.id, base + kon.file);
      }

      setStatus('loading original menu resources…');
      await loadMenuRes(manifest, base);

      // Original in-game interface image archives (INTR), per resolution.
      // Decoded and cached by the Rust side when a game starts.
      const intr = manifest.intr || {};
      if (intr['640']) await loadResource(base + intr['640'], KIND_INTR640);
      if (intr['800']) await loadResource(base + intr['800'], KIND_INTR800);

      // Dialogue/quest database (QUEST_R.RES). Decoded by the Rust quest module.
      if (manifest.quest) {
        setStatus('loading dialogue data (' + manifest.quest + ')…');
        await loadResource(base + manifest.quest, KIND_QUEST);
      }

      // Sound effects archive (SOUNDS.RES). Raw s16 mono PCM decoded by the
      // Rust sound module; the shell plays queued ids via WebAudio.
      if (manifest.sounds) {
        setStatus('loading sound archive (' + manifest.sounds + ')…');
        await loadResource(base + manifest.sounds, KIND_SOUNDS);
      }

      // Prop fire animation frames (FLAMES.RES).
      if (manifest.flames) {
        setStatus('loading fire animation (' + manifest.flames + ').');
        await loadResource(base + manifest.flames, KIND_FLAMES);
      }

      // Lighting masks/tiles (LIGHTS.RES) + per-map LIGHTS.N index maps.
      if (manifest.lights) {
        setStatus('loading lighting data (' + manifest.lights + ').');
        await loadResource(base + manifest.lights, KIND_LIGHTS);
      }
      for (const lm of manifest.light_maps || []) {
        await loadLightMap(lm.id, base + lm.file);
      }
    }

    // Configure the playlist only after the pack decision: packed music comes
    // from wasm memory, the fallback from manifest.music + base.
    setupMusic(manifest, base);
    setupVideo(manifest, base);

    setStatus('booting…');
    wireInput();
    // Sound effects need an AudioContext, which browsers only allow after a
    // gesture; create it at boot (may be suspended) and on first input.
    ensureAudio();
    window.addEventListener('pointerdown', ensureAudio);
    window.addEventListener('keydown', ensureAudio);

    // Default to the configured original layout (KONUNG.CFG; 800x600 default);
    // key 1/2 switches to 640/800 for layout debugging.
    loadConfig();
    let ok = wasm.frontend_boot(0);
    if (!ok) {
      console.warn('frontend_boot failed; falling back to direct game start');
      // hero_index 0 = Dragomir (Game.1, location 4).
      ok = wasm.boot(0, -1);
      if (!ok) throw new Error('boot() failed (missing/corrupt resources)');
    }
    resize();

    // Original startup state 9: POSTER.FLM before the menu. Skippable, and
    // disabled for headless/automated runs with ?nointro=1 or ?novideo=1.
    const params = new URLSearchParams(location.search);
    if (!params.has('nointro') && !params.has('novideo') &&
        typeof wasm.start_intro_video === 'function' &&
        wasm.start_intro_video() === 1) {
      maybeStartVideo();
    }

    // Prove which bundle is executing and what framebuffer it produced.
    const source = packLoaded ? 'pack' : 'files';
    console.log('[OpenKonung] build', build, 'resources=' + source, 'booted; frame',
      wasm.frame_w() + 'x' + wasm.frame_h(),
      'frontend_active=' + wasm.frontend_is_active());
    console.log('[OpenKonung] pack music tracks', wasm.pack_music_count ? wasm.pack_music_count() : 0);
    setStatus('ready (build ' + build + ', resources ' + source + ', frame ' +
      wasm.frame_w() + 'x' + wasm.frame_h() + ')');
    overlay.classList.add('hidden');

    // Try to start the music now that loading is done; falls back to the
    // first user gesture if the browser blocked the autoplay attempt.
    startMusic();

    lastTime = performance.now();
    running = true;
    requestAnimationFrame(frame);
  } catch (err) {
    console.error(err);
    overlay.classList.remove('hidden');
    setStatus('Failed: ' + (err && err.message ? err.message : err) +
      '\n\nStart the local server, then open the root URL:\n' +
      '  powershell -ExecutionPolicy Bypass -File .\\serve.ps1\n' +
      '  bash serve.sh\n' +
      '  http://localhost:8000/', 'err');
  }
}

main();
