// FlyPet 正式版宠物渲染器
// 职责：加载 GLB 模型 → 绑定三动画（待机/飞行/滑行）→ 消费 Rust state 事件驱动姿态
// 无 Tauri 环境（浏览器直开）时进入演示模式：内置小型状态机随机切换姿态
import * as THREE from './vendor/three.module.js';
import { GLTFLoader } from './vendor/GLTFLoader.js';


// 透明模式：canvas 与 body 全透明（Tauri 里叠在壁纸上；浏览器预览加浅底方便观察）
const isTauriEnv = !!window.__TAURI__;
if (!isTauriEnv) {
  document.body.style.background = '#2a3038';
  document.documentElement.style.background = '#2a3038';
} else {
  document.body.style.background = 'transparent';
  document.documentElement.style.background = 'transparent';
}

/* ======================= 1. 场景 ======================= */
// 浏览器预览辅助：?w=&h=&dist=&elev= 放大窗口 / 拉远相机，方便检查模型形态
const _q = new URLSearchParams(location.search);
const W = parseInt(_q.get('w') || '140', 10);
const H = parseInt(_q.get('h') || '160', 10);
const cv = document.getElementById('view');
cv.style.width = W + 'px';
cv.style.height = H + 'px';
let renderer;
try {
  renderer = new THREE.WebGLRenderer({ canvas: cv, alpha: true, antialias: true });
} catch (e) {
  throw e;
}
renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
renderer.setSize(W, H, false);
renderer.setClearColor(0x000000, 0);

const scene = new THREE.Scene();
// FOV 44°：窗口 140×160px（≈2.33×2.67 场景单位）在 CAM_DIST 3.3 处全可见，
// 蝴蝶贴边（窗口边缘）时不会超出视野消失。
const cam = new THREE.PerspectiveCamera(44, W / H, 0.1, 100);

/* 真实阴影：主平行光开 castShadow，地面用 ShadowMaterial 接收。
   影子随模型轮廓（翅膀扇动等）变化，比圆片真实。
   影子相机跟随模型 x/z（模型在窗口内平移 + 飞行），否则飞高/飞偏影子掉出 map。 */
renderer.shadowMap.enabled = true;
renderer.shadowMap.type = THREE.PCFSoftShadowMap;

scene.add(new THREE.HemisphereLight(0xffffff, 0x223344, 1.5));
const key = new THREE.DirectionalLight(0xffffff, 1.5);
key.position.set(-2, 4, 2);
key.castShadow = true;
key.shadow.mapSize.set(1024, 1024);
// 只让模型能投下影子的区域（昆虫离地高，范围要够大）
const S_CAM = 1.6;
Object.assign(key.shadow.camera, {
  left: -S_CAM, right: S_CAM, top: S_CAM, bottom: -S_CAM,
  near: 0.1, far: 12,
});
key.shadow.bias = -0.001; // 防自阴影 acne
scene.add(key);
scene.add(key.target); // key.target 跟随模型水平位置，让影子相机罩住模型

const rim = new THREE.DirectionalLight(0x88bbff, 0.5);
rim.position.set(2, 1, -3);
scene.add(rim);

// 地面接收面：ShadowMaterial 只显示影子，不遮挡透明背景
const groundMat = new THREE.ShadowMaterial({ opacity: 0.35 });
const ground = new THREE.Mesh(new THREE.PlaneGeometry(8, 8), groundMat);
ground.rotation.x = -Math.PI / 2;
ground.position.y = 0;
ground.receiveShadow = true;
scene.add(ground);

/* ======================= 2. 姿态 → 动画映射 =======================
   各物种的烘焙动画命名约定不同，规则挂在物种注册表（SPECIES_MODELS）里：
   蝴蝶（Sketchfab metarig|N）：|3 待机 / |! 飞行 / |2 滑行
   蜻蜓：Fly-IP 飞行 / Stand_01-IP 待机（无滑行，滑行姿态回落 Fly）
   匹配按名称包含特征字符，找不到则按顺序兜底。
   ================================================================ */
