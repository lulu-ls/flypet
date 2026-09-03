# FlyPet 疑难排查记录

> 记录开发中踩过的坑、定位过程与最终结论，避免重复排查。
> 最近更新：2026-09-01（WebGL + GLB 桌面宠物接入）

---

## 1. Tauri macOS 透明窗口下 WebGL 不显示

### 现象
- 透明置顶窗口（`transparent: true`）里，WebGL 渲染内容**完全不可见**
- 进程、窗口、日志都正常（窗口在移动、状态机在跑），但屏幕上看不到任何东西
- `screencapture` 对这种 layer 置顶 webview 窗口**也截不到内容**（截到的是下层壁纸）

### 根因
- macOS 的 WKWebView 对 **layer-backed 透明窗口 + WebGL** 支持不完整：透明 WebView 内容无法被正确合成显示
- 这是 **Tauri / WKWebView 平台层面的硬约束**，不是代码能修的
- `macOSPrivateApi: true` 只能让 **Canvas 2D** 透明工作，对 WebGL 无效

### 结论
- **WebGL（three.js）在 Tauri macOS 透明桌宠上不可行**（除非牺牲透明度/加边框，丢失"浮在桌面"的核心体验）
- **回退方案**：Canvas 2D 程序化渲染（`ui-lab/render.js` 已验证在透明窗口可见）
- **GLB/3D 模型留作后续升级**：需先解决透明窗口 + WebGL 的兼容（如离屏渲染、自定义 layer、或换渲染方案）

### 关键教训
- 透明窗口 + WebGL 的组合在 macOS 是"看起来能跑实则白屏"，**不要在现有透明窗口上调 WebGL 代码**——先在不透明窗口验证 WebGL，再单独验证透明，最后合并
- 参考 `doc/design.md` 的分阶段验证原则：Tauri → WebGL → GLB → 透明，逐步隔离

---

## 2. ESM 加载链 404（最隐蔽的坑）

### 现象
- `pet.js`（`<script type="module">`）在 Tauri 和 headless Chrome 里**都没执行**
- 页面停留在 HTML 初始状态（调试 div 显示初始文本、`document.title` 未变）
- 用户看到"白色方块"（WKWebView 默认白底，因为 JS 没跑、也没设透明）

### 根因
**GLTFLoader.js 内部一条跨目录相对导入解析失败**：

```js
// GLTFLoader.js 期望
import { toTrianglesDrawMode } from '../utils/BufferGeometryUtils.js';
// 实际目录是 vendor/utils/，应指向
import { toTrianglesDrawMode } from './utils/BufferGeometryUtils.js';
```

而 `BufferGeometryUtils.js` 里对 three 的导入也要修正：

```js
// 错误（相对 vendor/utils/ 找不到）
import ... from './three.module.js';
// 正确（回到上一级 vendor/）
import ... from '../three.module.js';
```

**一条 404 会拖垮整个 ESM 链**：三个文件（three.module.js → GLTFLoader.js → BufferGeometryUtils.js）只要一个相对路径错，module 就整体不执行，且**无 console 错误**（静默失败）。

### 排查方法（headless + node 解析链）
```bash
# 用 node 模拟浏览器 import 解析，找出所有断链
node -e "
const path = require('path'), fs = require('fs');
function check(f) {
  const s = fs.readFileSync(f, 'utf8');
  for (const imp of [...s.matchAll(/from '([^']+)'/g)].map(m => m[1])) {
    if (imp.startsWith('http')) continue;
    const r = path.resolve(path.dirname(f), imp);
    if (!fs.existsSync(r)) console.log('MISSING:', imp, '<-', f);
    else if (r.endsWith('.js')) check(r);
  }
}
check('vendor/GLTFLoader.js');
"
```

### 结论
- **不要用 importmap 的相对路径映射**（`"three": "./vendor/three.module.js"` 在某些环境下解析失败）
- **直接用相对路径 import**，且手动修正第三方库内部的所有相对依赖
- 本地化 third-party ESM 后，**必须全链验证所有 `from '...'` 都能解析**，否则静默失败

---

## 3. GLB 居中缩放顺序 bug

### 现象
- GLB 加载成功（`ADDED size=125.53,...` 说明模型进场景了），但**看不到模型**
- 相机没动，模型却"跑没影了"

### 根因
```js
// 错误：先缩放，再用「原始尺度」的 center 居中
g.scene.scale.setScalar(1.5 / maxSize);
const center = box.getCenter();       // 用的是缩放前的原始 box
g.scene.position.sub(center);         // 模型被挪到负方向，远离相机

// 正确：先缩放，再「重新计算缩放后的包围盒」居中
g.scene.scale.setScalar(1.5 / maxSize);
const box2 = new THREE.Box3().setFromObject(g.scene);  // 缩放后重算
const center = box2.getCenter();
g.scene.position.sub(center);
```

### 结论
- **`setScalar` 之后必须重新 `setFromObject` 再取 center**，否则用旧包围盒居中会偏到视野外
- 网上下载的 GLB 尺度从 0.01m 到 100m 不等，**必须自动归一化 + 缩放后重新居中**，不要用固定相机距离

---

## 4. 复现步骤（WebGL + GLB 在 Tauri 里的完整链路）

### 已验证可行（阶段 2）
1. Tauri 普通窗口（`transparent: false, decorations: true`）
2. WebGL 渲染彩色立方体 ✅
3. GLB 加载 + 缩放居中 + 播动画 ✅（帝王蝶正常显示）

### 失败点汇总
| 阶段 | 结果 | 原因 |
|------|------|------|
| 普通窗口 + WebGL 立方体 | ✅ | 基础管线正常 |
| 普通窗口 + GLB + 动画 | ✅ | 修好 ESM 链 + 居中后正常 |
| 透明窗口 + WebGL | ❌ | WKWebView 透明层不支持 WebGL 合成 |

---

## 5. 项目现状与待办

### 已实现
- `ui/vendor/`：three.module.js + GLTFLoader.js + utils/BufferGeometryUtils.js 本地化（修复 ESM 相对路径）
- `ui/pet.js`：GLB 加载 + 三动画映射（`|3`待机 / `|!`飞行 / `|2`滑行）+ Tauri state 事件驱动
- Rust 状态机 `Pose::Glide`（蝴蝶飞行末段滑翔减速）
- 托盘「开发者工具」入口（`w.open_devtools()`）
- `ui/models/butterfly.glb` 帝王蝶模型（Sketchfab，CC-BY-4.0）

### 待办
- [ ] 透明窗口 + WebGL 兼容方案（离屏渲染 / 自定义 layer / 等 Tauri 更新）
- [ ] 或回退 Canvas 2D 程序化渲染路线（`ui-lab/render.js` 已就绪）
- [ ] 苍蝇模型接入（复用蝴蝶管线，动画命名 `|3`/`|!`/`|2`）

### 复用资产清单
- 模型规范：`assets/models/README.md`
- 动画映射规则：`ui/pet.js` 的 `CLIP_RULES`
