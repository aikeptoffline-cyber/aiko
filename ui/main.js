import * as voice from './voice.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

// ---------------------------------------------------------------- avatar: 3D if a VRM was dropped in, else 2D sprites

let avatar;
try {
  const vrm = await invoke('load_vrm');
  avatar = await (await import('./avatar3d.js')).start($('avatar'), vrm);
} catch {
  $('avatar').replaceChildren();
  avatar = (await import('./avatar2d.js')).start($('avatar'));
}

addEventListener('dragover', (e) => e.preventDefault());
// images to show her: paste (Ctrl+V) or drop them; shrunk to 1024 px JPEG like her screenshots
const pending = [];
async function addImage(file) {
  if (pending.length >= 4) return say('Four pictures at a time. I only have two eyes.');
  const img = await createImageBitmap(file);
  const k = Math.min(1, 1024 / Math.max(img.width, img.height));
  const c = document.createElement('canvas');
  c.width = Math.round(img.width * k);
  c.height = Math.round(img.height * k);
  c.getContext('2d').drawImage(img, 0, 0, c.width, c.height);
  const url = c.toDataURL('image/jpeg', 0.85);
  pending.push(url.split(',')[1]);
  const thumb = Object.assign(document.createElement('img'), { src: url, title: 'Click to remove' });
  thumb.onclick = () => { pending.splice([...$('shots').children].indexOf(thumb), 1); thumb.remove(); };
  $('shots').append(thumb);
  $('text').focus();
}
addEventListener('paste', (e) => {
  const files = [...e.clipboardData.files].filter((f) => f.type.startsWith('image/'));
  if (!files.length) return;
  e.preventDefault();
  files.forEach((f) => addImage(f).catch((err) => say(String(err))));
});
addEventListener('dragover', (e) => e.preventDefault());

addEventListener('drop', async (e) => {
  e.preventDefault();
  const images = [...e.dataTransfer.files].filter((f) => f.type.startsWith('image/'));
  if (images.length) return images.forEach((f) => addImage(f).catch((err) => say(String(err))));
  const file = e.dataTransfer.files[0];
  if (!file?.name.toLowerCase().endsWith('.vrm')) return say('Drop a picture to show me, or a .vrm file to switch me to 3D.');
  await invoke('save_vrm', new Uint8Array(await file.arrayBuffer())).catch((err) => say(String(err)));
  location.reload();
});

invoke('name').then((n) => { $('name').textContent = n; });

// ---------------------------------------------------------------- chat

const bubble = $('bubble');
let hideTimer;
function say(text, append = false) {
  bubble.hidden = false;
  // a reply that continues after a tool call can start again with a mood tag; don't show those
  bubble.textContent = (append ? bubble.textContent + text : text).replace(/\[(joyful|laughing|charming|teasing|surprised|sad|angry)\]\s*/gi, '');
  bubble.scrollTop = bubble.scrollHeight;
  hideLater();
}
// the reply stays long enough to read (about 12 characters a second, at least 20 s) and never hides
// while the mouse is on it; the 💬 history keeps everything anyway
function hideLater() {
  clearTimeout(hideTimer);
  if (bubble.matches(':hover')) return;
  hideTimer = setTimeout(() => { bubble.hidden = true; }, Math.max(20000, bubble.textContent.length * 85));
}
bubble.addEventListener('mouseenter', () => clearTimeout(hideTimer));
bubble.addEventListener('mouseleave', hideLater);

async function showHistory() {
  const log = $('history').querySelector('.log');
  const name = $('name').textContent;
  log.replaceChildren(...(await invoke('history')).map(([role, text]) => {
    const msg = document.createElement('div');
    msg.className = `msg ${role}`;
    const who = document.createElement('span');
    who.className = 'who';
    who.textContent = role === 'user' ? 'You' : name;
    msg.append(who, text);
    return msg;
  }));
  $('history').hidden = false;
  log.scrollTop = log.scrollHeight;
}
$('chat').onclick = () => ($('history').hidden ? showHistory() : ($('history').hidden = true));
$('histclose').onclick = () => { $('history').hidden = true; };
function setState(s, status = '') { avatar.state(s); $('status').textContent = status; }

