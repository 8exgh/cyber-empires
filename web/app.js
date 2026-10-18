// Host shell for the Cyber Empires WebAssembly build: loads the original game data
// (from ./game/ on the server, or files/zip dropped by the user), runs the game loop,
// and forwards input.
"use strict";

const canvas = document.getElementById("screen");
const ctx = canvas.getContext("2d");
const overlay = document.getElementById("overlay");
const status = document.getElementById("status");
const drop = document.getElementById("drop");

const NEEDED = /\.(gif|map|dat|iat|bin|mus)$/i;

let wasm, mem, exports;
let running = false;

function utf8(s) { return new TextEncoder().encode(s); }

function b64(bytes) { let s = ""; for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]); return btoa(s); }
function unb64(s) { const t = atob(s); const b = new Uint8Array(t.length); for (let i = 0; i < t.length; i++) b[i] = t.charCodeAt(i); return b; }
const env = {
  // save slots live in localStorage as base64 copies of the original GAME<n>.DAT format
  js_store(slot, ptr, len) {
    try { localStorage.setItem("cyber-empires-save-" + slot, b64(new Uint8Array(mem.buffer, ptr, len))); } catch (e) { console.warn(e); }
  },
  js_fetch(slot, ptr, max) {
    try {
      const s = localStorage.getItem("cyber-empires-save-" + slot);
      if (!s) return 0;
      const b = unb64(s); if (b.length > max) return 0;
      new Uint8Array(mem.buffer, ptr, b.length).set(b); return b.length;
    } catch (e) { return 0; }
  },
};
async function loadWasm() {
  const imports = { env };
  const r = await fetch("cyber_empires.wasm", { cache: "no-store" });
  if (!r.ok) throw new Error("cyber_empires.wasm not found (run scripts/build.sh)");
  const { instance } = await WebAssembly.instantiate(await r.arrayBuffer(), imports);
  exports = instance.exports;
  mem = exports.memory;
}

function addAsset(name, bytes) {
  const ptr = exports.ce_alloc(bytes.length);
  new Uint8Array(mem.buffer, ptr, bytes.length).set(bytes);
  const n = utf8(name);
  const nptr = exports.ce_alloc(n.length);
  new Uint8Array(mem.buffer, nptr, n.length).set(n);
  exports.ce_add_asset(nptr, n.length, ptr, bytes.length);
}