// 通用兜底动画键：物种注册表没写 clipKeys 时的匹配约定（蝴蝶 Sketchfab 命名）
const CLIP_RULES = {
  perch: ['|3', 'stand', 'idle'],
  fly:   ['|!', 'fly', '4'],
  glide: ['|2', '5', 'fly']
};

// 物种 → 模型 / 动画键 / 运动参数注册表（加新模型物种在这里加一条）
const SPECIES_MODELS = {
  butterfly: {
    url: 'models/butterfly.glb',
    clipKeys: { perch: ['|3'], fly: ['|!', '4'], glide: ['|2', '5'] },
    // 蝴蝶：飘忽（慢转向、大横滚、轻俯仰）
    // turnRate：朝向往目标角速度（rad/s，慢速优雅）；bank：最大侧倾（rad，飞行动作）
    turnRate: 2.2, bank: 0.9, pitchLean: 0.55, bobAmp: 0.012,
    bobFreq: 2.6, turb: 0.5
  },
  dragonfly: {
    url: 'models/dragonfly.glb',
    clipKeys: { perch: ['stand'], fly: ['fly'], glide: ['fly'] },
    // 蜻蜓：笔直疾飞（快转向、小横滚、几乎无俯仰波动）
    // dim：模型目标尺寸（蜻蜓默认观感偏小，调大 30%）
    dim: 0.845,
    turnRate: 4.5, bank: 0.28, pitchLean: 0.18, bobAmp: 0.004,
    bobFreq: 1.2, turb: 0.2
  },
  fly: {
    url: 'models/fly.glb',
    // 苍蝇动画（Mixamo 命名）：Armature|Idle1-4 待机 / Armature|Fly 飞行 /
    // Armature|Hovering 悬停。perchRandom：待机在 4 个 Idle 里随机选。
    clipKeys: { perch: ['idle'], fly: ['fly'], glide: ['hover', 'fly'] },
    perchRandom: true,
    // 苍蝇：身形小、机动急（高转向速率、小横滚、颠簸明显）
    dim: 0.338,
    turnRate: 6.0, bank: 0.35, pitchLean: 0.2, bobAmp: 0.006,
    bobFreq: 1.6, turb: 0.6
  }
};
const MODEL_DEFAULT = SPECIES_MODELS.butterfly;

let clips = [];
let mixer = null;
let activeAction = null;
let currentSpecies = null;   // 当前已加载模型的物种 id
let activeSpecies = null;    // 当前逻辑物种（Rust 下发，决定加载哪个模型）
let loadToken = 0;           // 并发加载令牌：旧的慢加载结果到达时丢弃

function findClip(keys) {
  if (!clips.length) return null;
  for (const k of keys) {
    const hit = clips.find(c => c.name && c.name.toLowerCase().includes(k));
    if (hit) return hit;
  }
  return null;
}

// 返回匹配某姿态的全部动画（供随机选择）
function findAllClips(keys) {
  if (!clips.length) return [];
  return clips.filter(c =>
    keys.some(k => c.name && c.name.toLowerCase().includes(k))
  );
}

function pickClip(kind) {
  if (!clips.length) return null;
  const spec = SPECIES_MODELS[currentSpecies] || MODEL_DEFAULT;
  const keys = (spec.clipKeys && spec.clipKeys[kind]) || CLIP_RULES[kind];
  // 标记 perchRandom 的物种（苍蝇）：待机从多个 Idle 里随机选一个
  if (kind === 'perch' && spec.perchRandom) {
    const cands = findAllClips(keys);
    if (cands.length) return cands[Math.floor(Math.random() * cands.length)];
  }
  const hit = findClip(keys);
  return hit || clips[0];
}

