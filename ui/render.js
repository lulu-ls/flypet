/* ======================= 1. 可复现随机 & 噪声 ======================= */
function mulberry32(a) {
  return function () {
    a |= 0; a = a + 0x6D2B79F5 | 0;
    let t = Math.imul(a ^ a >>> 15, 1 | a);
    t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
    return ((t ^ t >>> 14) >>> 0) / 4294967296;
  };
}
// 1D value noise：给翅膀边缘 / 体节轮廓加有机不规则
function makeNoise(rand, n = 24) {
  const arr = Array.from({ length: n }, () => rand() * 2 - 1);
  return (x) => {
    const i = Math.floor(x), f = x - i;
    const a = arr[((i % n) + n) % n], b = arr[(((i + 1) % n) + n) % n];
    const u = f * f * (3 - 2 * f);
    return a + (b - a) * u;
  };
}
const R = (rand, [a, b]) => a + rand() * (b - a);
const pick = (rand, arr) => arr[Math.floor(rand() * arr.length)];

/* ======================= 1.5 词条系统 ======================= */
const RARITY = {
  common:    { w: 60, label: 'COMMON' },
  rare:      { w: 27, label: 'RARE' },
  epic:      { w: 10, label: 'EPIC' },
  legendary: { w: 3,  label: 'LEGEND' }
};

// kind: look = 纯外观；stat = 影响玩法数值
const TRAITS = {
  /* ---- 外观类 ---- */
  iridescent: { name: '虹彩',   rarity: 'legendary', kind: 'look', desc: '翅膀泛七彩流光' },
  albino:     { name: '白化',   rarity: 'legendary', kind: 'look', desc: '通体雪白的变异' },
  crown:      { name: '王冠',   rarity: 'legendary', kind: 'look', desc: '头顶生有冠饰' },
  crystal:    { name: '晶翼',   rarity: 'epic',      kind: 'look', desc: '双翅如水晶通透' },
  glow:       { name: '荧光',   rarity: 'epic',      kind: 'look', desc: '身体透出微光' },
  stardust:   { name: '星尘',   rarity: 'epic',      kind: 'look', desc: '周身环绕光点' },
  ribbon:     { name: '绶带',   rarity: 'epic',      kind: 'look', desc: '腹部拖着长尾' },
  bigWing:    { name: '巨翼',   rarity: 'rare',      kind: 'look', desc: '翼展显著增大' },
  fuzzKing:   { name: '绒王',   rarity: 'rare',      kind: 'look', desc: '绒毛格外浓密' },
  melanic:    { name: '黑化',   rarity: 'rare',      kind: 'look', desc: '通体墨黑的变异' },
  metallic:   { name: '鎏金',   rarity: 'rare',      kind: 'look', desc: '金属质感的甲壳' },
  neon:       { name: '霓虹',   rarity: 'rare',      kind: 'look', desc: '鲜明的发光轮廓' },
  /* ---- 属性类 ---- */
  bigEater:     { name: '大胃王', rarity: 'rare',   kind: 'stat', desc: '进食速度 +60%',
                  mods: { eatSpeed: +0.60 } },
  affectionate: { name: '黏人',   rarity: 'rare',   kind: 'stat', desc: '亲密度获取 +30%',
                  mods: { affinity: +0.30 } },
  swift:        { name: '疾风',   rarity: 'rare',   kind: 'stat', desc: '飞行速度 +35%',
                  mods: { flySpeed: +0.35 } },
  lucky:        { name: '幸运',   rarity: 'epic',   kind: 'stat', desc: '进化时更容易出稀有词条',
                  mods: { luck: +1.0 } },
  sluggish:     { name: '慢吞吞', rarity: 'common', kind: 'stat', desc: '飞行速度 −25%（更从容）',
                  mods: { flySpeed: -0.25 } },
  timid:        { name: '胆小',   rarity: 'common', kind: 'stat', desc: '警戒半径 +45%',
                  mods: { fleeRadius: +0.45 } },
  fearless:     { name: '无畏',   rarity: 'common', kind: 'stat', desc: '警戒半径 −45%（更亲近你）',
                  mods: { fleeRadius: -0.45 } },
  sleepy:       { name: '贪睡',   rarity: 'common', kind: 'stat', desc: '停留时间 +60%',
                  mods: { idleTime: +0.60 } },
  lively:       { name: '活泼',   rarity: 'common', kind: 'stat', desc: '停留时间 −40%，更爱乱飞',
                  mods: { idleTime: -0.40 } }
};

// 按稀有度加权抽取 n 个不重复词条
function rollTraits(rand, n, luck = 0) {
  const pool = Object.keys(TRAITS);
  const out = [];
  for (let i = 0; i < n && out.length < pool.length; i++) {
    const avail = pool.filter(id => !out.includes(id));
    const weights = avail.map(id => {
      const w = RARITY[TRAITS[id].rarity].w;
      // 幸运：提高稀有及以上权重
      return TRAITS[id].rarity === 'common' ? w : w * (1 + luck);
    });
    const total = weights.reduce((a, b) => a + b, 0);
    let r = rand() * total, idx = 0;
    for (let j = 0; j < weights.length; j++) { r -= weights[j]; if (r <= 0) { idx = j; break; } }
    out.push(avail[idx]);
  }
  return out;
}

// 词条数量：随进化阶段增加，华彩保底一个罕见以上
function traitCountFor(stage, rand) {
  if (stage <= 1) return 0;
  if (stage === 2) return 1;
  if (stage === 3) return 1 + (rand() < .45 ? 1 : 0);
  return 2 + Math.floor(rand() * 3); // 2~4
}