// ---- zip reading (store + deflate) using DecompressionStream ----
async function readZip(buf) {
  const dv = new DataView(buf);
  const u8 = new Uint8Array(buf);
  // find end of central directory
  let eocd = -1;
  for (let i = buf.byteLength - 22; i >= 0 && i >= buf.byteLength - 70000; i--) {
    if (dv.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) throw new Error("not a zip file");
  const count = dv.getUint16(eocd + 10, true);
  let off = dv.getUint32(eocd + 16, true);
  const files = [];
  for (let i = 0; i < count; i++) {
    if (dv.getUint32(off, true) !== 0x02014b50) break;
    const method = dv.getUint16(off + 10, true);
    const csize = dv.getUint32(off + 20, true);
    const usize = dv.getUint32(off + 24, true);
    const nlen = dv.getUint16(off + 28, true);
    const elen = dv.getUint16(off + 30, true);
    const clen = dv.getUint16(off + 32, true);
    const lho = dv.getUint32(off + 42, true);
    const name = new TextDecoder().decode(u8.subarray(off + 46, off + 46 + nlen));
    files.push({ name, method, csize, usize, lho });
    off += 46 + nlen + elen + clen;
  }
  const out = [];
  for (const f of files) {
    if (f.name.endsWith("/") || !NEEDED.test(f.name)) continue;
    const nlen = dv.getUint16(f.lho + 26, true);
    const elen = dv.getUint16(f.lho + 28, true);
    const start = f.lho + 30 + nlen + elen;
    const data = u8.subarray(start, start + f.csize);
    let bytes;
    if (f.method === 0) bytes = new Uint8Array(data);
    else if (f.method === 8) {
      const ds = new DecompressionStream("deflate-raw");
      const stream = new Blob([data]).stream().pipeThrough(ds);
      bytes = new Uint8Array(await new Response(stream).arrayBuffer());
    } else continue;
    out.push({ name: f.name.split("/").pop(), bytes });
  }
  return out;
}

async function filesFromServer() {
  // Expect web/game/manifest.txt listing file names (one per line), produced by scripts/extract-assets.py
  const r = await fetch("game/manifest.txt", { cache: "no-store" });
  if (!r.ok) return null;
  const names = (await r.text()).split(/\r?\n/).map(s => s.trim()).filter(Boolean);
  const out = [];
  for (const n of names) {
    const fr = await fetch("game/" + n, { cache: "no-store" });
    if (fr.ok) out.push({ name: n, bytes: new Uint8Array(await fr.arrayBuffer()) });
  }
  return out;
}

async function filesFromUser(fileList) {
  const out = [];
  for (const f of fileList) {
    const buf = await f.arrayBuffer();
    if (/\.zip$/i.test(f.name)) out.push(...await readZip(buf));
    else if (NEEDED.test(f.name)) out.push({ name: f.name, bytes: new Uint8Array(buf) });
  }
  return out;
}

async function startWith(files) {
  for (const f of files) addAsset(f.name, f.bytes);
  const err = exports.ce_start((Math.random() * 0xffffffff) >>> 0);
  if (err) {
    const msg = new TextDecoder().decode(new Uint8Array(mem.buffer, exports.ce_error_ptr(), err));
    status.textContent = msg;
    drop.hidden = false;
    return;
  }
  overlay.hidden = true;
  running = true;
  requestAnimationFrame(loop);
}

// ---- input ----
const KEY = { Escape: 1, Enter: 2, Backspace: 3, Space: 4, Tab: 5, ArrowUp: 10, ArrowDown: 11, ArrowLeft: 12, ArrowRight: 13,
  F1: 20, F2: 21, F3: 22, F4: 23, F5: 24, F6: 25, F7: 26, F8: 27, F9: 28, F10: 29,
  Numpad0: 40, Numpad1: 41, Numpad2: 42, Numpad3: 43, Numpad4: 44, Numpad5: 45, Numpad6: 46, Numpad7: 47, Numpad8: 48, Numpad9: 49,
  NumpadAdd: 50, NumpadEnter: 51, NumpadDecimal: 52, ShiftLeft: 60, ShiftRight: 60, ControlLeft: 61, ControlRight: 61, AltLeft: 62, AltRight: 62 };
function keyCode(e) {
  if (KEY[e.code] !== undefined) return KEY[e.code];
  const m = /^Key([A-Z])$/.exec(e.code); if (m) return m[1].charCodeAt(0);
  const d = /^Digit([0-9])$/.exec(e.code); if (d) return d[1].charCodeAt(0);
  return -1;
}
window.addEventListener("keydown", e => {
  if (!running) return;
  const k = keyCode(e);
  const browserKey = e.code === "F5" || e.code === "F11" || e.code === "F12";
  if (k >= 0) { exports.ce_key(k, 1); if (!browserKey) e.preventDefault(); }
  if (e.key.length === 1) exports.ce_typed(e.key.charCodeAt(0));
  if (e.key === "Backspace") exports.ce_typed(8);
});
window.addEventListener("keyup", e => { if (!running) return; const k = keyCode(e); if (k >= 0) { exports.ce_key(k, 0); if (e.code !== "F5" && e.code !== "F11" && e.code !== "F12") e.preventDefault(); } });
window.addEventListener("blur", () => { if (running) for (let i = 0; i < 256; i++) exports.ce_key(i, 0); });

let buttons = 0;
let clickLatch = 0; // keeps a very short click visible for at least one frame
function mousePos(e) {
  const r = canvas.getBoundingClientRect();
  const x = Math.floor((e.clientX - r.left) * 320 / r.width);
  const y = Math.floor((e.clientY - r.top) * 200 / r.height);
  return [Math.max(0, Math.min(319, x)), Math.max(0, Math.min(199, y))];
}
canvas.addEventListener("mousemove", e => { if (!running) return; const [x, y] = mousePos(e); exports.ce_mouse(x, y, buttons); });
canvas.addEventListener("mousedown", e => { if (!running) return; buttons = e.buttons; clickLatch = 2; const [x, y] = mousePos(e); exports.ce_mouse(x, y, buttons | 1); e.preventDefault(); });
// A press shorter than one frame would otherwise be released before the game samples it;
// while the click latch is active the release is deferred to the frame loop.
window.addEventListener("mouseup", e => { if (!running) return; buttons = e.buttons; if (clickLatch > 0) return; const [x, y] = mousePos(e); exports.ce_mouse(x, y, buttons); });
canvas.addEventListener("contextmenu", e => e.preventDefault());

// ---- loop ----
let last = performance.now();
const img = ctx.createImageData(320, 200);
function loop(now) {
  const dt = Math.min(0.1, (now - last) / 1000);
  last = now;
  if (clickLatch > 0) { clickLatch--; if (clickLatch === 0) exports.ce_mouse(curX, curY, buttons); }
  exports.ce_frame(dt);
  const ptr = exports.ce_framebuffer();
  img.data.set(new Uint8Array(mem.buffer, ptr, 320 * 200 * 4));
  ctx.putImageData(img, 0, 0);
  // software cursor
  if (exports.ce_cursor_visible()) drawCursor();
  requestAnimationFrame(loop);
}
let curX = 0, curY = 0;
canvas.addEventListener("mousemove", e => { [curX, curY] = mousePos(e); });
function drawCursor() {
  ctx.fillStyle = "#fff";
  ctx.beginPath(); ctx.moveTo(curX, curY); ctx.lineTo(curX + 7, curY + 7); ctx.lineTo(curX + 3, curY + 7); ctx.lineTo(curX, curY + 10); ctx.closePath(); ctx.fill();
  ctx.strokeStyle = "#000"; ctx.lineWidth = 0.5; ctx.stroke();
}

// ---- boot ----
(async () => {
  try {
    await loadWasm();
    status.textContent = "Looking for game data…";
    const files = await filesFromServer();
    if (files && files.length) { await startWith(files); return; }
  } catch (e) {
    status.textContent = String(e.message || e);
  }
  status.textContent = "";
  drop.hidden = false;
})();

document.getElementById("file").addEventListener("change", async e => {
  status.textContent = "Reading files…";
  await startWith(await filesFromUser(e.target.files));
});
for (const ev of ["dragenter", "dragover"]) document.body.addEventListener(ev, e => { e.preventDefault(); drop.classList.add("active"); });
document.body.addEventListener("dragleave", () => drop.classList.remove("active"));
document.body.addEventListener("drop", async e => {
  e.preventDefault(); drop.classList.remove("active");
  status.textContent = "Reading files…";
  const items = [];
  const list = e.dataTransfer.items;
  async function walk(entry) {
    if (entry.isFile) { items.push(await new Promise(res => entry.file(res))); }
    else if (entry.isDirectory) {
      const reader = entry.createReader();
      const entries = await new Promise(res => reader.readEntries(res));
      for (const en of entries) await walk(en);
    }
  }
  if (list && list.length && list[0].webkitGetAsEntry) {
    for (const it of list) { const en = it.webkitGetAsEntry(); if (en) await walk(en); }
  } else {
    items.push(...e.dataTransfer.files);
  }
  await startWith(await filesFromUser(items));
});