// Replies start with a mood tag like "[joyful]"; peel it off the stream before showing text.
let head = null, forcedMood = null; // the app picks her face when you sound down
const reply = (t) => { say(t, true); voice.feed(t); };
function onToken(piece) {
  if (head === null) return reply(piece);
  head += piece;
  const m = head.match(/^\s*\[(\w+)\]\s*/);
  if (m) {
    avatar.emotion(forcedMood ?? m[1].toLowerCase());
    reply(head.slice(m[0].length));
    head = null;
  } else if (!/^\s*\[\w*$/.test(head)) { // not a tag after all
    reply(head);
    head = null;
  }
}
// she keeps her talking animation until she has finished speaking
const settle = () => { voice.flush(); voice.whenQuiet(() => { if (!busy) setState('idle'); }); };

let busy = false;
const MUFFLED = ['Mmmph!', 'Mmf mmmf mmph.', 'Hmmmph! 🤐', 'Mm-mmph mmf!'];
async function send(text, screen, shared = []) {
  if (busy || !text) return;
  // taped: she can't answer, and the model isn't even asked
  if (voiceMode === 'taped') {
    avatar.emotion('angry');
    return say(`${MUFFLED[Math.floor(Math.random() * MUFFLED.length)]}  (take the tape off with 🤐 to hear her)`);
  }
  busy = true;
  asleep = false;
  voice.stop();
  say('');
  head = '';
  forcedMood = null;
  setState('thinking', 'thinking…');
  try {
    await invoke('chat', { text, screen, shared });
  } catch (err) {
    say(`(${err} - is Ollama running?)`);
  } finally {
    busy = false;
    settle();
  }
}

const STATUS = { looking: 'looking at your screen…', thinking: 'thinking…', searching: 'searching the web…', recalling: 'checking my notes…', working: 'noting that down…' };
listen('status', (e) => {
  if (e.payload === 'working' && voice.speaking()) return; // saving memories happens while she's still talking
  setState(e.payload, STATUS[e.payload]);
});
listen('mood', (e) => { forcedMood = e.payload; avatar.emotion(e.payload); });
listen('token',(e) => { setState('talking'); onToken(e.payload); });
listen('done', settle);
listen('brain', (e) => { $('status').textContent = `learned ${e.payload} new note(s)`; });
listen('memorized',(e) => { if (e.payload > 0) $('status').textContent = `noted ${e.payload} thing(s) about you`; });

$('ask').addEventListener('submit', async (e) => {
  e.preventDefault();
  const text = $('text').value.trim();
  $('text').value = '';
  if (text.startsWith('/remember ')) {
    const ok = await invoke('remember', { text: text.slice(10) }).catch((err) => say(String(err)));
    return say(ok ? 'Got it, I will remember that.' : 'I already knew that.');
  }
  if (text === '/brain') {
    const [dir, files, chunks] = await invoke('brain_info');
    return say(`My second brain is ${dir}: ${files} note(s), ${chunks} passages indexed. Drop .md or .txt files in there and I'll learn them within a minute.`);
  }
  if (busy) return;
  const shared = pending.splice(0);
  $('shots').replaceChildren();
  send(text || (shared.length ? 'What do you make of this?' : ''), $('screen').checked, shared);
});

// AI model switch (2B <-> 4B); the backend downloads a model the first time and remembers the choice
async function showModel() {
  const [current, list] = await invoke('models');
  const now = list.find((m) => m[1] === current);
  $('model').textContent = now?.[0] ?? '?';
  const next = list[(list.indexOf(now) + 1) % list.length];
  $('model').title = `Brain size: ${now?.[0] ?? current}. Click to switch to ${next[0]}${next[3] ? '' : ` (downloads ${next[2]} once)`}`;
  return { list, now, next };
}
showModel().catch(() => {});
listen('pull', (e) => { $('status').textContent = `downloading model… ${e.payload}%`; });
$('model').onclick = async () => {
  if (busy) return;
  const { next } = await showModel();
  busy = true;
  setState('thinking', next[3] ? `switching to ${next[0]}…` : `downloading ${next[0]} (${next[2]})…`);
  try {
    await invoke('set_model', { model: next[1] });
    say(`Switched to the ${next[0]} model.`);
  } catch (err) {
    say(`Couldn't switch models: ${err}`);
  } finally {
    busy = false;
    setState('idle');
    showModel().catch(() => {});
  }
};

// voice: on (speaks) -> quiet (text only) -> taped (silent: no voice, no comments of her own), remembered
const VOICE = {
  on: { icon: '🔊', next: 'quiet', tip: () => (voice.name() ? `Voice on (${voice.name()}). Click for quiet: text only`
    : 'Voice on (getting her voice ready). Click for quiet: text only') },
  quiet: { icon: '🔇', next: 'taped', tip: () => 'Quiet: she replies in text, no audio. Click to tape her mouth' },
  taped: { icon: '🤐', next: 'on', tip: () => 'Taped: no voice and no comments of her own. Click to let her speak' },
};
let voiceMode = 'on';
try { voiceMode = { off: 'quiet' }[localStorage.getItem('voice')] ?? localStorage.getItem('voice') ?? 'on'; } catch {}
if (!VOICE[voiceMode]) voiceMode = 'on';
const showVoice = () => {
  $('voice').textContent = VOICE[voiceMode].icon;
  $('voice').title = VOICE[voiceMode].tip();
  voice.setEnabled(voiceMode === 'on');
  avatar.wear?.('tape', voiceMode === 'taped');
};
// her voice is downloaded the first time it's switched on (about 21 MB, once)
async function getVoice() {
  if (voiceMode !== 'on' || voice.name()) return;
  try {
    await voice.install((p) => { $('status').textContent = `downloading my voice… ${p}%`; });
    $('status').textContent = 'voice ready';
  } catch (err) {
    say(`I couldn't download my voice (${err}). I'll stick to text for now.`);
  }
  showVoice();
}
showVoice();
setTimeout(getVoice, 1500); // voice_ready answers first
$('voice').onclick = () => {
  voiceMode = VOICE[voiceMode].next;
  if (voiceMode !== 'on') voice.stop();
  try { localStorage.setItem('voice', voiceMode); } catch {}
  showVoice();
  getVoice();
};

$('brain').onclick =() => invoke('open_brain').catch((err) => say(String(err)));

// eyes and head follow the cursor anywhere on screen; she falls asleep when it hasn't moved for 5 minutes
let asleep = false, lastPos = null, lastMove = Date.now();
setInterval(async () => {
  const c = await invoke('cursor').catch(() => null);
  if (!c) return;
  if (!lastPos || Math.abs(c[0] - lastPos[0]) + Math.abs(c[1] - lastPos[1]) > 2) {
    lastPos = c;
    lastMove = Date.now();
    if (asleep) { asleep = false; setState('idle'); avatar.emotion('joyful'); }
  } else if (!asleep && !busy && !voice.speaking() && Date.now() - lastMove > 5 * 60000) {
    asleep = true;
    setState('sleeping', 'zzz…');
  }
  if (!asleep) avatar.look(c[0], c[1]);
}, 80);

// glance at the screen now and then and react if something's worth it; the backend skips the model
// when the screen hasn't changed, and she doesn't glance while asleep
let watching = true;
try { watching = localStorage.getItem('watch') !== 'off'; } catch {}
$('watch').classList.toggle('on', watching);
$('watch').onclick = () => {
  watching = !watching;
  $('watch').classList.toggle('on', watching);
  try { localStorage.setItem('watch', watching ? 'on' : 'off'); } catch {}
};

// opt-in screen journal: each glance at a changed screen adds one line about what you're doing to the second brain
let journaling = false;
try { journaling = localStorage.getItem('journal') === 'on'; } catch {}
$('journal').classList.toggle('on', journaling);
$('journal').onclick = async () => {
  journaling = !journaling;
  $('journal').classList.toggle('on', journaling);
  try { localStorage.setItem('journal', journaling ? 'on' : 'off'); } catch {}
  if (!journaling) return say('Screen journal off. I won\'t write down what you do anymore.');
  const [dir] = await invoke('brain_info');
  say(`Screen journal on. When your screen changes, I'll add one line about what you're doing (app and task, never passwords, messages, names or numbers) to ${dir}\\Screen journal\\<date>.md. You can read, edit or delete it any time. Click 📓 again to stop.`);
};

// every 45 s, or every 15 s while you're playing a game so she can cheer and groan along
let gaming = false;
async function glance() {
  if (((!watching || voiceMode === 'taped') && !journaling) || busy || asleep) return;
  const d = new Date();
  const args = { chime: watching && voiceMode !== 'taped', journal: journaling, date: d.toLocaleDateString('sv-SE'), time: d.toTimeString().slice(0, 5) };
  const result = await invoke('react', args).catch((err) => { $('status').textContent = `glance failed: ${err}`; });
  gaming = !!result?.game;
  if (!result?.reaction || busy) return;
  say('');
  head = '';
  onToken(result.reaction);
  setState('talking');
  voice.flush();
  voice.whenQuiet(() => setTimeout(() => { if (!busy) setState('idle'); }, 1500));
}
(function glanceLoop() {
  setTimeout(async () => { await glance(); glanceLoop(); }, gaming ? 15000 : 45000);
})();

$('look').onclick =() => send('Take a look at my screen. What am I doing, and can you help with anything?', true);
// ☰ menu: incognito, forgetting over time, quit
const openMenu = (open) => {
  $('menu').hidden = !open;
  $('more').classList.toggle('on', open);
  $('more').setAttribute('aria-expanded', open);
};
$('more').onclick = (e) => { e.stopPropagation(); openMenu($('menu').hidden); };
addEventListener('click', (e) => { if (!$('menu').hidden && !$('menu').contains(e.target)) openMenu(false); });
$('quit').onclick = () => window.__TAURI__.window.getCurrentWindow().close();

// incognito: a chat she keeps nothing from; it ends when you turn it off or close her
let incognito = false;
$('incognito').onclick = async () => {
  if (busy) return;
  incognito = !incognito;
  await invoke('set_incognito', { on: incognito });
  $('incognito').classList.toggle('on', incognito);
  $('incognito').textContent = `🕶 Incognito: ${incognito ? 'on' : 'off'}`;
  document.body.classList.toggle('incognito', incognito);
  avatar.wear?.('sunglasses', incognito);
  $('text').placeholder = incognito ? 'Incognito: I keep nothing…' : 'Talk to me…';
  $('history').hidden = true;
  say(incognito ? "Incognito on. I won't keep this chat, learn anything about you, save web notes or write the journal. Turn it off and it's gone."
    : 'Incognito off. That chat is gone; back to our usual one.');
};

// forget over time: memories and chat older than the choice are deleted, checked every minute
const FORGET = { never: 'never forget', day: 'forget anything older than a day', week: 'forget anything older than a week',
  month: 'forget anything older than a month', year: 'forget anything older than a year' };
const showForget = (v) => $('forget').querySelectorAll('button').forEach((b) => {
  b.classList.toggle('on', b.dataset.v === v);
  b.setAttribute('aria-checked', b.dataset.v === v);
});
invoke('forget_after').then(showForget).catch(() => {});
$('forget').onclick = async (e) => {
  const v = e.target.dataset?.v;
  if (!v) return;
  const gone = await invoke('set_forget_after', { choice: v }).catch((err) => say(String(err)));
  if (gone === undefined) return;
  showForget(v);
  say(`Okay, I'll ${FORGET[v]}.${gone ? ` That just cleared ${gone} old memories and messages.` : ''}`);
};

async function showMemories() {
  const list = $('memories').querySelector('ul');
  const mems = await invoke('list_memories');
  list.replaceChildren(...(mems.length ? mems.map((m, i) => {
    const li = document.createElement('li');
    const span = document.createElement('span');
    span.textContent = m;
    const del = document.createElement('button');
    del.textContent = 'Forget';
    del.onclick = async () => { await invoke('forget', { index: i }); showMemories(); };
    li.append(span, del);
    return li;
  }) : [Object.assign(document.createElement('li'), { textContent: 'Nothing yet. Chat with me, or type /remember <fact>.' })]));
  $('memories').hidden = false;
}
$('mem').onclick = showMemories;
$('memclose').onclick = () => { $('memories').hidden = true; };
