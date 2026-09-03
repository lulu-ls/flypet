// FlyPet 喂食动画窗
// 流程：金粉从顶部飘落 → 宠物从下方飞入 → 绕 3~5 圈 → 金粉消散 → 宠物飞走 → 关窗
// 用 Canvas 2D（透明窗口可用），宠物复用 render.js 的程序化昆虫渲染
import { makeGenome, applyTraitEffects, drawInsect } from './render.js';

const W = 320, H = 240;
const cv = document.getElementById('view');
const ctx = cv.getContext('2d');
cv.width = W * 2;
cv.height = H * 2;
ctx.scale(2, 2);

const isTauri = !!window.__TAURI__;

// ---------- 品级配色（与 food.rs Rarity 对应） ----------
// 每品级只存色相 + 是否彩光。光点（尘点/亮核/光晕/星芒）统一用 h 着色，
// 保证光点颜色与食物品质颜色严格对应。仙/神品为流动彩光（rainbow）。
const RARITY = {
  fan:   { h: 210, rainbow: false },
  ling:  { h: 140, rainbow: false },
  xuan:  { h: 208, rainbow: false },
  di:    { h: 268, rainbow: false },
  tian:  { h: 43,  rainbow: false },
  xian:  { h: 218, rainbow: true },
  shen:  { h: 45,  rainbow: true }
};

// ---------- 动画参数获取 + 事件驱动播放 ----------
// 窗口加载一次；Rust 每次投喂后发 "feed-anim-start"，收到事件重新取参播放。
let loopToken = 0;   // 递增令牌：旧 loop 发现令牌变了就退出（支持快速连喂）

async function fetchAnim() {
  if (!isTauri) {
    // 浏览器预览：支持 ?rarity=xxx&name=xxx 覆盖（方便看不同品级配色）
    const q = new URLSearchParams(location.search);
    const r = q.get('rarity');
    return {
      name: q.get('name') || '聚气丹',
      rarity: r && RARITY[r] ? r : 'ling',
      seed: 42,
      species: q.get('species') || 'butterfly'
    };
  }
  try {
    return await window.__TAURI__.core.invoke('anim_params');
  } catch (e) {
    console.error('[feed-anim] 取参数失败', e);
    return { name: '露水珠', rarity: 'fan', seed: 7, species: 'butterfly' };
  }
}

async function play() {
  const anim = await fetchAnim();
  if (!anim) return;
  // 若上一轮还在播，立刻中止（令牌失效）
  loopToken++;
  start(anim);
}

if (isTauri) {
  window.__TAURI__.event.listen('feed-anim-start', () => play());
  // 兜底：窗口首次加载完成时若 Rust 已有待播参数（事件可能在 listen 注册前到达），
  // 主动取一次。无参数时返回 null 不播，幂等安全。
  play();
} else {
  // 浏览器预览：直接播一次
  play();
}

// ---------- 玻璃反光粉尘粒子系统 ----------
// 效果：细碎粉尘如玻璃碎屑，大部分时间昏暗微亮，随机某颗被光扫到——
// 骤亮成锐利的点光（尖峰脉冲）再快速暗下。稀疏、随机、瞬间，像阳光下玻璃的反光。
// 绘制用「暗淡底色 + 闪光时叠光晕+星芒」。
function makeParticles(rarityKey, maxCount) {
  const base = RARITY[rarityKey] || RARITY.fan;
  const parts = [];
  const bandW = W * 0.85;
  const spawnParticle = () => {
    const layer = Math.random();           // 0 远景(暗小) ~ 1 近景(亮大)
    const big = Math.random() < 0.03 + layer * 0.03;   // 极少量大颗粒（闪起来更显眼）
    return {
      x: W * 0.5 + (Math.random() - 0.5) * bandW,
      y: -8 - Math.random() * 40,
      size: big ? (0.9 + Math.random() * 0.5) : (0.3 + Math.random() * (0.25 + layer * 0.25)),
      vy: (10 + Math.random() * 22) * (0.5 + layer * 0.7),
      drift: (Math.random() - 0.5) * 14,
      swayAmp: 5 + Math.random() * 10,
      swayFreq: 0.6 + Math.random() * 1.2,
      phase: Math.random() * Math.PI * 2,
      big,
      layer,
      rot: Math.random() * Math.PI,       // 星芒朝向
      hueBase: Math.floor(Math.random() * 360),
      // ---- 反光状态 ----
      baseA: (big ? 0.12 : 0.07) + layer * 0.07,  // 暗态基底亮度（隐约可见的粉尘层）
      flashT: 0,                            // 当前闪光强度 0..1（指数衰减）
      flashSpeed: 3.0 + Math.random() * 5.0, // 熄灭速率（越大灭得越快）
      freq: Math.random() < 0.08,           // 仅极少数“角度好”的粒子频繁闪，多数几乎不闪
      nextFlash: Math.random() * 8.0,       // 距下次闪光的随机计时（开局分散，避免集中闪）
      flashPeak: 0.7 + Math.random() * 0.3  // 峰值强度
    };
  };
  // 初始铺满一屏中上部分
  for (let i = 0; i < maxCount * 0.8; i++) {
    const p = spawnParticle();
    p.y = -10 + Math.random() * H * 0.6;
    parts.push(p);
  }
  return { parts, base, spawnParticle, maxCount };
}

