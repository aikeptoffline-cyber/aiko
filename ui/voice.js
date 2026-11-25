// Her voice: an offline neural voice (Piper "Kristin") run by the app itself, the same on Windows, macOS and
// Linux. The voice is downloaded once (about 21 MB); after that nothing leaves the PC. Replies are spoken
// sentence by sentence while they stream in: each sentence is synthesized as soon as it's written and they
// play in order, so she starts talking before the reply is finished.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let ready = false, installing = null, enabled = true, buffer = '', pending = 0, quiet = [];
let gen = 0, chain = Promise.resolve(), playing = null;
const progress = [];

invoke('voice_ready').then((r) => { ready = r; }).catch(() => {});
listen('voice', (e) => progress.forEach((fn) => fn(e.payload)));

// what she says, not how it looks: no mood tags, markdown, emoji or raw links
const clean = (t) => t.replace(/\[\w+\]/g, '').replace(/https?:\/\/\S+/g, 'a link').replace(/[*_#`>~|]/g, '')
  .replace(/\p{Extended_Pictographic}/gu, '').replace(/\s+/g, ' ').trim();

function settled() {
  if (pending > 0) return;
  const fns = quiet; quiet = [];
  fns.forEach((fn) => fn());
}

function play(b64, mine) {
  return new Promise((done) => {
    if (!b64 || mine !== gen) return done();
    playing = new Audio(`data:audio/wav;base64,${b64}`);
    playing.onended = playing.onerror = () => { playing = null; done(); };
    playing.play().catch(() => done());
  });
}

function speak(text) {
  const t = clean(text);
  if (!t || !enabled || !ready) return;
  const mine = gen;
  pending++;
  const audio = invoke('speak', { text: t }).catch(() => ''); // synthesize now, in parallel with playback
  chain = chain.then(() => audio).then((b64) => play(b64, mine)).finally(() => {
    if (mine === gen) { pending = Math.max(0, pending - 1); settled(); }
  });
}

export const name = () => (ready ? 'Kristin' : null);
export const speaking = () => pending > 0;

/// Downloads her voice the first time it's needed; onProgress gets 0-100.
export function install(onProgress) {
  if (ready) return Promise.resolve();
  if (!installing) {
    installing = invoke('install_voice').then(() => { ready = true; }).finally(() => { installing = null; });
  }
  if (onProgress) progress.push(onProgress);
  return installing;
}

export function setEnabled(on) {
  enabled = on;
  if (!on) stop();
}

// streamed text in; every finished sentence is spoken right away
export function feed(piece) {
  if (!enabled) return;
  buffer += piece;
  let m;
  while ((m = buffer.match(/^([\s\S]*?[.!?…:])\s+/))) {
    speak(m[1]);
    buffer = buffer.slice(m[0].length);
  }
}

// end of the reply: speak whatever is left
export function flush() {
  speak(buffer);
  buffer = '';
  settled();
}

export function stop() {
  buffer = '';
  gen++;
  pending = 0;
  playing?.pause();
  playing = null;
  chain = Promise.resolve();
  settled();
}

// runs fn once she has finished speaking (right away if she isn't)
export function whenQuiet(fn) {
  quiet.push(fn);
  settled();
}