/* ======================= 2. 物种模板：形态参数空间 ======================= */
const SPECIES = {
  butterfly: {
    hue: [0, 360], sat: [.62, .92], light: [.48, .64],
    bodyLen: [.95, 1.15], bodyW: [.55, .75], segs: [4, 6],
    wingLen: [1.35, 1.65], wingW: [1.05, 1.3], wingAlpha: [.72, .9],
    hindRatio: [.7, .88], fuzz: [.3, .55], gloss: [.35, .6],
    pattern: ['spot', 'band', 'gradient'], patternDensity: [.3, .7], patternHueShift: [140, 200],
    eyeSize: [.5, .7], antennaLen: [1.15, 1.5], legLen: [.7, .9], spots: [2, 4]
  },
  dragonfly: {
    hue: [150, 210], sat: [.4, .7], light: [.25, .45],
    bodyLen: [1.5, 1.8], bodyW: [.35, .5], segs: [7, 9],
    wingLen: [1.5, 1.8], wingW: [.45, .6], wingAlpha: [.1, .2], wingStyle: 'slim',
    hindRatio: [.85, 1.0], fuzz: [.1, .2], gloss: [.4, .6],
    pattern: ['band'], patternDensity: [.4, .65], patternHueShift: [0, 10],
    eyeSize: [1.1, 1.4], antennaLen: [.12, .2], legLen: [.6, .8], spots: 0
  },
  fly: {  // 蝇参考：暗石板灰 + 腹横带 + 直前缘三角形翅
    hue: [195, 210], sat: [.10, .18], light: [.20, .30],
    bodyLen: [.78, .88], bodyW: [.98, 1.12], segs: [4, 5],
    wingLen: [1.02, 1.16], wingW: [.62, .70], wingAlpha: [.34, .46], wingStyle: 'slim',
    hindRatio: [.10, .16], fuzz: [.55, .85], gloss: [.62, .80],
    pattern: ['bands'], patternDensity: [.40, .65], patternHueShift: [0, 6],
    eyeSize: [1.08, 1.22], antennaLen: [.30, .42], legLen: [1.05, 1.25], spots: 0
  },
  spider: {
    hue: [0, 20], sat: [.10, .25], light: [.10, .22],
    bodyLen: [.75, .9], bodyW: [1.0, 1.2], segs: [3, 4],
    wingLen: [.05, .08], wingW: [.1, .2], wingAlpha: [.3, .5], wingStyle: 'slim',
    hindRatio: [.9, 1.0], fuzz: [.5, .8], gloss: [.3, .5],
    pattern: ['gradient', 'stripe'], patternDensity: [.3, .55], patternHueShift: [0, 15],
    eyeSize: [.5, .7], antennaLen: [.15, .25], legLen: [1.6, 2.0], spots: 0
  }
};

/* ======================= 3. Genome：种子 → 形态基因组 ======================= */
function makeGenome(seed, speciesId, stage = 3) {
  const t = SPECIES[speciesId] || SPECIES.butterfly; // 未知物种回退蝴蝶
  const rand = mulberry32(seed);
  const noise = makeNoise(rand);

  const baseHue = R(rand, t.hue);
  const g = {
    seed, species: speciesId, stage,
    hue: baseHue,
    sat: R(rand, t.sat),
    light: R(rand, t.light),
    bodyLen: R(rand, t.bodyLen),
    bodyW: R(rand, t.bodyW),
    segs: Math.round(R(rand, t.segs)),
    wingLen: R(rand, t.wingLen),
    wingW: R(rand, t.wingW),
    wingAlpha: R(rand, t.wingAlpha),
    hindRatio: R(rand, t.hindRatio),
    fuzz: R(rand, t.fuzz),
    gloss: R(rand, t.gloss),
    pattern: pick(rand, t.pattern),
    patternDensity: R(rand, t.patternDensity),
    patternHue: (baseHue + R(rand, t.patternHueShift)) % 360,
    eyeSize: R(rand, t.eyeSize),
    antennaLen: R(rand, t.antennaLen),
    legLen: R(rand, t.legLen),
    spots: t.spots ? Math.round(R(rand, t.spots)) : 0,
    // 有机不规则：贝塞尔控制点扰动
    wc: [R(rand, [-.18, .18]), R(rand, [-.18, .18]), R(rand, [-.2, .2]), R(rand, [-.2, .2])],
    wingStyle: t.wingStyle || 'round',
    noise,
    // 个体差异（稳定随机，供绘制细节使用）
    jitter: Array.from({ length: 40 }, () => rand())
  };

  // 词条：由种子决定，因此可复现；数量随阶段增长
  const n = traitCountFor(stage, rand);
  g.traits = rollTraits(rand, n);
  return g;
}

// 词条效果：外观类烘焙进渲染参数，属性类累计成数值修正
function applyTraitEffects(g) {
  const ids = g.traits || [];
  const fx = {};
  const mods = { eatSpeed: 0, affinity: 0, flySpeed: 0, fleeRadius: 0, idleTime: 0, luck: 0 };

  for (const id of ids) {
    const t = TRAITS[id];
    if (!t) continue;
    if (t.mods) for (const k in t.mods) mods[k] += t.mods[k];
    switch (id) {
      case 'melanic':    g.light *= .40; g.sat *= .55; break;
      case 'albino':     g.light = Math.min(.88, .60 + g.light * .38); g.sat *= .26; break;
      case 'metallic':   g.gloss = Math.min(1, g.gloss * 1.55); g.sat *= .72; break;
      case 'fuzzKing':   g.fuzz = Math.min(1.7, g.fuzz * 1.95); break;
      case 'bigWing':    g.wingLen *= 1.30; g.wingW *= 1.12; break;
      case 'crystal':    g.wingAlpha = Math.min(.88, g.wingAlpha * 1.9); fx.crystal = 1; break;
      case 'iridescent': fx.iridescent = 1; break;
      case 'glow':       fx.glow = 1; break;
      case 'neon':       fx.neon = 1; break;
      case 'crown':      fx.crown = 1; break;
      case 'ribbon':     fx.ribbon = 1; break;
      case 'stardust':   fx.stardust = 1; break;
    }
  }
  g.fx = fx;
  g.mods = mods;
  return g;
}

