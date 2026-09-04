// FlyPet 主面板：亲密度/喂食/互动/道具 + 真实 GLB 待机展示 + 投喂按钮
import * as THREE from './vendor/three.module.js';
import { GLTFLoader } from './vendor/GLTFLoader.js';

const isTauri = !!window.__TAURI__;
const $ = (id) => document.getElementById(id);

const SPECIES_LABEL = {
  butterfly: '蝴蝶', dragonfly: '蜻蜓', fly: '苍蝇', spider: '蜘蛛',
  bee: '蜜蜂', mantis: '螳螂', longhorn: '天牛'
};

const RARITY_META = {
  fan:  { label: '凡品', ico: '🍂' },
  ling: { label: '灵品', ico: '🌿' },
  xuan: { label: '玄品', ico: '🔷' },
  di:   { label: '地品', ico: '🟣' },
  tian: { label: '天品', ico: '✨' },
  xian: { label: '仙品', ico: '🌠' },
  shen: { label: '神品', ico: '👑' }
};

const SPECIES_MODELS = {
  butterfly: {
    url: 'models/butterfly.glb',
    clipKeys: ['|3', 'stand', 'idle'],
    dim: 1.15
  },
  dragonfly: {
    url: 'models/dragonfly.glb',
    clipKeys: ['stand', 'idle'],
    dim: 1.35
  },
  fly: {
    url: 'models/fly.glb',
    clipKeys: ['idle1', 'idle'],
    dim: 0.604
  }
};

// ---------- Three.js 待机展示 ----------
const cv = $('pet');
const stage = cv.parentElement;
const W = () => stage.clientWidth || 300;
const H = () => stage.clientHeight || 150;

const renderer = new THREE.WebGLRenderer({ canvas: cv, alpha: true, antialias: true });
renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
renderer.setClearColor(0x000000, 0);
renderer.shadowMap.enabled = true;
renderer.shadowMap.type = THREE.PCFSoftShadowMap;

const scene = new THREE.Scene();
const cam = new THREE.PerspectiveCamera(40, W() / H(), 0.1, 50);

scene.add(new THREE.HemisphereLight(0xffffff, 0x88775a, 1.35));
const key = new THREE.DirectionalLight(0xfff6e0, 1.35);
key.position.set(-1.6, 3.4, 2.2);
key.castShadow = true;
key.shadow.mapSize.set(512, 512);
Object.assign(key.shadow.camera, { left: -2, right: 2, top: 2, bottom: -2, near: 0.1, far: 10 });
key.shadow.bias = -0.002;
scene.add(key);
scene.add(key.target);

const fill = new THREE.DirectionalLight(0x88aacc, 0.45);
fill.position.set(2.2, 1.2, -1.6);
scene.add(fill);

const ground = new THREE.Mesh(
  new THREE.PlaneGeometry(6, 6),
  new THREE.ShadowMaterial({ opacity: 0.22 })
);
ground.rotation.x = -Math.PI / 2;
ground.position.y = -0.55;
ground.receiveShadow = true;
scene.add(ground);

function resize() {
  const w = W(), h = H();
  renderer.setSize(w, h, false);
  cam.aspect = w / Math.max(1, h);
  cam.updateProjectionMatrix();
}
resize();
window.addEventListener('resize', resize);

// 强制允许上下滑动
document.body.style.overflow = 'auto';
document.body.style.height = '100vh';
document.body.style.minHeight = '100vh';

const panel = document.querySelector('.pet-stage, .stats, .actions, .msg');
if (panel) {
  panel.style.overflow = 'auto';
  panel.style.maxHeight = '100vh';
  panel.style.height = 'auto';
}

const loader = new GLTFLoader();
let petRoot = null;
let mixer = null;
let activeAction = null;
let clips = [];
let currentSpecies = null;
let loadToken = 0;
let yaw = 0.35;
const clock = new THREE.Clock();

function findClip(keys) {
  for (const k of keys) {
    const hit = clips.find(c => c.name && c.name.toLowerCase().includes(k.toLowerCase()));
    if (hit) return hit;
  }
  return clips[0] || null;
}

function playIdle() {
  if (!mixer) return;
  const spec = SPECIES_MODELS[currentSpecies] || SPECIES_MODELS.butterfly;
  const clip = findClip(spec.clipKeys);
  if (!clip) return;
  if (activeAction && activeAction._clip === clip) return;
  if (activeAction) activeAction.fadeOut(0.15);
  const action = mixer.clipAction(clip);
  action.reset();
  action.setLoop(THREE.LoopRepeat);
  action.fadeIn(0.2);
  action.play();
  activeAction = action;
}