// 向顶部源补粒子（每帧补充，维持“持续飘落”感）
function refillParticles(state, dt) {
  const rate = state.maxCount * 0.5;
  let n = Math.floor(rate * dt) + (Math.random() < rate * dt % 1 ? 1 : 0);
  while (n-- > 0 && state.parts.length < state.maxCount * 1.15) {
    state.parts.push(state.spawnParticle());
  }
}

// 推进闪光计时：随机触发锐利尖峰，指数衰减快速熄灭（玻璃反光质感）
function updateFlashes(p, dt) {
  if (p.flashT > 0) {
    // 指数衰减：0.25~0.5s 内从峰值快速熄灭
    p.flashT *= Math.max(0, 1 - dt * (4.5 + p.flashSpeed));
    if (p.flashT < 0.02) p.flashT = 0;
    return;
  }
  p.nextFlash -= dt;
  if (p.nextFlash <= 0) {
    p.flashT = 1;                              // 骤亮
    // 极少数常闪粒子（3~7s），其余很偶尔闪（8~20s）
    p.nextFlash = p.freq
      ? 3.0 + Math.random() * 4.0
      : 8.0 + Math.random() * 12.0;
  }
}

// ---------- 宠物状态 ----------
let genome = null;
let pet = { x: 0, y: 0, radiusX: 0, radiusY: 0 };

function initPet(seed, speciesId, stage) {
  const g = makeGenome(seed, speciesId, stage);
  applyTraitEffects(g);
  return g;
}

// ---------- 阶段时间线 ----------
const PHASE = {
  fall:   { dur: 1.6 },   // 金粉飘落
  enter:  { dur: 0.8 },   // 宠物飞入
  circle: { dur: 4.2 },   // 绕圈 3~5 圈
  vanish: { dur: 0.9 },   // 金粉消散
  flyout: { dur: 0.8 }    // 宠物飞走
};

let phase = 'fall';
let phaseT = 0;
let elapsed = 0;
let circles = 0;
let targetCircles = 3;
let petAngle = 0;
let particleState = null;
let lastTick = 0;      // 上一帧真实时间戳（计算实际 dt，抗定时器节流）

function start(anim) {
  // 重置播放状态
  phase = 'fall';
  phaseT = 0;
  elapsed = 0;
  circles = 0;
  petAngle = Math.PI * 0.2;
  lastTick = performance.now();
  const myToken = loopToken;

  // 绕圈数 3~5（品级越高绕越多，神品 5）
  const rarityKey = anim.rarity;
  targetCircles = Math.min(5, 3 + (rarityKey === 'shen' ? 2 : rarityKey === 'tian' || rarityKey === 'xian' ? 1 : 0));
  // 金粉密度随品级增多（凡 180 → 神 380）
  const n = 180 + (rarityKey === 'shen' ? 200 : rarityKey === 'tian' || rarityKey === 'xian' ? 140 : rarityKey === 'di' ? 80 : rarityKey === 'xuan' ? 40 : rarityKey === 'ling' ? 20 : 0);
  particleState = makeParticles(rarityKey, n);

  // 用 seed + 当前物种生成宠物基因组（外观与主窗一致）
  const speciesId = anim.species || 'butterfly';
  const stage = 3;
  genome = initPet(anim.seed || 7, speciesId, stage);

  // 宠物初始位置：底部偏右，绕圈中心在碎屑区下方
  pet = {
    x: W * 0.5,
    y: H * 0.72,
    radiusX: 70,
    radiusY: 34
  };

  const loop = () => {
    if (loopToken !== myToken || finished) return;   // 新一轮已开始 / 已完成，退出
    let done = false;
    try {
      done = step(anim, myToken) === true;
    } catch (e) {
      console.error('[feed-anim] step error', e);
      done = true;
    }
    if (done) finishOnce(myToken);
  };
  // 完成回调：只调一次 finish_anim，且停掉所有驱动循环
  let finished = false;
  const finishOnce = (tok) => {
    if (finished || tok !== loopToken) return;
    finished = true;
    if (isTauri) {
      try { window.__TAURI__.core.invoke('finish_anim'); } catch (e) {}
    }
  };
  // 主驱动 rAF（窗口可见时流畅 60fps）
  const rafLoop = () => {
    if (loopToken !== myToken || finished) return;
    loop();
    requestAnimationFrame(rafLoop);
  };
  requestAnimationFrame(rafLoop);
  // 备胎 setTimeout：若 rAF 被暂停（窗口刚 show / hidden），仍能推进
  const stLoop = () => {
    if (loopToken !== myToken || finished) return;
    loop();
    setTimeout(stLoop, 33);
  };
  setTimeout(stLoop, 33);
}