// 进化：沿基因衍生，核心特征（色相/花纹）保留
function evolve(g, stage) {
  const e = Object.assign({}, g, { stage });
  const k = stage / 3;
  if (stage <= 1) return e;                       // 卵：只用色相画蛋
  if (stage === 2) {                              // 幼虫：无翅、体节明显
    e.wingLen = 0; e.legLen *= .45; e.antennaLen *= .35;
    e.bodyW *= 1.12; e.segs = Math.max(5, g.segs + 1);
    return e;
  }
  if (stage === 4) {                              // 华彩：放大 + 更华丽
    e.wingLen = g.wingLen * 1.12;
    e.patternDensity = Math.min(1, g.patternDensity * 1.35);
    e.gloss = Math.min(1, g.gloss * 1.3);
    e.light = Math.min(.72, g.light * 1.12);
    e.sat = Math.min(1, g.sat * 1.15);
  }
  return e;
}

/* ======================= 4. Canvas 渲染器 ======================= */
const hsl = (h, s, l, a = 1) => `hsla(${h},${(s * 100).toFixed(0)}%,${(l * 100).toFixed(0)}%,${a})`;

// 体积感渐变（光源左上）；fly 用金属蓝绿渐变模拟虹彩甲壳
function bodyGrad(ctx, x, y, rx, ry, h, s, l) {
  const g = ctx.createRadialGradient(x - rx * .32, y - ry * .38, rx * .06, x, y, Math.max(rx, ry) * 1.2);
  if (h >= 150 && h <= 220 && s > .25) {           // 金属苍蝇：亮部偏青绿、暗部偏蓝紫
    g.addColorStop(0, hsl(h, s * .9, Math.min(.92, l + .26)));
    g.addColorStop(.5, hsl(h, s, l));
    g.addColorStop(1, hsl((h + 28) % 360, s * .95, Math.max(.06, l - .3)));
  } else {
    g.addColorStop(0, hsl(h, s * .85, Math.min(.96, l + .20)));
    g.addColorStop(.5, hsl(h, s, l));
    g.addColorStop(1, hsl(h, s * .92, Math.max(.05, l - .24)));
  }
  return g;
}

// 绒毛：沿轮廓法线画短线
function fuzz(ctx, cx, cy, rx, ry, n, len, g, color) {
  ctx.strokeStyle = color; ctx.lineWidth = .9; ctx.lineCap = 'round';
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + (g.jitter[i % 40] - .5) * .3;
    const px = cx + Math.cos(a) * rx, py = cy + Math.sin(a) * ry;
    const na = Math.atan2(Math.sin(a) / ry, Math.cos(a) / rx);
    const L = len * (.6 + g.jitter[(i + 7) % 40] * .8);
    ctx.beginPath();
    ctx.moveTo(px, py);
    ctx.lineTo(px + Math.cos(na) * L + (g.jitter[(i + 3) % 40] - .5) * 1.5,
               py + Math.sin(na) * L + (g.jitter[(i + 11) % 40] - .5) * 1.5);
    ctx.stroke();
  }
}

// 刚毛：粗硬短刺，带向后倾角（+tailBias 尾向偏置 0..1）——苍蝇体表质感的关键
function bristles(ctx, cx, cy, rx, ry, n, len, g, color, tailBias = .45) {
  ctx.strokeStyle = color; ctx.lineWidth = 1.05; ctx.lineCap = 'round';
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + (g.jitter[(i * 3) % 40] - .5) * .5;
    const px = cx + Math.cos(a) * rx, py = cy + Math.sin(a) * ry;
    const na = Math.atan2(Math.sin(a) / ry, Math.cos(a) / rx);
    // 法线方向与尾向(+90°)混合 → 刺整体朝尾后方倒
    const dx = Math.cos(na) * (1 - tailBias) + Math.cos(1.57) * tailBias;
    const dy = Math.sin(na) * (1 - tailBias) + Math.sin(1.57) * tailBias;
    const da = Math.atan2(dy, dx);
    const L = len * (.55 + g.jitter[(i + 5) % 40] * .9);
    ctx.beginPath();
    ctx.moveTo(px, py);
    ctx.lineTo(px + Math.cos(da) * L + (g.jitter[(i + 9) % 40] - .5) * .8,
               py + Math.sin(da) * L + (g.jitter[(i + 13) % 40] - .5) * .8);
    ctx.stroke();
  }
}