function disposeRoot(root) {
  if (!root) return;
  scene.remove(root);
  root.traverse(o => {
    if (o.geometry) o.geometry.dispose();
    const mats = Array.isArray(o.material) ? o.material : [o.material];
    for (const m of mats) if (m && m.dispose) m.dispose();
  });
}

function setupModel(gltf, speciesId) {
  const root = gltf.scene;
  root.traverse(o => {
    if (o.scale && Math.abs(o.scale.x - 0.01) < 0.002 && o.isBone !== true) {
      o.scale.set(1, 1, 1);
    }
  });
  root.traverse(o => {
    if (!o.isMesh) return;
    o.castShadow = true;
    const mats = Array.isArray(o.material) ? o.material : [o.material];
    for (const m of mats) {
      if (m && (m.isMeshStandardMaterial || m.isMeshPhysicalMaterial) && m.metalness > 0.4) {
        m.metalness = 0.15;
        m.roughness = Math.max(m.roughness, 0.7);
      }
    }
  });

  disposeRoot(petRoot);
  if (mixer) mixer.stopAllAction();

  petRoot = root;
  scene.add(petRoot);
  clips = gltf.animations || [];
  mixer = new THREE.AnimationMixer(petRoot);
  activeAction = null;
  currentSpecies = speciesId;

  root.updateMatrixWorld(true);
  root.traverse(o => { if (o.isSkinnedMesh && o.skeleton) o.skeleton.update(); });

  const box = new THREE.Box3().setFromObject(root, true);
  const size = new THREE.Vector3();
  box.getSize(size);
  const maxDim = Math.max(size.x, size.y, size.z) || 1;
  const dim = (SPECIES_MODELS[speciesId] && SPECIES_MODELS[speciesId].dim) || 1.15;
  root.scale.setScalar(dim / maxDim);
  root.updateMatrixWorld(true);
  const box2 = new THREE.Box3().setFromObject(root, true);
  const c = new THREE.Vector3();
  box2.getCenter(c);
  root.position.sub(c);
  root.position.y += 0.08;
  root.updateMatrixWorld(true);

  playIdle();
}

function loadSpeciesModel(speciesId) {
  const loadId = SPECIES_MODELS[speciesId] ? speciesId : 'butterfly';
  if (loadId === currentSpecies && petRoot) return;
  const spec = SPECIES_MODELS[loadId];
  const token = ++loadToken;
  fetch(spec.url)
    .then(r => r.arrayBuffer())
    .then(buf => {
      if (token !== loadToken) return;
      loader.parse(buf, '', (gltf) => {
        if (token !== loadToken) return;
        setupModel(gltf, loadId);
      }, (e) => console.error('[panel] parse 失败:', e));
    })
    .catch(e => console.error('[panel] 模型加载失败:', e));
}

function setSpeciesLabel(id) {
  $('speciesName').textContent = SPECIES_LABEL[id] || '蝴蝶';
  $('speciesName').title = '成长数据按宠物独立，每只各自积累';
}

function loop() {
  requestAnimationFrame(loop);
  const dt = Math.min(0.05, clock.getDelta());
  if (mixer) mixer.update(dt);
  yaw += dt * 0.35;
  if (petRoot) petRoot.rotation.y = yaw;
  // 侧上俯视，模型居中。画布加高后宽高比变小（横向视野收窄），
  // 需拉远保证展翅完整；elev 稍降使模型投影更居中不顶边
  const dist = 3.15;
  const elev = 0.62;
  cam.position.set(Math.sin(0.15) * dist * 0.15, elev, dist);
  cam.lookAt(0, 0.05, 0);
  renderer.render(scene, cam);
}
loop();

// ---------- 数据加载 ----------
async function loadInfo() {
  let info = null;
  if (isTauri) {
    try { info = await window.__TAURI__.core.invoke('feed_info'); } catch (e) {}
  }
  if (!info) info = { affinity: 0, affinityLevel: 1, fedCount: 0, interactCount: 0, durationSecs: 0, ageSecs: 0, lastItem: null };
  $('affinity').textContent = info.affinity;
  $('affinityLv').textContent = 'Lv.' + (info.affinityLevel || 1);
  $('fedCount').textContent = info.fedCount;
  $('interactCount').textContent = info.interactCount;
  $('duration').textContent = fmtDuration(info.durationSecs || 0);
  $('age').textContent = fmtAge(info.ageSecs || 0);
  const card = $('itemCard');
  if (info.lastItem) {
    const meta = RARITY_META[info.lastItem.rarity] || RARITY_META.fan;
    $('itemName').textContent = info.lastItem.name;
    $('itemName').className = 'nm rarity-' + info.lastItem.rarity;
    $('itemRarity').textContent = meta.label + ' · 亲密度 +' + affinityGain(info.lastItem.rarity);
    card.querySelector('.ico').textContent = meta.ico;
  } else {
    $('itemName').textContent = '尚未喂食';
    $('itemName').className = 'nm';
    $('itemRarity').textContent = '喂食后这里显示获得的道具';
    card.querySelector('.ico').textContent = '🍯';
  }
  updateFeedBtn(info);
  return info;
}

