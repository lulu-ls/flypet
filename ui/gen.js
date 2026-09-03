/* ======================= 6. Tauri 集成：渲染主循环 =======================
 * 依赖 ../ui/render.js 提供的 makeGenome / drawInsect / drawEgg。
 * 职责：接收 Rust 状态 → 驱动 Canvas → 暴露给 Tauri。
 */
let species = 'butterfly', stage = 3, seed = 20260831;
let pose = 'perch', facing = 1;

// 同一 (seed,species,stage) 缓存 genome，避免每帧重建
let cacheKey = '', cacheG = null;
function currentGenome() {
  const k = seed + '|' + species + '|' + stage;
  if (k !== cacheKey) {
    cacheKey = k;
    cacheG = applyTraitEffects(evolve(makeGenome(seed, species, stage), stage));
  }
  return cacheG;
}

const W = 120, H = 120, DPR = Math.min(2, window.devicePixelRatio || 1);
const main = document.getElementById('main');
main.width = W * DPR; main.height = H * DPR;
main.style.width = W + 'px'; main.style.height = H + 'px';
const ctx = main.getContext('2d');
ctx.scale(DPR, DPR);

let t0 = performance.now(), tAnim = 0;
function render(now) {
  if (!window.__paused) tAnim = (now - t0) / 1000;
  ctx.clearRect(0, 0, W, H);
  const g = currentGenome();
  const flying = pose === 'flee' || pose === 'spawn';
  if (g.stage <= 1) drawEgg(ctx, g, tAnim, W);
  else drawInsect(ctx, g, tAnim, W, flying, facing);
  requestAnimationFrame(render);
}
requestAnimationFrame(render);

// ---- Rust → 前端状态同步 ----
function applyState(s) {
  if (!s) return;
  if (s.pose) pose = s.pose;
  if (typeof s.facing === 'number') facing = s.facing;
  if (s.species) species = s.species;
  if (s.stage) stage = s.stage;
  if (typeof s.seed === 'number') seed = s.seed;
}
(async () => {
  const t = window.__TAURI__;
  if (!t) return;
  try { applyState(await t.core.invoke('state')); } catch (e) {}
  try { t.event.listen('state', (e) => applyState(e.payload)); } catch (e) {}
})();