// 翅膀：贝塞尔 + 翅脉 + 边缘光；phase 0..1 控制拍打
// rest > 0：平铺收拢姿态（fly）——翅面向下平压在背上，沿身体纵轴向后掠过，
// 交叉盖住腹部（翅膀尖越过后中线，左右翅像小盾牌叠在背上）
function wing(ctx, g, side, phase, scaleF, isHind, time = 0, rest = 0, restOff = 0) {
  const len = 46 * g.wingLen * (isHind ? g.hindRatio : 1) * scaleF;
  const wid = 26 * g.wingW * (isHind ? g.hindRatio * 1.05 : 1) * scaleF;
  const n = g.noise, w = g.wc;

  ctx.save();
  if (rest > 0) {
    // 平铺姿态：翅面压扁（高度 * .18）后旋转，把沿 +x 的翅尖带到沿 +y 的尾端。
    // 左右镜像由外层 scale(side,1) 提供，旋转同为 +90°（向尾部），两侧翅尖在尾端交叉。
    // restOff：一侧角度略大/略小，让左右翅尖前后错开（更接近真实叠翅）
    ctx.scale(1, .18);
    ctx.rotate(rest * (1.32 + restOff) + Math.sin(time * 1.1) * .03);
  } else {
    const flapK = .22 + .78 * (1 - phase);   // 拍打：水平投影变化
    ctx.scale(flapK * side, 1);
  }

  // 轮廓：slim（蝇）= 直角三角形——前缘完全笔直、后缘是大弧外鼓收尖，
  // 翅尖在远端；round（默认）= 圆润椭圆
  const slim = g.wingStyle === 'slim';
  ctx.beginPath();
  ctx.moveTo(0, 0);
  if (slim) {
    // 前缘：完全直线到翅尖（最小波动防机械感）
    ctx.lineTo(len, -wid * .04 + n(1.7) * 2);
    // 后缘：从翅尖大弧外鼓（约 50% 处达最宽），再缓收回翅根
    ctx.bezierCurveTo(len * .92, wid * .35,
                      len * .70, wid * .96,
                      len * .48, wid * .98 + n(3.1) * 1.5);
    ctx.bezierCurveTo(len * .24, wid * .94, len * .06, wid * .42, 0, 0);
  } else {
    ctx.bezierCurveTo(len * .28 + w[0] * 8, -wid * .82 + w[1] * 6,
                      len * .78 + w[2] * 8, -wid * .62 + w[3] * 6,
                      len, -wid * .06 + n(1.7) * 3);
    ctx.bezierCurveTo(len * 1.04, wid * .48, len * .62 + w[1] * 6, wid * .92,
                      len * .26, wid * .74 + n(3.1) * 2.5);
    ctx.bezierCurveTo(len * .08, wid * .46, len * .02, wid * .18, 0, 0);
  }
  ctx.closePath();

  // 膜质填充：翅根琥珀晕 → 翅尖全透明（真实蝇翅），底色极淡，轮廓靠翅脉/描边撑
  const grad = ctx.createLinearGradient(0, -wid, len, wid);
  const wh = isHind ? g.hue : (g.hue + 12) % 360;
  const restA = rest > 0 ? .30 : .5;   // 平铺更透
  grad.addColorStop(0, `hsla(30, 55%, 48%, ${.34 * restA})`);           // 翅根琥珀
  grad.addColorStop(.3, hsl(wh, .35, .78, .16 * restA));                // 中段微冷反光
  grad.addColorStop(1, hsl(wh, .3, .92, .04 * restA));                  // 翅尖近透明
  ctx.fillStyle = grad;
  ctx.fill();

  // 翅缘描边：浅色薄膜边界，平铺时清晰（盖在背上的膜片轮廓）
  ctx.strokeStyle = `hsla(45, 25%, ${rest > 0 ? 88 : 78}%, ${rest > 0 ? .45 : .25})`;
  ctx.lineWidth = .8;
  ctx.stroke();

  // 琥珀翅基：翅根暖色晕（蝇翅特征），沿翅面前 1/3 渐隐
  if (!isHind && g.species === 'fly') {
    const ag = ctx.createRadialGradient(0, 0, len * .04, 0, 0, len * .55);
    ag.addColorStop(0, 'hsla(28, 60%, 42%, .30)');
    ag.addColorStop(.6, 'hsla(24, 55%, 45%, .12)');
    ag.addColorStop(1, 'hsla(24, 55%, 45%, 0)');
    ctx.save(); ctx.clip();
    ctx.fillStyle = ag; ctx.fillRect(-2, -wid * 1.2, len * 1.1, wid * 2.4);
    ctx.restore();
  }

  // 晶翼：通透 + 棱镜折线
  if (g.fx && g.fx.crystal && !isHind) {
    ctx.save(); ctx.clip();
    for (let i = 0; i < 4; i++) {
      const a = -.9 + i * .42;
      ctx.strokeStyle = `hsla(${(time * 60 + i * 90) % 360},90%,72%,.34)`;
      ctx.lineWidth = 1.6;
      ctx.beginPath();
      ctx.moveTo(0, 0);
      ctx.lineTo(Math.cos(a) * len * 1.2, Math.sin(a) * wid * 1.6);
      ctx.stroke();
    }
    ctx.restore();
  }

  // 翅脉：蝇脉型——前缘 2 条直脉（粗/细）、3 条纵脉弧形后展、第 4 纵脉分叉、翅窗横脉
  const veinP = `hsla(40, 35%, ${rest > 0 ? 84 : 70}%, ${.30 + g.wingAlpha * .4})`;
  const veinB = `hsla(40, 25%, ${rest > 0 ? 76 : 60}%, ${.25 + g.wingAlpha * .35})`;
  // 1. 前缘脉（最粗、紧贴前缘直线）
  ctx.strokeStyle = `hsla(45, 35%, ${rest > 0 ? 84 : 72}%, ${.38 + g.wingAlpha * .4})`;
  ctx.lineWidth = 1.0;
  ctx.beginPath();
  ctx.moveTo(len * .03, wid * .01);
  ctx.lineTo(len, -wid * .03 + (rest > 0 ? 0 : 0));
  ctx.stroke();
  // 2. 次前缘脉（与前缘脉平行、稍后）
  ctx.lineWidth = .8;
  ctx.beginPath();
  ctx.moveTo(len * .06, wid * .03);
  ctx.lineTo(len * .98, wid * .08);
  ctx.stroke();
  // 3. 纵脉 3（弧形后展）
  ctx.strokeStyle = veinB; ctx.lineWidth = .7;
  ctx.beginPath();
  ctx.moveTo(len * .08, wid * .05);
  ctx.bezierCurveTo(len * .30, wid * .12, len * .55, wid * .24, len * .90, wid * .42);
  ctx.stroke();
  // 4. 纵脉 4（关键分叉：先分两叉再汇）
  ctx.beginPath();
  ctx.moveTo(len * .10, wid * .06);
  ctx.bezierCurveTo(len * .30, wid * .18, len * .46, wid * .28, len * .56, wid * .40);
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(len * .10, wid * .06);
  ctx.bezierCurveTo(len * .26, wid * .30, len * .40, wid * .50, len * .56, wid * .56);
  ctx.stroke();
  // 5. 纵脉 5（弧线外鼓到后缘中段）
  ctx.beginPath();
  ctx.moveTo(len * .12, wid * .08);
  ctx.bezierCurveTo(len * .34, wid * .44, len * .52, wid * .76, len * .68, wid * .90);
  ctx.stroke();
  // 6. 翅窗横脉（连接纵脉3-4 之间的"窗"）
  ctx.lineWidth = .5;
  ctx.beginPath();
  ctx.moveTo(len * .54, wid * .40);
  ctx.bezierCurveTo(len * .60, wid * .34, len * .64, wid * .32, len * .54, wid * .36);
  ctx.stroke();

  // 蝴蝶眼斑
  if (g.spots && !isHind) {
    for (let i = 0; i < g.spots; i++) {
      const a = -.5 + i * .45;
      ctx.beginPath();
      ctx.arc(len * (.42 + i * .16), -wid * (.34 + (i % 2) * .3), 3.4 * scaleF, 0, 7);
      ctx.fillStyle = hsl(g.patternHue, .75, .32, .85); ctx.fill();
      ctx.beginPath();
      ctx.arc(len * (.42 + i * .16) - .8, -wid * (.34 + (i % 2) * .3) - .8, 1.3 * scaleF, 0, 7);
      ctx.fillStyle = 'rgba(255,255,255,.75)'; ctx.fill();
    }
  }
  ctx.restore();
}