// ---------- 投喂按钮冷却倒计时 ----------
let feedCooldownTimer = null;

function updateFeedBtn(info) {
  const btn = $('btnFeed');
  clearInterval(feedCooldownTimer);
  feedCooldownTimer = null;
  if (info && !info.canFeed && info.remainingSec > 0) {
    let remain = info.remainingSec;
    const render = () => {
      const m = Math.floor(remain / 60), s = remain % 60;
      btn.textContent = `⏳ ${m}:${String(s).padStart(2, '0')}`;
    };
    render();
    btn.disabled = true;
    feedCooldownTimer = setInterval(() => {
      remain--;
      if (remain <= 0) {
        clearInterval(feedCooldownTimer);
        feedCooldownTimer = null;
        btn.disabled = false;
        btn.textContent = '🍽️ 投喂';
        return;
      }
      render();
    }, 1000);
  } else {
    btn.disabled = false;
    btn.textContent = '🍽️ 投喂';
  }
}

function affinityGain(r) {
  return { fan: 2, ling: 4, xuan: 8, di: 16, tian: 32, xian: 64, shen: 128 }[r] || 2;
}

// 相处时长格式化：秒 → 「X 天 X 小时 / X 小时 X 分 / X 分钟 / X 秒」
function fmtDuration(totalSecs) {
  const s = Math.floor(totalSecs);
  if (s < 60) return s + ' 秒';
  const m = Math.floor(s / 60);
  if (m < 60) return m + ' 分钟';
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? h + ' 小时 ' + (m % 60) + ' 分' : h + ' 小时';
  const d = Math.floor(h / 24);
  return h % 24 ? d + ' 天 ' + (h % 24) + ' 小时' : d + ' 天';
}

// 存活格式化：单位随周期自动升级（分钟 → 小时 → 天 → 年），只显示最大单位
// 例：1 分钟 / 5 小时 / 22 天 / 1.2 年（年保留 1 位小数）
function fmtAge(totalSecs) {
  const s = Math.floor(totalSecs);
  if (s < 3600) return Math.max(1, Math.floor(s / 60)) + ' 分钟';
  if (s < 86400) return Math.floor(s / 3600) + ' 小时';
  const days = s / 86400;
  if (days < 365) return Math.floor(days) + ' 天';
  return (days / 365).toFixed(1) + ' 年';
}

function showMsg(text) {
  const m = $('msg');
  m.textContent = text;
  m.classList.add('show');
  clearTimeout(showMsg._t);
  showMsg._t = setTimeout(() => m.classList.remove('show'), 3500);
}

$('btnFeed').addEventListener('click', async () => {
  const btn = $('btnFeed');
  btn.disabled = true;
  try {
    if (isTauri) {
      const info = await window.__TAURI__.core.invoke('feed');
      if (info && info.lastItem) {
        const meta = RARITY_META[info.lastItem.rarity] || RARITY_META.fan;
        showMsg(`获得 ${meta.label}·${info.lastItem.name}，亲密度 +${affinityGain(info.lastItem.rarity)}`);
      } else if (info && !info.canFeed) {
        showMsg(`小家伙还没吃完，约 ${Math.max(1, Math.ceil((info.remainingSec || 0) / 60))} 分钟后再投喂`);
      }
    } else {
      showMsg('浏览器预览：获得 灵品·聚气丹');
    }
    await loadInfo();
  } catch (e) {
    console.error('[main] feed failed', e);
    btn.disabled = false;
  }
});

(async () => {
  let app = { species: 'butterfly', seed: 42, stage: 3 };
  if (isTauri) {
    try { app = await window.__TAURI__.core.invoke('pet_appearance'); } catch (e) {}
    try { window.__TAURI__.event.listen('profile-updated', () => loadInfo()); } catch (e) {}
    try {
      window.__TAURI__.event.listen('state', (ev) => {
        const s = ev.payload;
        if (s && s.species && s.species !== currentSpecies) {
          setSpeciesLabel(s.species);
          loadSpeciesModel(s.species);
          loadInfo();
        }
      });
    } catch (e) {}
  }
  setSpeciesLabel(app.species);
  loadSpeciesModel(app.species);
  await loadInfo();
})();
