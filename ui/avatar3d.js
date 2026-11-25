// 3D avatar: a VRM model via three-vrm, with blinking, mouth flaps, expressions and hair physics.
import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { VRMLoaderPlugin, VRMUtils } from '@pixiv/three-vrm';

// state + emotion -> VRM expression targets
const STATES = {
  idle:     {},
  thinking: { relaxed: 0.4 },
  talking:  {},
  looking:  { surprised: 0.5 },
};
const EMOTIONS = {
  charming: { relaxed: 0.4, happy: 0.15 },
  joyful:   { happy: 0.6 },
  laughing: { happy: 1 },
  teasing:  { happy: 0.4, relaxed: 0.2 },
  angry:    { angry: 0.7 },
  sad:      { sad: 0.7 },
};
const lerp = (a, b, k) => a + (b - a) * k;

export async function start(el, buffer) {
  const canvas = document.createElement('canvas');
  canvas.setAttribute('data-tauri-drag-region', '');
  el.append(canvas);
  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 1.5));
  renderer.setClearColor(0x000000, 0);
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 20);
  scene.add(new THREE.AmbientLight(0xffffff, 1.2));
  const sun = new THREE.DirectionalLight(0xffffff, 1.6);
  sun.position.set(0.5, 1.5, 2);
  scene.add(sun);
  const lookTarget = new THREE.Object3D();
  scene.add(lookTarget);

  const resize = () => {
    renderer.setSize(el.clientWidth, el.clientHeight, false);
    camera.aspect = el.clientWidth / el.clientHeight;
    camera.updateProjectionMatrix();
  };
  addEventListener('resize', resize);
  resize();

  const loader = new GLTFLoader();
  loader.register((parser) => new VRMLoaderPlugin(parser));
  const vrm = (await loader.parseAsync(buffer, '')).userData.vrm;
  VRMUtils.removeUnnecessaryVertices(vrm.scene);
  VRMUtils.rotateVRM0(vrm); // VRM 0.x faces away from the camera
  scene.add(vrm.scene);
  vrm.lookAt.target = lookTarget;

  // T-pose -> relaxed arms
  const h = vrm.humanoid;
  h.getNormalizedBoneNode('leftUpperArm').rotation.z = -1.25;
  h.getNormalizedBoneNode('rightUpperArm').rotation.z = 1.25;

  // frame head and shoulders
  vrm.scene.updateMatrixWorld(true);
  const headY = h.getRawBoneNode('head').getWorldPosition(new THREE.Vector3()).y;
  camera.position.set(0, headY - 0.05, 1.15);
  camera.lookAt(0, headY - 0.15, 0);
  lookTarget.position.set(0, headY, 1.15);

  let state = 'idle', emotion = 'charming';
  const face = { happy: 0, relaxed: 0, surprised: 0, angry: 0, aa: 0, blink: 0 };
  let t = 0, nextBlink = 2, blinkAt = -1, mouseX = 0, mouseY = 0;

  function animate(dt) {
    t += dt;
    // blink: 80ms close, 80ms open, every 2-6s
    if (t > nextBlink) { blinkAt = t; nextBlink = t + 2 + Math.random() * 4; }
    const b = (t - blinkAt) / 0.08;
    face.blink = b < 1 ? b : b < 2 ? 2 - b : 0;

    const talk = state === 'talking' ? 0.25 + 0.6 * Math.abs(Math.sin(t * 13)) * (0.6 + 0.4 * Math.random()) : 0;
    face.aa = lerp(face.aa, talk, 0.5);

    const target = { ...EMOTIONS[emotion], ...STATES[state] };
    for (const k of ['happy', 'relaxed', 'surprised', 'angry']) face[k] = lerp(face[k], target[k] ?? 0, Math.min(1, dt * 4));
    for (const [k, v] of Object.entries(face)) vrm.expressionManager?.setValue(k, v);

    // breathing, sway, head tilt when thinking, follow the mouse
    h.getNormalizedBoneNode('spine').rotation.x = Math.sin(t * 1.6) * 0.015;
    const head = h.getNormalizedBoneNode('head');
    head.rotation.y = lerp(head.rotation.y, Math.sin(t * 0.5) * 0.05 + mouseX * 0.25, 0.1);
    head.rotation.z = lerp(head.rotation.z, state === 'thinking' ? 0.12 : Math.sin(t * 0.7) * 0.02, 0.05);
    head.rotation.x = lerp(head.rotation.x, mouseY * 0.12, 0.1);
    lookTarget.position.x = mouseX * 0.6;
    lookTarget.position.y = headY - mouseY * 0.4;

    vrm.update(dt); // expressions, look-at, hair/cloth spring bones
  }

  // 30 fps while active, 15 fps idle; rAF itself stops when the window is hidden
  let last = performance.now();
  function loop(now) {
    requestAnimationFrame(loop);
    if (now - last < 1000 / (state === 'idle' ? 15 : 30) - 1) return;
    const dt = Math.min((now - last) / 1000, 0.1);
    last = now;
    animate(dt);
    renderer.render(scene, camera);
  }
  requestAnimationFrame(loop);

  return {
    state(s) { state = s; },
    emotion(e) { if (EMOTIONS[e]) emotion = e; },
    // cursor position in window CSS px (can be far outside the window)
    look(x, y) {
      mouseX = Math.max(-1, Math.min(1, (x - innerWidth / 2) / 350));
      mouseY = Math.max(-1, Math.min(1, (y - innerHeight * 0.3) / 250));
    },
  };
}