function drawInsect(ctx, g, time, size, flying, facing = 1, extra = null) {
  const S = size / 120;
  const alt = (extra && extra.alt) || 0;
  ctx.save();
  ctx.translate(size / 2, size / 2);
  if (facing === -1) ctx.scale(-1, 1);
  ctx.scale(S, S);

  const flyPhase = flying ? (Math.sin(time * 26) * .5 + .5) : (Math.sin(time * 1.6) * .5 + .5) * .12;
  const bob = flying ? Math.sin(time * 13) * 2.2 : Math.sin(time * 1.1) * .8;
  // 蝇类休息姿：双翅向后掠拢在背上
  const wingRest = (!flying && g.species === 'fly') ? 1 : 0;

  // --- 环境阴影 ---
  ctx.save();
  ctx.translate(3, 26 + bob * .3 + alt * .5);
  ctx.scale(1, .32);
  const sh = ctx.createRadialGradient(0, 0, 2, 0, 0, 34);
  sh.addColorStop(0, 'rgba(0,0,0,.34)');
  sh.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.fillStyle = sh;
  ctx.beginPath(); ctx.arc(0, 0, 34, 0, 7); ctx.fill();
  ctx.restore();

  // 行为钩子：飞行朝向旋转（影子不随转）+ 悬空升降
  if (extra && extra.rot) ctx.rotate(extra.rot);
  ctx.translate(0, bob - alt * .7);

  const thoraxY = -6, thoraxR = 11 * g.bodyW, abdomenTop = thoraxY + thoraxR * .55;
  const abdLen = 40 * g.bodyLen, segN = g.segs;

  const fx = g.fx || {};
  // 荧光 / 霓虹：整只虫外发光
  ctx.save();
  if (fx.glow) { ctx.shadowColor = hsl(g.hue, .9, .62, .95); ctx.shadowBlur = 16; }
  else if (fx.neon) { ctx.shadowColor = hsl((g.hue + 30) % 360, 1, .58, .9); ctx.shadowBlur = 10; }

  // --- 后翅（蝇类退化成平衡棒大小；休息平铺时藏起）---
  if (g.wingLen > .2 && wingRest === 0) {
    ctx.save(); ctx.translate(0, thoraxY + 4);
    wing(ctx, g, -1, flyPhase * .88, 1, true, time, wingRest);
    wing(ctx, g, 1, flyPhase * .88, 1, true, time, wingRest);
    ctx.restore();
  }

  // --- 平衡棒（蝇类后翅退化成的小棒，飞行时可见，柄 + 圆头）---
  if (g.species === 'fly' && flying) {
    ctx.strokeStyle = 'rgba(20,22,26,.9)'; ctx.lineWidth = .9;
    for (let s = -1; s <= 1; s += 2) {
      const hx = s * thoraxR * .45, hy = thoraxY + 8;
      ctx.beginPath(); ctx.moveTo(hx, hy); ctx.lineTo(s * 8, hy + 5); ctx.stroke();
      ctx.beginPath(); ctx.arc(s * 9, hy + 5.8, 1.6, 0, 7);
      ctx.fillStyle = 'rgba(18,20,24,.95)'; ctx.fill();
    }
  }

  // --- 腿（蝇类：近黑）---
  ctx.strokeStyle = g.species === 'fly'
    ? 'rgba(14,16,20,.95)'
    : hsl(g.hue, .3, Math.max(.08, g.light - .32), .92);
  ctx.lineCap = 'round';
  for (let s = -1; s <= 1; s += 2) {
    for (let i = 0; i < 3; i++) {
      const y0 = thoraxY - 3 + i * 4.5;
      const sp = (i - 1) * .34;
      const ll = 20 * g.legLen;
      const sw = flying ? .55 : 1;
      // 行为钩子：走路腿部摆动 / 搓腿（前足）
      let wob = 0, wobY = 0;
      if (extra && extra.legMode === 'walk') {
        const wp = extra.legPhase * 13 + i * 2.1 + (s > 0 ? Math.PI : 0);
        wob = Math.sin(wp) * 2.6 * sw; wobY = Math.max(0, Math.cos(wp)) * 1.1;
      }
      if (extra && extra.legMode === 'groom' && i === 0) {
        wob = Math.sin(extra.legPhase * 11) * 3.2; wobY = Math.cos(extra.legPhase * 11) * 1.6;
      }
      const mx = s * ll * .58 + wob, my = y0 + 6 * sw + sp * 4 - wobY;
      const ex = s * ll * (flying ? .78 : 1.02) + wob * 1.5
               - (extra && extra.legMode === 'groom' && i === 0 ? s * 4 : 0),
            ey = y0 + 14 * sw + sp * 7 - wobY;
      ctx.lineWidth = 2.3;
      ctx.beginPath(); ctx.moveTo(s * thoraxR * .55, y0);
      ctx.quadraticCurveTo(mx, my - 2, mx * 1.02, my); ctx.stroke();
      ctx.lineWidth = 1.4;
      ctx.beginPath(); ctx.moveTo(mx, my);
      ctx.quadraticCurveTo(ex * .9, ey - 1, ex, ey + 1.5); ctx.stroke();
    }
  }

  // --- 腹部（分节 + 错相位波动 = 生命感） ---
  // 沿腹部的连续波动函数：tt = 0(前端) → 1(尾端)。
  // 体节、花纹、绒毛全部共用它，保证附着元素严格跟随身体形变（否则花纹会「浮」在表面）
  const waveFreq = flying ? 9 : 2.2, waveAmp = flying ? 1.9 : 1.0;
  const abdWave = (tt) =>
    Math.sin(time * waveFreq + tt * (segN - 1) * .95) * waveAmp * (.4 + tt * .6);

  for (let i = segN - 1; i >= 0; i--) {
    const t = i / Math.max(1, segN - 1);
    const w = (g.species === 'fly' ? 17 : 15) * g.bodyW * (.52 + .68 * Math.sin(Math.PI * (.22 + .78 * t)));
    const segH = abdLen / segN * 1.12;
    const cy = abdomenTop + (t * (abdLen - segH * .4));
    // 体节波动：相邻节相位差
    const cx = abdWave(t);
    const ry = segH * .58;

    ctx.beginPath();
    ctx.ellipse(cx, cy, w, ry, 0, 0, 7);
    ctx.fillStyle = bodyGrad(ctx, cx, cy, w, ry, g.hue, g.sat, g.light);
    ctx.fill();

    // 节间深色缝（蝇类甲壳连成一体，缝更淡）
    if (i < segN - 1) {
      const seamA = g.species === 'fly' ? .22 : .55;
      ctx.strokeStyle = hsl(g.hue, g.sat, Math.max(.05, g.light - .3), seamA);
      ctx.lineWidth = 1.1;
      ctx.beginPath();
      ctx.ellipse(cx, cy + ry * .82, w * .94, ry * .3, 0, .15, Math.PI - .15);
      ctx.stroke();
    }
    // 高光（中轴偏左；蝇类弱化，避免毛毛虫感）
    const hlK = g.species === 'fly' ? .55 : 1;
    ctx.beginPath();
    ctx.ellipse(cx - w * .3, cy - ry * .34, w * .3, ry * .26, -.35, 0, 7);
    ctx.fillStyle = `rgba(255,255,255,${(.10 + g.gloss * .20) * hlK})`;
    ctx.fill();
  }

  // --- 腹部花纹（严格跟随体节波动，避免「浮在表面」）---
  const segH0 = abdLen / segN * 1.12;
  const segW = (t) => (g.species === 'fly' ? 17 : 15) * g.bodyW * (.52 + .68 * Math.sin(Math.PI * (.22 + .78 * t)));
  const segY = (t) => abdomenTop + t * (abdLen - segH0 * .4);
  // 由 y 反推沿腹部的归一化位置 → 取该处波动偏移
  const abdSpan = Math.max(1, abdLen - segH0 * .4);
  const ttOf = (y) => Math.max(0, Math.min(1, (y - abdomenTop) / abdSpan));

  ctx.save();
  ctx.beginPath();
  for (let i = 0; i < segN; i++) {
    const t = i / Math.max(1, segN - 1);
    // 与体节完全同心的裁剪区（略收 3% 防止花纹溢出轮廓）
    ctx.ellipse(abdWave(t), segY(t), segW(t) * .97, segH0 * .58 * .97, 0, 0, 7);
  }
  ctx.clip();

  const pHue = g.patternHue, pAlpha = .5 + g.patternDensity * .45;

  // 横带：上下边界各自按所在位置的波动偏移，带子会随身体一起扭
  const stripe = (y, h) => {
    const y0 = y - h * .5, y1 = y + h * .5;
    const o0 = abdWave(ttOf(y0)), o1 = abdWave(ttOf(y1));
    ctx.beginPath();
    ctx.moveTo(-32 + o0, y0); ctx.lineTo(32 + o0, y0);
    ctx.lineTo(32 + o1, y1);  ctx.lineTo(-32 + o1, y1);
    ctx.closePath(); ctx.fill();
  };

  if (g.pattern === 'stripe') {
    const n = Math.round(2 + g.patternDensity * 4);
    for (let i = 0; i < n; i++) {
      ctx.fillStyle = hsl(pHue, .55, .12, pAlpha * .9);
      stripe(abdomenTop + (i + .5) * (abdLen / n), abdLen / n * .42);
    }
  } else if (g.pattern === 'spot') {
    const n = Math.round(3 + g.patternDensity * 5);
    for (let i = 0; i < n; i++) {
      const j = g.jitter[i % 40];
      const y = abdomenTop + 6 + (i / n) * abdLen * .85;
      const tt = ttOf(y);
      // 局部切线角：斑点随身体扭转而倾斜，像贴在曲面上
      const rot = (abdWave(Math.min(1, tt + .03)) - abdWave(Math.max(0, tt - .03)))
                / (.06 * abdSpan) * .55;
      ctx.save();
      ctx.translate(abdWave(tt), y);
      ctx.rotate(rot);
      ctx.beginPath();
      ctx.ellipse((j - .5) * segW(tt) * 1.3, 0, 2.4 + j * 2.6, 2.0 + j * 2.0, 0, 0, 7);
      ctx.fillStyle = hsl(pHue, .6, .18, pAlpha);
      ctx.fill();
      ctx.restore();
    }
  } else if (g.pattern === 'band') {
    ctx.fillStyle = hsl(pHue, .7, .3, pAlpha * .8);
    stripe(abdomenTop + abdLen * .52, abdLen * .2);
  } else if (g.pattern === 'gradient') {
    // 渐变分带近似，每带跟随波动，避免整块矩形「贴片感」
    const n = 8;
    for (let i = 0; i < n; i++) {
      const k = i / (n - 1);
      ctx.fillStyle = hsl(pHue, .6, .3 - k * .14, pAlpha * .8 * k);
      stripe(abdomenTop + (i + .5) * (abdLen / n), abdLen / n * 1.05);
    }
  } else if (g.pattern === 'bands') {
    // 蝇类腹部横带：每节前缘一条深色横带、随体节波动
    const n = Math.max(3, segN);
    for (let i = 0; i < n; i++) {
      const y = abdomenTop + (i + .15) * (abdLen / n);
      const bandH = abdLen / n * .35;
      ctx.fillStyle = hsl(pHue, .35, .05, pAlpha * .78);
      stripe(y, bandH);
    }
  }
  ctx.restore();

  // --- 腹末金属蓝泽（丽蝇特征）：沿腹部后 1/3 渐显的青蓝光泽 ---
  ctx.save();
  ctx.beginPath();
  for (let i = 0; i < segN; i++) {
    const t = i / Math.max(1, segN - 1);
    ctx.ellipse(abdWave(t), segY(t), segW(t) * .97, segH0 * .58 * .97, 0, 0, 7);
  }
  ctx.clip();
  const mg = ctx.createLinearGradient(0, abdomenTop, 0, abdomenTop + abdLen);
  mg.addColorStop(.55, 'hsla(200, 80%, 48%, 0)');
  mg.addColorStop(.84, 'hsla(202, 75%, 42%, .22)');
  mg.addColorStop(1, 'hsla(198, 85%, 50%, .42)');
  ctx.fillStyle = mg;
  ctx.fillRect(-34, abdomenTop, 68, abdLen + 4);
  ctx.restore();

  // 尾端刚毛（跟着最后一节波动）
  if (g.species === 'fly') {
    const tEnd = 1, oEnd = abdWave(tEnd);
    bristles(ctx, oEnd, segY(tEnd), segW(tEnd) * .9, segH0 * .5, 10, 4.2 * g.fuzz, g,
             'rgba(8,10,14,.8)', .62);
  }

  // --- 绒毛（同样长在体节上，跟随波动）---
  if (g.fuzz > .25) {
    for (let i = 0; i < segN; i++) {
      const t = i / Math.max(1, segN - 1);
      fuzz(ctx, abdWave(t), segY(t), segW(t), segH0 * .53, 9, 3.4 * g.fuzz, g,
           hsl(g.hue, g.sat * .6, Math.min(.92, g.light + .3), .5 * g.fuzz));
    }
  }

  // --- 胸部 ---
  ctx.beginPath();
  ctx.ellipse(0, thoraxY, thoraxR, thoraxR * .86, 0, 0, 7);
  ctx.fillStyle = bodyGrad(ctx, 0, thoraxY, thoraxR, thoraxR * .86, g.hue, g.sat, g.light * .94);
  ctx.fill();

  if (g.species === 'fly') {
    // 中胸纵纹：4 条深色纵带（clip 在胸椭圆内）
    ctx.save(); ctx.clip();
    ctx.fillStyle = 'rgba(6,8,12,.5)';
    for (const kx of [-.62, -.22, .22, .62]) {
      const x0 = kx * thoraxR;
      ctx.beginPath();
      ctx.ellipse(x0, thoraxY, thoraxR * .09, thoraxR * .8, 0, 0, 7);
      ctx.fill();
    }
    ctx.restore();
    // 刚毛（替代软绒毛）：胸缘向后倒的黑刺
    bristles(ctx, 0, thoraxY, thoraxR * .96, thoraxR * .82, 18, 4.6 * g.fuzz, g,
             'rgba(8,10,14,.85)', .5);
  } else if (g.fuzz > .3) {
    fuzz(ctx, 0, thoraxY, thoraxR * .98, thoraxR * .84, 16, 4.2 * g.fuzz, g,
         hsl(g.hue, g.sat * .5, Math.min(.95, g.light + .34), .55 * g.fuzz));
  }
  // 光泽
  ctx.beginPath();
  ctx.ellipse(-thoraxR * .34, thoraxY - thoraxR * .34, thoraxR * .42, thoraxR * .26, -.5, 0, 7);
  ctx.fillStyle = `rgba(255,255,255,${.08 + g.gloss * .26})`;
  ctx.fill();

  // --- 前翅（在身体之上，覆盖；休息时平铺盖背、翅尖尾端交叉错开） ---
  if (g.wingLen > .2) {
    ctx.save(); ctx.translate(0, thoraxY - 1);
    wing(ctx, g, -1, flyPhase, 1, false, time, wingRest, -.09);
    wing(ctx, g, 1, flyPhase, 1, false, time, wingRest, .09);
    ctx.restore();
  }
  ctx.restore(); // 结束 glow/neon 发光作用域

  // --- 头部 ---
  const headY = thoraxY - thoraxR * .92, headR = 7.6 * g.eyeSize * 1.14;
  ctx.beginPath();
  ctx.ellipse(0, headY, headR * 1.06, headR * .88, 0, 0, 7);
  ctx.fillStyle = bodyGrad(ctx, 0, headY, headR, headR * .88, g.hue, g.sat * .8, g.light * .88);
  ctx.fill();

  // 复眼（带高光 → 立刻有神）；蝇类：红棕色大复眼占满头部
  const isFly = g.species === 'fly';
  const eyeR = 4.6 * g.eyeSize * (isFly ? 1.22 : 1);
  for (let s = -1; s <= 1; s += 2) {
    const ex = s * headR * (isFly ? .52 : .62);
    ctx.beginPath();
    ctx.ellipse(ex, headY - .5, eyeR * (isFly ? .82 : .72), eyeR, s * .18, 0, 7);
    const eg = ctx.createRadialGradient(ex - 1, headY - 2, .5, ex, headY - .5, eyeR * 1.2);
    const eh = isFly ? 12 : 25;                       // 12 = 红棕
    eg.addColorStop(0, hsl(eh, isFly ? .68 : .55, isFly ? .40 : .34));
    eg.addColorStop(.7, hsl(eh, .6, .20));
    eg.addColorStop(1, hsl(eh, .55, .07));
    ctx.fillStyle = eg; ctx.fill();
    // 小facet反光点×2（复眼质感）
    ctx.beginPath();
    ctx.arc(ex - eyeR * .28, headY - eyeR * .42, eyeR * .24, 0, 7);
    ctx.fillStyle = 'rgba(255,240,225,.78)'; ctx.fill();
    ctx.beginPath();
    ctx.arc(ex + eyeR * .18, headY - eyeR * .55, eyeR * .11, 0, 7);
    ctx.fillStyle = 'rgba(255,225,200,.5)'; ctx.fill();
  }

  // 王冠：头顶三尖冠饰
  if (fx.crown) {
    const cy0 = headY - headR * .92;
    ctx.beginPath();
    ctx.moveTo(-headR * 1.05, cy0 + 3);
    ctx.lineTo(-headR * .5, cy0 - 5);
    ctx.lineTo(-headR * .18, cy0 + 1);
    ctx.lineTo(0, cy0 - 8.5);
    ctx.lineTo(headR * .18, cy0 + 1);
    ctx.lineTo(headR * .5, cy0 - 5);
    ctx.lineTo(headR * 1.05, cy0 + 3);
    ctx.closePath();
    const cg = ctx.createLinearGradient(0, cy0 - 8, 0, cy0 + 3);
    cg.addColorStop(0, 'hsl(48,95%,78%)');
    cg.addColorStop(1, 'hsl(38,85%,52%)');
    ctx.fillStyle = cg; ctx.fill();
    ctx.strokeStyle = 'rgba(120,80,10,.55)'; ctx.lineWidth = .7; ctx.stroke();
    // 冠上宝石
    ctx.beginPath(); ctx.arc(0, cy0 - 2.2, 1.5, 0, 7);
    ctx.fillStyle = 'hsl(340,80%,62%)'; ctx.fill();
  }

  // 绶带：腹部拖尾，跟随身体摆动
  if (fx.ribbon) {
    const rbY = abdomenTop + abdLen * .95;
    for (let s = -1; s <= 1; s += 2) {
      const sw = Math.sin(time * (flying ? 8 : 2.4) + (s > 0 ? 0 : 1.1)) * (flying ? 5 : 2.4);
      ctx.beginPath();
      ctx.moveTo(s * 2.5, rbY);
      ctx.bezierCurveTo(s * 7 + sw, rbY + 12,
                        s * 3 + sw * 1.7, rbY + 22,
                        s * 9 + sw * 2.2, rbY + 31);
      ctx.lineWidth = 3.4; ctx.lineCap = 'round';
      ctx.strokeStyle = hsl((g.hue + 20) % 360, .8, Math.min(.85, g.light + .22), .82);
      ctx.stroke();
      ctx.lineWidth = 1.1;
      ctx.strokeStyle = hsl((g.hue + 40) % 360, .9, .78, .5);
      ctx.stroke();
    }
  }

  // 触角（惯性摆动）
  const sway = Math.sin(time * (flying ? 7 : 1.7)) * (flying ? 2.6 : 1.5);
  ctx.strokeStyle = hsl(g.hue, .3, Math.max(.1, g.light - .18), .95);
  for (let s = -1; s <= 1; s += 2) {
    const ax = s * headR * .42, ay = headY - headR * .5;
    const al = 15 * g.antennaLen;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(ax, ay);
    ctx.quadraticCurveTo(ax + s * al * .55 + sway * .5, ay - al * .62,
                         ax + s * al * .82 + sway, ay - al);
    ctx.stroke();
    if (g.species === 'butterfly') {                    // 棒状末端
      ctx.beginPath();
      ctx.arc(ax + s * al * .82 + sway, ay - al, 2.1, 0, 7);
      ctx.fillStyle = hsl(g.hue, .4, Math.max(.1, g.light - .25)); ctx.fill();
    }
  }

  // --- 华彩阶段 / 星尘词条：环绕粒子 ---
  if (g.stage >= 4 || fx.stardust) {
    const n = fx.stardust ? 14 : 7;
    const big = fx.stardust ? 1.35 : 1;
    for (let i = 0; i < n; i++) {
      const a = time * 1.4 + i * (6.283 / n);
      const r = (26 + Math.sin(time * 2 + i) * 7) * big;
      const px = Math.cos(a) * r, py = Math.sin(a) * r * .6;
      const al = (.25 + .45 * (Math.sin(time * 3 + i * 2) * .5 + .5)) * (fx.stardust ? 1.1 : .8);
      ctx.beginPath();
      ctx.arc(px, py, (1.5 + (i % 3) * .6) * big, 0, 7);
      ctx.fillStyle = hsl((g.hue + 45) % 360, .9, .75, Math.min(1, al));
      ctx.fill();
    }
  }
  ctx.restore();
}

