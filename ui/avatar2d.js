// 2D avatar: poses on one shared canvas, each drawn in three soft-masked bands so she can move like a rig:
// the waist stays put, chest and shoulders breathe, the head rides on top, sways and turns to the cursor,
// and the irises (masked to the eye whites) look at it. Moods cross-fade; blinks and mouth flaps are instant.
const MOODS = { charming: 'master', joyful: 'joyful', laughing: 'laughing', teasing: 'teasing', angry: 'angry', surprised: 'surprised', sad: 'sad' };
const ACTIVITIES = { thinking: 'thinking', recalling: 'thinking', looking: 'tablet', searching: 'tablet', working: 'tablet', sleeping: 'sleepy' }; // state -> pose
const TALK = { master: 'talk', teasing: 'teasing_talk', angry: 'angry_talk', tablet: 'tablet_talk' }; // pose -> mouth-open pose
const POSES = ['master', 'blink', ...Object.values(MOODS).slice(1), 'thinking', 'tablet', 'sleepy', ...Object.values(TALK)];
// accessories she can wear over any pose: sprites/acc-<name>.png, cut from art by art/accessories.ps1
const ACCESSORIES = ['sunglasses', 'tape'];
const ARMED = new Set(['master', 'talk', 'blink']); // drawn armless (art/arms.ps1); separate arms swing on top
const CANVAS = [687, 1024]; // source canvas, px
const EYE_Y = 0.26; // eye height as a fraction of the canvas, for aiming the look

export function start(el) {
  el.classList.add('sprite');
  el.innerHTML = `<div class="rig">
    <div class="band lower"></div>
    <div class="band upper breathe"></div>
    <div class="arms breathe"><div class="arm l"><img src="sprites/arm_l.png" alt="" draggable="false"></div>
      <div class="arm r"><img src="sprites/arm_r.png" alt="" draggable="false"></div></div>
    <div class="breathe head-ride"><div class="sway"><div class="band head"></div></div></div>
  </div>`;
  const rig = el.querySelector('.rig');
  const bands = [...el.querySelectorAll('.band')];
  const poses = {};
  for (const p of POSES) {
    poses[p] = bands.map((band) => {
      const pose = document.createElement('div');
      pose.className = 'pose';
      pose.innerHTML = `<img src="sprites/${p}${ARMED.has(p) ? '_noarms' : ''}.png" alt="" draggable="false" data-tauri-drag-region>` + (band.classList.contains('head')
        ? `<div class="eyes" style="-webkit-mask-image: url(sprites/${p}-eyes.png); mask-image: url(sprites/${p}-eyes.png)">
             <img src="sprites/${p}-iris.png" alt="" draggable="false"></div>` : '');
      band.append(pose);
      return pose;
    });
  }
  // accessories ride on the head band, above every pose, so they turn and sway with her
  const acc = {};
  for (const a of ACCESSORIES) {
    acc[a] = Object.assign(document.createElement('img'), { className: 'acc', src: `sprites/acc-${a}.png`, alt: '', draggable: false });
    el.querySelector('.band.head').append(acc[a]);
  }
  // shoulder pivots, as canvas fractions
  fetch('sprites/arms.json').then((r) => r.json()).then((piv) => {
    for (const side of ['l', 'r']) el.querySelector(`.arm.${side}`).style.transformOrigin = `${piv[side][0] * 100}% ${piv[side][1] * 100}%`;
  }).catch(() => {});
  const fit = () => el.style.setProperty('--s', rig.clientWidth / CANVAS[0]);
  addEventListener('resize', fit);
  fit();

  let current = 'master', mood = 'charming', state = 'idle', sawScreen = false, calmTimer, taped = false;
  poses.master.forEach((q) => q.classList.add('on'));
  rig.classList.add('armed');
  function show(p, instant = false) {
    if (!poses[p] || p === current) return;
    for (const q of [...poses[current], ...poses[p]]) q.classList.toggle('instant', instant);
    poses[current].forEach((q) => q.classList.remove('on'));
    poses[p].forEach((q) => q.classList.add('on'));
    current = p;
    rig.classList.toggle('armed', ARMED.has(p));
  }
  // replies about the screen keep the tablet out while she talks
  const base = () => (state === 'talking' && sawScreen ? 'tablet' : ACTIVITIES[state] ?? MOODS[mood]);

  // blink every 2-6s, only on the resting face (other poses have their own eyes)
  (function blink() {
    setTimeout(() => {
      if (current === 'master') {
        show('blink', true);
        setTimeout(() => { if (current === 'blink') show('master', true); }, 130);
      }
      blink();
    }, 2000 + Math.random() * 4000);
  })();

  // mouth flaps while talking, on poses that have a mouth-open twin; the rest talk with a head nod
  setInterval(() => {
    const b = base(), open = TALK[b];
    if (state !== 'talking' || !open || taped) return; // taped: no mouth under there to flap
    show(current === open ? b : open, true);
  }, 140);

  const clamp = (v) => Math.max(-1, Math.min(1, v));
  return {
    // put an accessory on or take it off (names from ACCESSORIES)
    wear(name, on) {
      acc[name]?.classList.toggle('on', on);
      if (name === 'tape') taped = on;
    },
    state(s) { // idle | thinking | looking | working | talking | sleeping
      if (s === 'looking' || s === 'searching') sawScreen = true;
      if (s === 'idle' || s === 'sleeping') sawScreen = false;
      state = s;
      el.dataset.state = s;
      show(base());
    },
    emotion(e) {
      if (!MOODS[e]) return;
      mood = e;
      show(base());
      clearTimeout(calmTimer);
      calmTimer = setTimeout(() => { mood = 'charming'; show(base()); }, 30000);
    },
    // cursor position in window CSS px (can be far outside the window)
    look(x, y) {
      const r = rig.getBoundingClientRect();
      const eyeY = r.top + r.height * EYE_Y;
      el.style.setProperty('--lx', clamp((x - (r.left + r.width / 2)) / 400).toFixed(3));
      el.style.setProperty('--ly', clamp((y - eyeY) / 300).toFixed(3));
    },
  };
}