// 栖息期间随机换一个不同的待机动作（苍蝇 Idle1-4 轮播）
function switchPerchAnim() {
  const spec = SPECIES_MODELS[currentSpecies] || MODEL_DEFAULT;
  if (!spec.perchRandom || !mixer) return;
  const keys = (spec.clipKeys && spec.clipKeys.perch) || CLIP_RULES.perch;
  const cands = findAllClips(keys);
  if (cands.length < 2) return;
  const cur = activeAction && activeAction._clip;
  const others = cands.filter(c => c !== cur);
  const clip = others[Math.floor(Math.random() * others.length)];
  const action = mixer.clipAction(clip);
  if (activeAction) activeAction.fadeOut(0.3);
  action.reset();
  action.setLoop(THREE.LoopRepeat);
  action.fadeIn(0.3);
  action.play();
  activeAction = action;
}

function setPose(pose) {
  if (!mixer) return;
  const clip = pickClip(pose);
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

/* ======================= 3. 模型加载 =======================
   用 fetch → parse（同步）代替 loader.load（XHR 流式），
   规避 macOS WKWebView 对 GLTFLoader 内部 XHR 加载的兼容问题。
   按当前物种加载对应模型；切物种时把旧模型/旧 mixer 整体丢弃。 */
const loader = new GLTFLoader();
function setupModel(gltf, speciesId) {
  const root = gltf.scene;

  // 兼容修复：部分 glb（如蜻蜓）的骨架容器带 0.01 单位缩放，会把蒙皮顶点压到
  // 近乎一点导致模型不可见。检测并拉回 1（骨骼 transform 由动画驱动）。
  let skelFix = false;
  root.traverse(o => {
    if (o.scale && Math.abs(o.scale.x - 0.01) < 0.002 && o.isBone !== true) {
      o.scale.set(1, 1, 1);
      skelFix = true;
    }
  });
  if (skelFix) console.log('[flypet] 已修复骨架单位缩放（0.01 → 1）');
  // 金属度修正 + 开启投影：无环境贴图时 metalness 高的材质会渲染成纯黑，压到可感知范围
  root.traverse(o => {
    if (!o.isMesh) return;
    o.castShadow = true;
    const mats = Array.isArray(o.material) ? o.material : [o.material];
    for (const m of mats) {
      if (m && (m.isMeshStandardMaterial || m.isMeshPhysicalMaterial) && m.metalness > 0.4) {
        m.metalness = 0.15;
        m.roughness = Math.max(m.roughness, 0.7);
        m.envMapIntensity = 1.0;
      }
    }
  });

  petRoot = root;
  scene.add(petRoot);
  clips = gltf.animations || [];
  mixer = new THREE.AnimationMixer(petRoot);
  activeAction = null;
  currentSpecies = speciesId;

  // 先刷新一次矩阵，让蒙皮骨骼进入 bind 姿势，precise 包围盒才能算到蒙皮后顶点
  root.updateMatrixWorld(true);
  root.traverse(o => { if (o.isSkinnedMesh && o.skeleton) o.skeleton.update(); });

  // 归一化 + 居中：precise=true 让 SkinnedMesh 顶点按骨骼姿势计算，否则用的是
  // 原始几何包围盒（未蒙皮），骨骼缩放修复后两者会不一致导致比例错误。
  const box = new THREE.Box3().setFromObject(root, true);
  const size = new THREE.Vector3();
  box.getSize(size);
  const maxDim = Math.max(size.x, size.y, size.z) || 1;
  // 目标尺寸：URL ?scale= 覆盖（预览）；否则用物种注册表 dim（默认 0.65）
  const specDef = SPECIES_MODELS[speciesId];
  const urlScale = _q.get('scale');
  const dim = (specDef && specDef.dim) || 0.65;
  const targetDim = urlScale !== null ? parseFloat(urlScale) : dim;
  root.scale.setScalar(targetDim / maxDim);
  root.updateMatrixWorld(true);
  const box2 = new THREE.Box3().setFromObject(root, true);
  const c = new THREE.Vector3();
  box2.getCenter(c);
  root.position.sub(c);
  root.updateMatrixWorld(true);

  const names = clips.map(c => c.name || '(未命名)').join(', ');
  console.log(`[flypet] ${speciesId} 模型加载完成 动画:`, names || '无');
  if (!clips.length) console.warn('[flypet] 模型无烘焙动画，仅有静态展示');
  // 加载完成切到当前姿态：切换发生在飞行中则直接播飞行动画
  setPose(pose === 'perch' ? 'perch' : (pose === 'glide' ? 'glide' : 'fly'));
}

function loadSpeciesModel(speciesId) {
  // 只有 GLB 的物种走 3D 模型；无 GLB 的物种回退蝴蝶模型
  const hasModel = !!SPECIES_MODELS[speciesId];
  const loadId = hasModel ? speciesId : 'butterfly';
  const spec = SPECIES_MODELS[loadId];
  activeSpecies = speciesId;
  const token = ++loadToken;
  fetch(spec.url)
    .then(r => r.arrayBuffer())
    .then(buf => {
      if (token !== loadToken) return; // 已被更新的切换取代
      loader.parse(buf, '', (gltf) => {
        if (token !== loadToken) return;
        // 丢弃旧模型与旧动画状态
        if (petRoot) { scene.remove(petRoot); petRoot.traverse(o => { if (o.geometry) o.geometry.dispose(); }); }
        if (mixer) mixer.stopAllAction();
        setupModel(gltf, loadId);
      }, (e) => {
        console.error('[flypet] parse 失败:', e);
      });
    })
    .catch(e => {
      console.error('[flypet] 模型加载失败:', e);
    });
}

/* ======================= 4. 姿态状态 ======================= */
let pose = 'perch';
let poseT = 0;
let heading = 0;          // 目标朝向（世界坐标弧度，来自 Rust）
let curHeading = 0;       // 当前朝向（平滑趋近 heading）
let roll = 0;             // 侧倾角（转弯时）
let targetAlt = 0;               // 目标高度（场景单位，来自 Rust alt/60）
let curAlt = 0;                  // 渲染用平滑高度
let pitch = 0;                   // 俯仰（爬升抬头 / 俯冲低头）
let curShadowOpacity = 0.35;     // 阴影透明度（平滑缓动，避免闪烁）

// 动态相机：栖息时俯视（看它停在哪），飞行/滑翔时侧上视角（看高度与姿态）。
// 贴边停靠时切到侧面视角（从垂直边缘的方向看蝴蝶顺边趴着）。
// 所有机位到目标的距离保持相等（CAM_DIST 恒定），视角切换只有角度变化，没有大小跳变。
const CAM_DIST = parseFloat(_q.get('dist') || '3.3'); // 相机到目标距离（恒定），预览可拉远
const CAM_ELEV = _q.get('elev') !== null ? parseFloat(_q.get('elev')) : null; // 预览覆盖俯仰角
const CAM = {
  perch: { elev: CAM_ELEV ?? 1.02, yaw: 0.0 },   // 栖息：近顶俯视（elev≈58°）
  air:   { elev: CAM_ELEV ?? 0.66, yaw: 0.0 },   // 飞行：侧上 38°（看高度与姿态，但不过分侧视）
  edge:  { elev: CAM_ELEV ?? 0.18, yaw: 0.0 }    // 贴边：低角度侧视（看蝴蝶顺边趴）
};
// 当前相机参数（由 elev/yaw 实时计算位置，保证距离恒定）
let camElev = CAM.perch.elev, camYaw = 0;
// 相机视线焦点平滑（跟随模型位置，阻尼防抖）
let smoothFocusX = 0, smoothFocusY = 0, smoothFocusZ = 0;
// 贴边状态：收到 landingEdge 后，栖息时用侧视机位
let edgeCam = null;              // null / 'top' / 'bottom' / 'left' / 'right'
// 窗口 clamp 后的蝴蝶偏移（物理像素，Rust 下发）：模型在窗口内平移，保证窗口不越界
let offX = 0, offY = 0, curOffX = 0, curOffY = 0;

function applyState(s) {
  if (!s || !s.pose) return;
  // 物种切换：换模型（幂等，同一物种只加载一次）
  if (s.species && s.species !== activeSpecies) {
    loadSpeciesModel(s.species);
  }
  if (s.pose !== pose) {
    pose = s.pose;
    poseT = 0;
    targetAlt = (pose === 'fly' || pose === 'spawn') ? 0.55 :
                (pose === 'glide') ? 0.30 :
                (pose === 'feed' || pose === 'approach') ? 0.33 : 0;   // 进食/亲近低空
    // feed/approach 无专属动画，映射到飞行动画（煽翅）
    setPose(pose === 'flee' || pose === 'spawn' || pose === 'feed' || pose === 'approach' ? 'fly' : pose);
  }
  // 贴边方位：Rust 落点贴边时下发（top/bottom/left/right），起飞后清除
  if (s.landingEdge !== undefined) {
    edgeCam = s.landingEdge;
  }
  // 窗口内偏移（物理像素 → 场景单位 60px=1）：模型平移到真实位置
  if (s.dx !== undefined && s.dy !== undefined) {
    offX = s.dx / 60;
    offY = s.dy / 60;
  }
  // 朝向：飞行/滑翔用 Rust 的运动方向，栖息用 facing 翻面
  if (s.heading !== undefined) {
    heading = s.heading;
  } else if (s.facing < 0) {
    heading = Math.PI;
  } else {
    heading = 0;
  }
  // 高度：Rust 下发的物理像素高度（缩放为场景单位）
  if (s.alt !== undefined) {
    targetAlt = s.alt / 60; // 60px ≈ 1 场景单位，蝴蝶巡航 46px ≈ 0.77
  }
}

/* ======================= 5. 演示模式（无 Tauri 直开浏览器） ======================= */
let demoHeading = 0, demoTurnT = 0;
function demoLoop(dt) {
  if (window.__petDemoOff) return; // 调试钩子可冻结演示随机切换
  poseT += dt;
  if (pose === 'perch' && poseT > 2.5 + Math.random() * 3) {
    const r = Math.random();
    applyState({ pose: r < 0.4 ? 'flee' : (r < 0.7 ? 'glide' : 'perch'), facing: 1 });
  } else if ((pose === 'flee' || pose === 'glide') && poseT > 2.0) {
    // 演示：约 45% 概率贴边停靠（验证侧视）
    const edges = [null, null, null, null, 'top', 'bottom', 'left', 'right', 'top', 'bottom', 'left', 'right'];
    const e = edges[Math.floor(Math.random() * edges.length)];
    const h = e === 'left' ? Math.PI / 2 : e === 'right' ? -Math.PI / 2 : (Math.random() < 0.5 ? 0 : Math.PI);
    applyState({ pose: 'perch', facing: 1, heading: h, alt: 0, landingEdge: e });
  }
  // 演示：飞行/滑行时模拟蝴蝶转向（盘旋）与高度起伏
  if (airPose()) {
    demoTurnT += dt;
    if (demoTurnT > 0.9) { demoTurnT = 0; demoHeading += (Math.random() - 0.5) * 1.8; }
    const demoAlt = (pose === 'glide' ? 14 : 40) * (0.7 + 0.3 * Math.sin(demoTurnT * 3));
    applyState({ pose, heading: demoHeading, facing: 1, alt: demoAlt, landingEdge: null });
  }
}
const airPose = () => pose === 'flee' || pose === 'glide' || pose === 'spawn' || pose === 'feed' || pose === 'approach';

/* ======================= 6. 主循环 ======================= */
const clock = new THREE.Clock();
let isTauri = false;
let petRoot = null;  // 模型根引用（不靠 scene.children[0]，因为还有灯光）
// 栖息待机动画轮换（苍蝇 Idle1-4）：当前动作已播时长 + 下次换动作时间
let perchAnimT = 0;
let perchAnimNext = 4 + Math.random() * 4;

function loop() {
  const dt = Math.min(0.05, clock.getDelta());
  const t = clock.elapsedTime;

  if (!isTauri) demoLoop(dt);

  // 运动参数（按物种）
  const sp = SPECIES_MODELS[currentSpecies] || MODEL_DEFAULT;

  // 整体运动（永远代码驱动：位置/朝向/浮动）
  poseT += dt;
  // 高度平滑（跟随 Rust alt，但带缓动避免跳变）
  curAlt += (targetAlt - curAlt) * Math.min(1, dt * 5);
  const air = pose === 'flee' || pose === 'glide' || pose === 'spawn' || pose === 'feed' || pose === 'approach';
  // 浮动：飞行=低频小幅起伏（身体稳定，翅膀动画已表现拍翅），栖息=缓慢呼吸感（幅度按物种）
  const bob = air
    ? Math.sin(t * 4.5) * sp.bobAmp * 0.6
    : Math.sin(t * sp.bobFreq) * sp.bobAmp;
  const root = petRoot;
  if (root) {
    // 位置：高度在场景里用 y 表现；窗口偏移（Rust dx/dy）平移到真实位置。
    // 屏幕坐标 → 场景：x→x，y→z（俯视 z 朝下）。平滑过渡避免跳变。
    curOffX += (offX - curOffX) * Math.min(1, dt * 6);
    curOffY += (offY - curOffY) * Math.min(1, dt * 6);
    root.position.x = curOffX;
    root.position.y = curAlt + bob;
    root.position.z = curOffY;

    // ---- 朝向：平滑转向运动方向 ----
    // 世界坐标：角度 0=朝右(+x)，π/2=朝下(+y)。
    // three 顶视 rotation.y 绕 Y 轴，0=面向 -Z；把世界方向映射过去：
    //   three 角 = -(世界角) - π/2（右手系换算，验证后微调）
    // 模型默认面朝 +Z（three 约定），加 π 补偿把头部对准运动方向。
    // 栖息也用 Rust 下发的 rest_heading（落地保留的最后飞行朝向），不再硬编码朝右/朝左。
    const target = -(heading) - Math.PI / 2 + Math.PI;
    // 角差值归一化到 [-π, π]（d 之后别再用，含跳变）
    const d = ((target - curHeading + Math.PI) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2) - Math.PI;
    // 每帧角速度上限：d 里混着之字换段的「航向阶跃」与盘旋切线的连续旋转。
    // 直接按 d 指数趋近，换段时曲线会被拉成直角的方角；改按速度上限追赶：
    // 连续转（盘旋 / 之字缓变）被速率限制平滑跟随，航向阶跃才以 maxTurn 内插。
    // turnRate = 正常转向角速度，maxTurn 是短暂尖峰（阶跃回正）的允许峰值。
    const maxTurn = air ? sp.turnRate * 3.2 : sp.turnRate;
    const step = Math.min(Math.abs(d), maxTurn * dt);
    const newH = curHeading + Math.sign(d) * step;
    // 侧倾 = 平滑后的瞬时转向速率比例 → 连续小转倾角小、急转倾角大，永远有限
    const effRate = (newH - curHeading) / Math.max(dt, 1e-4);
    const maxBank = air ? sp.bank * 0.9 : 0;
    const targetRoll = -Math.max(-maxBank, Math.min(maxBank, effRate * 0.42));
    curHeading = newH;
    root.rotation.y = curHeading;

    // ---- 俯仰：飞行前倾 / 爬升抬头 / 俯冲低头 / 滑翔微倾 / 栖息回正 ----
    if (pose === 'flee' || pose === 'spawn' || pose === 'approach') {
      // 用目标高度的变化率估计爬升/俯冲（目标来自 Rust alt，每 33ms 才跳一次；
      // 直接对 curAlt 微分会把平滑滞后误判成俯冲）。
      const altRate = (targetAlt - curAlt) * 5; // 近似 dAlt/dt
      pitch += (0.10 + altRate * 0.25 - pitch) * Math.min(1, dt * 6);
      root.rotation.x = -pitch * (sp.pitchLean / 0.55);
    } else if (pose === 'glide') {
      pitch += (0.06 - pitch) * Math.min(1, dt * 6);
      root.rotation.x = -pitch;
    } else {
      pitch *= (1 - Math.min(1, dt * 6));
      root.rotation.x = -pitch;
    }

    // 侧倾主体：平滑趋近目标倾角（上一帧由转向速率算好）
    roll += (targetRoll - roll) * Math.min(1, dt * 4);

    // ---- 飞行姿态波动（小幅，不改变朝向，避免倒飞感）----
    if (air) {
      // 身体轻微起伏（气流颠簸）：俯仰 ±0.06（蜻蜓更平稳）
      root.rotation.x += Math.sin(t * 7.3) * 0.04 * sp.turb;
      // 侧向轻微摇晃（滚转 ±0.05），叠加在转向侧倾上
      root.rotation.z = roll + Math.sin(t * 5.1 + 1.3) * 0.05 * sp.turb;
      // 尾部轻微左右摆（偏航 ±0.05）
      root.rotation.y += Math.sin(t * 4.2 + 0.6) * 0.05 * sp.turb;
    } else {
      root.rotation.z = roll;
    }

    // ---- 真实阴影：灯光 target 跟随模型水平位置，影子落在 y=0 地面 ----
    // 模型越高影子越淡（空气透视），透明度按高度平滑衰减（缓动，避免闪烁）
    const h = curAlt;
    key.target.position.set(root.position.x, 0, root.position.z);
    key.position.set(root.position.x - 2, 4 + h * 0.5, root.position.z + 2); // 光方向保持俯照
    key.target.updateMatrixWorld();
    const targetOpacity = Math.max(0.06, 0.35 * Math.max(0, 1 - h * 0.7));
    curShadowOpacity += (targetOpacity - curShadowOpacity) * Math.min(1, dt * 5);
    groundMat.opacity = curShadowOpacity;
  }

  // ---- 动态相机：三态机位（飞行侧上 / 栖息俯视 / 贴边侧视）----
  // 相机围绕蝴蝶当前位置旋转：半径恒定 → 蝴蝶视觉大小恒定。
  // 飞行→侧上 25°；栖息→俯视；贴边栖息→低角度侧视（从垂直边缘方向看）。
  // 视线跟随模型实际位置（root.position.x/z），不钉死窗口中心——窗口被屏幕
  // clamp 后蝴蝶可偏到窗口边缘，视线不跟着会把蝴蝶甩出画面（「消失」）。
  let tgtElev, tgtYaw;
  if (air) {
    tgtElev = CAM.air.elev;
    tgtYaw = 0;
  } else if (edgeCam) {
    // 贴边：低角度侧视。yaw 转向让相机位于边缘的外侧方向。
    tgtElev = CAM.edge.elev;
    tgtYaw = (edgeCam === 'left') ? Math.PI / 2 :
             (edgeCam === 'right') ? -Math.PI / 2 : 0;
    // 水平边（top/bottom）：从正侧面看（z 轴负方向已是侧视），yaw 保持 0
  } else {
    tgtElev = CAM.perch.elev;
    tgtYaw = 0;
  }
  // 平滑过渡到目标机位（角度插值，距离恒为 CAM_DIST）
  camElev += (tgtElev - camElev) * Math.min(1, dt * 2.2);
  camYaw  += (tgtYaw - camYaw) * Math.min(1, dt * 2.2);
  const ce = camElev;
  // 视线焦点 = 模型水平位置（部分跟随，水平由 Rust 窗口 clamp 负责）+
  // 高度 100% 跟随 curAlt：蝴蝶升降时相机同步平移，模型始终保持在画面内。
  // curAlt 本身是低通平滑值（dt*5），不会把高频起伏传给相机；bob 不进焦点
  // （模型绕焦点小幅浮动是真实运动，幅度小不会出画）。
  const focus = new THREE.Vector3(
    (root ? root.position.x : 0) * 0.25,
    (root ? curAlt : 0),
    (root ? root.position.z : 0) * 0.25
  );
  // 相机空间速度平滑（阻尼）——防止 off 平滑到位前模型仍在窗口内横移时
  // 相机生硬地推拉。上一帧到本帧的期望焦点位移被部分抑制，避免剧烈跳变。
  const kp = Math.min(1, dt * 8);
  smoothFocusX += (focus.x - smoothFocusX) * kp;
  smoothFocusY += (focus.y - smoothFocusY) * kp;
  smoothFocusZ += (focus.z - smoothFocusZ) * kp;
  const sf = new THREE.Vector3(smoothFocusX, smoothFocusY, smoothFocusZ);
  // 球面坐标（绕 smooth focus 旋转，半径恒定）
  cam.position.set(
    sf.x + CAM_DIST * Math.cos(ce) * Math.sin(camYaw),
    sf.y + CAM_DIST * Math.sin(ce),
    sf.z + CAM_DIST * Math.cos(ce) * Math.cos(camYaw)
  );
  cam.lookAt(sf);

  // 栖息待机动作轮换（苍蝇）：停留期间每隔几秒随机换一个 Idle，不一直做同一个
  const spCur = SPECIES_MODELS[currentSpecies];
  if (pose === 'perch' && spCur && spCur.perchRandom && mixer) {
    perchAnimT += dt;
    if (perchAnimT >= perchAnimNext) {
      switchPerchAnim();
      perchAnimT = 0;
      perchAnimNext = 3.5 + Math.random() * 5; // 3.5~8.5s 换一次动作
    }
  } else {
    perchAnimT = 0;
    perchAnimNext = 4 + Math.random() * 4;
  }

  if (mixer) mixer.update(dt);
  renderer.render(scene, cam);
  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);

/* ======================= 7. Tauri 集成 + 演示背景 ======================= */
(async () => {
  const t = window.__TAURI__;
  if (t) {
    isTauri = true;
    try { applyState(await t.core.invoke('state')); } catch (e) {}
    try { t.event.listen('state', (e) => applyState(e.payload)); } catch (e) {}
  }
})();
// 无 Tauri 演示模式（浏览器直开）：无 state 事件，按 URL ?species= 加载（默认蝴蝶）并让演示状态机跑起来
if (!isTauri) {
  const sp = new URLSearchParams(location.search).get('species');
  loadSpeciesModel(SPECIES_MODELS[sp] ? sp : 'butterfly');
}

/* 调试钩子（仅 URL 带 __hook=1 时挂 window.__pet）：
   供无头 / 控制台确定性验证「相机取景框与蝴蝶位置关系」用，不影响正式运行。 */
if (new URLSearchParams(location.search).has('__hook')) {
  window.__pet = {
    applyState,
    inspect() {
      const box = petRoot && new THREE.Box3().setFromObject(petRoot);
      if (!box) return null;
      const s = box.getBoundingSphere(new THREE.Sphere());
      const project = v => {
        const p = v.clone().project(cam);
        return { x: (p.x * 0.5 + 0.5) * W, y: (-p.y * 0.5 + 0.5) * H };
      };
      const c = project(s.center);
      const top = project(new THREE.Vector3(s.center.x, s.center.y + s.radius, s.center.z));
      const rpx = Math.abs(top.y - c.y);
      return {
        cx: Math.round(c.x), cy: Math.round(c.y), rpx: Math.round(rpx),
        inX: c.x - rpx >= 0 && c.x + rpx <= W,
        inY: c.y - rpx >= 0 && c.y + rpx <= H,
        W, H, pose, alt: curAlt, off: [curOffX.toFixed(3), curOffY.toFixed(3)],
        rotY: petRoot ? +petRoot.rotation.y.toFixed(3) : null,
        rotZ: petRoot ? +petRoot.rotation.z.toFixed(3) : null,
        targetHeading: +heading.toFixed(3),
        anim: activeAction && activeAction._clip ? activeAction._clip.name : null,
      };
    },
  };
  console.log('[flypet] 调试钩子已挂载 (__hook)');
}