// 卵 / 幼虫：低阶段形态
function drawEgg(ctx, g, time, size) {
  const S = size / 120;
  ctx.save();
  ctx.translate(size / 2, size / 2 + 6);
  ctx.scale(S, S);
  const breathe = 1 + Math.sin(time * 1.5) * .012;
  ctx.save(); ctx.translate(2, 30); ctx.scale(1, .3);
  const sh = ctx.createRadialGradient(0, 0, 2, 0, 0, 26);
  sh.addColorStop(0, 'rgba(0,0,0,.32)'); sh.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.fillStyle = sh; ctx.beginPath(); ctx.arc(0, 0, 26, 0, 7); ctx.fill();
  ctx.restore();

  ctx.scale(breathe, breathe);
  ctx.beginPath();
  ctx.ellipse(0, 0, 19, 25, Math.sin(time * .8) * .05, 0, 7);
  ctx.fillStyle = bodyGrad(ctx, 0, 0, 19, 25, g.hue, g.sat * .8, Math.min(.82, g.light + .22));
  ctx.fill();
  ctx.save(); ctx.clip();
  for (let i = 0; i < 9; i++) {
    const j = g.jitter[i % 40];
    ctx.beginPath();
    ctx.ellipse((j - .5) * 26, (g.jitter[(i + 5) % 40] - .5) * 44, 2 + j * 2.6, 1.8 + j * 2, 0, 0, 7);
    ctx.fillStyle = hsl(g.patternHue, .5, Math.max(.15, g.light - .2), .55); ctx.fill();
  }
  const gl = ctx.createLinearGradient(-19, -25, 19, 25);
  gl.addColorStop(0, 'rgba(255,255,255,.34)'); gl.addColorStop(.5, 'rgba(255,255,255,0)');
  ctx.fillStyle = gl; ctx.fillRect(-25, -30, 50, 60);
  ctx.restore();
  ctx.restore();
}

// ESM 导出：供 module 型脚本（feed-anim.js）import；<script src> 方式仍作为全局可用
export { mulberry32, makeNoise, R, pick, RARITY, TRAITS, rollTraits, traitCountFor, SPECIES,
         makeGenome, applyTraitEffects, evolve, drawInsect, drawEgg };