function step(anim, myToken) {
  // 真实 dt（毫秒→秒）：定时器可能被 WKWebView 节流，固定 1/60 会慢放
  const now = performance.now();
  const dt = Math.min(0.1, Math.max(0.001, (now - lastTick) / 1000));
  lastTick = now;
  elapsed += dt;

  const rarityKey = anim.rarity;
  const base = RARITY[rarityKey] || RARITY.fan;

  ctx.clearRect(0, 0, W, H);

  // ---- 金粉 ----
  // fall/enter/circle 阶段持续飘落 + 顶部补充；vanish 阶段上浮淡出
  const particleActive = phase === 'fall' || phase === 'enter' || phase === 'circle' || phase === 'vanish';
  if (particleActive && particleState) {
    if (phase !== 'vanish') refillParticles(particleState, dt);
    for (const p of particleState.parts) {
      if (phase !== 'vanish') {
        // 缓慢飘落 + 左右摆动
        p.y += p.vy * dt;
        p.x += (p.drift + Math.sin(elapsed * p.swayFreq + p.phase) * p.swayAmp * 0.4) * dt;
        p.rot += p.vr * dt;
        if (p.y > H + 10) {
          // 飘出底部：回收重投到顶部（保持密度）
          Object.assign(p, particleState.spawnParticle());
          p.y = -8 - Math.random() * 20;
          continue;
        }
      } else {
        // 消散：轻微上浮 + 淡出（不再闪光）
        p.y -= 26 * dt;
        p.baseA -= dt * 0.5;
        p.flashT = 0;
        if (p.baseA <= 0) continue;
      }

      // 闪光计时（飘落阶段随机触发；消散阶段禁用）
      if (phase !== 'vanish') updateFlashes(p, dt);
      const flash = p.flashT * p.flashPeak;   // 0..~1 当前闪光强度

      // 最终可见度 = 昏暗基底 + 闪光尖峰
      // 基底几乎不可见；闪光时骤亮
      const alpha = Math.min(1, p.baseA + flash);
      // 颜色：神/仙品彩光随时间流动；其余取本品级色相，保证与食物品质一致
      const hue = base.rainbow ? (elapsed * 120 + p.hueBase) % 360 : base.h;
      const sat = base.rainbow ? 100 : 72;
      const mainColor = (light) => `hsl(${hue},${sat}%,${light}%)`;

      // 暗态：细小的品级色尘点（若隐若现）
      if (alpha < 0.06) continue;
      ctx.globalAlpha = Math.max(0.02, alpha);
      ctx.fillStyle = mainColor(68);
      ctx.beginPath();
      ctx.arc(p.x, p.y, Math.max(0.3, p.size * 0.8), 0, Math.PI * 2);
      ctx.fill();

      // 闪光阶段：品级色亮核 + 品级光晕 + 白亮星芒（玻璃高光）
      if (flash > 0.25) {
        // 亮核：品级色偏亮（接近白但保留色调）
        ctx.globalAlpha = alpha;
        ctx.fillStyle = mainColor(base.rainbow ? 82 : 76);
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * 1.2 + flash * 0.6, 0, Math.PI * 2);
        ctx.fill();
        // 光晕：品级色
        ctx.globalAlpha = alpha * 0.3;
        ctx.fillStyle = mainColor(base.rainbow ? 70 : 62);
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size * 3.2 + flash * 2.0, 0, Math.PI * 2);
        ctx.fill();
        // 十字星芒：中央白亮、末端带品级色（高光反射感）
        ctx.globalAlpha = alpha * 0.9;
        ctx.strokeStyle = base.rainbow ? mainColor(85) : 'rgba(255,255,255,1)';
        ctx.lineWidth = Math.max(0.5, p.size * 0.35);
        ctx.lineCap = 'round';
        const arm = p.size * (3.5 + flash * 4.5);
        const sa = p.rot, ca = p.rot + Math.PI / 2;
        ctx.beginPath();
        ctx.moveTo(p.x - Math.cos(sa) * arm, p.y - Math.sin(sa) * arm);
        ctx.lineTo(p.x + Math.cos(sa) * arm, p.y + Math.sin(sa) * arm);
        ctx.moveTo(p.x - Math.cos(ca) * arm, p.y - Math.sin(ca) * arm);
        ctx.lineTo(p.x + Math.cos(ca) * arm, p.y + Math.sin(ca) * arm);
        ctx.stroke();
      }
    }
    ctx.globalAlpha = 1;   // 复位，避免影响后续绘制
  }

  // ---- 宠物绕圈 ----
  const air = phase === 'circle' || phase === 'enter' || phase === 'flyout';
  if (phase === 'enter') {
    // 从底部右侧飞入到绕圈起点
    const t = Math.min(1, phaseT / PHASE.enter.dur);
    const e = t * t * (3 - 2 * t); // smoothstep
    pet.x = W * 0.85 + (W * 0.5 - W * 0.85) * e;
    pet.y = H * 0.95 + (H * 0.72 - H * 0.95) * e;
    if (t >= 1) { phase = 'circle'; phaseT = 0; }
  } else if (phase === 'circle') {
    // 椭圆绕圈：顺时针（速度 5 rad/s ≈ 0.8 圈/s，3~5 圈约 3.8~6.3s）
    petAngle += dt * 5.0;
    const completed = petAngle / (Math.PI * 2);
    if (completed > circles) circles = Math.floor(completed);
    if (circles >= targetCircles && completed >= targetCircles) {
      phase = 'vanish'; phaseT = 0;
    }
    pet.x = W * 0.5 + Math.cos(petAngle) * pet.radiusX;
    pet.y = H * 0.72 + Math.sin(petAngle) * pet.radiusY;
  } else if (phase === 'vanish') {
    // 保持绕圈位置继续小幅盘旋，金粉消散
    pet.x = W * 0.5 + Math.cos(petAngle) * pet.radiusX;
    pet.y = H * 0.72 + Math.sin(petAngle) * pet.radiusY;
    petAngle += dt * 0.8;
  } else if (phase === 'flyout') {
    const t = Math.min(1, phaseT / PHASE.flyout.dur);
    const e = t * t;
    pet.x += (W * 1.2 - pet.x) * e * 0.05 + 140 * dt;
    pet.y -= 90 * dt;
  }

  // ---- 绘制宠物 ----
  if (genome && (air || phase === 'vanish')) {
    // 苍蝇体型比其他物种小 30%（与 pet/main 窗 dim 比例一致）
    const size = genome.species === 'fly' ? 56.4 : 70;
    const fly = true;
    // 朝向：绕圈时沿切线方向；飞行时朝运动方向
    const dir = phase === 'circle'
      ? Math.atan2(Math.cos(petAngle), -Math.sin(petAngle)) // 顺时针切线
      : (phase === 'enter' ? Math.atan2(pet.y - H * 0.95, pet.x - W * 0.85) : -Math.PI / 2);
    const facing = Math.cos(dir) >= 0 ? 1 : -1;
    // 高度：绕圈时轻微起伏
    const bob = Math.sin(elapsed * 6) * 3;
    ctx.save();
    ctx.translate(pet.x, pet.y + bob);
    drawInsect(ctx, genome, elapsed, size, fly, facing, {});
    ctx.restore();
  }

  // ---- 阶段推进 ----
  phaseT += dt;
  if (phase === 'fall' && phaseT >= PHASE.fall.dur) { phase = 'enter'; phaseT = 0; }
  if (phase === 'vanish' && phaseT >= PHASE.vanish.dur) { phase = 'flyout'; phaseT = 0; }
  if (phase === 'flyout' && phaseT >= PHASE.flyout.dur) {
    return true;   // 动画完成（由驱动循环调 finishOnce）
  }
  return false;
}
