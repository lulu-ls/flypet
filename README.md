# FlyPet

桌面飞虫宠物：一只蝴蝶（后续支持苍蝇/蜜蜂/甲虫）停在桌面上，鼠标靠近就惊飞，飞行时转向跟随运动方向，滑翔落地后保持落地朝向。macOS / Windows 通用。

## 当前状态

- **可运行的桌面宠物**（Tauri 2 + Rust 状态机 + Three.js WebGL 渲染蝴蝶 GLB 模型）
- 蝴蝶模型：`ui/models/butterfly.glb`（Sketchfab 帝王蝶，CC-BY-4.0，3 段烘焙动画）
- 动画映射：`|3` 待机轻微煽翅 / `|!` 起飞+飞行煽翅 / `|2` 滑行轻煽
- 行为：栖息（等待）→ 鼠标靠近惊飞 → 飞行转向 → 滑翔 → 落地保持朝向

## 环境要求

- macOS（本项目在 macOS 开发验证；Windows 需配好 Tauri 环境）
- [Rust 工具链](https://rustup.rs)（`rustc` + `cargo`）
- Xcode Command Line Tools（macOS 编译必需）：
  ```bash
  xcode-select --install
  ```

## 运行（开发）

```bash
cd /Users/liuxs/github/fly/src-tauri
cargo run
```

`Ctrl+C` 退出。改动 Rust 后重新 `cargo run`；改动前端（`ui/`）后重新 `cargo run`（会自动重新嵌入资源）。

## 构建（打包 .app）

```bash
cd /Users/liuxs/github/fly
npx @tauri-apps/cli@2 build
```

产物：`src-tauri/target/release/bundle/macos/FlyPet.app`

安装到桌面：

```bash
open src-tauri/target/release/bundle/macos/FlyPet.app
```

## 直接运行已编译的调试版

```bash
cd /Users/liuxs/github/fly/src-tauri
./target/debug/flypet
```

## 调试

- **前端控制台**：托盘菜单 →「开发者工具」（打开 Web Inspector）
- **Rust 日志**：`/tmp/flypet.log`（启动时输出 `POS x y pose`，状态切换时输出 `EVENT`）
- **浏览器预览前端**（无 Tauri，内置演示模式，浅灰底便于观察）：
  ```bash
  cd /Users/liuxs/github/fly/ui
  python3 -m http.server 8778
  # 打开 http://localhost:8778/index.html
  ```

## 项目结构

```
fly/
├── src-tauri/            # Rust 壳（Tauri 2）
│   ├── src/
│   │   ├── main.rs       # 主循环：指针 → 状态机 → 窗口移动 → state 事件
│   │   ├── insect.rs     # 状态机 Spawn/Perch/Flee/Glide + 物种注册表
│   │   ├── pointer.rs    # 全局光标
│   │   └── platform.rs   # 工作区 / 缩放
│   └── tauri.conf.json   # 140×140 透明置顶窗口
├── ui/                   # 前端
│   ├── index.html        # 入口（透明背景）
│   ├── pet.js            # Three.js 渲染 + GLB 加载 + 动画映射 + state 驱动
│   ├── vendor/           # three.js 本地化（离线可用）
│   └── models/           # GLB 模型（打包自动嵌入）
├── ui-lab/               # 行为实验室（v2/v3 引擎对比、外观调参）
├── assets/models/        # 原始模型工作区（.fbx/.blend，不入 git）
└── doc/                  # 设计/玩法/路线/疑难排查
```

## 技术要点

- **透明 + 置顶 + 穿透**：`tauri.conf.json` 里 `transparent: true`（macOS 需 `macOSPrivateApi`）、`alwaysOnTop`、运行时 `set_ignore_cursor_events(true)`
- **ESM 加载链**：three.js 已本地化到 `ui/vendor/`，`GLTFLoader`/`BufferGeometryUtils` 的相对导入已修正（见 `doc/troubleshooting.md` §2）
- **GLB 加载**：用 `fetch → loader.parse`（同步解析），规避 WKWebView 对 XHR 流式的兼容问题（见 `doc/troubleshooting.md`）
- **朝向**：Rust 下发实时速度方向 `heading`，前端平滑转向 + 转弯侧倾；落地保留最后飞行朝向

## 已知限制

- **透明窗口 + WebGL**：macOS WKWebView 对透明层 WebGL 合成支持不完整，当前在开发环境已验证可显示；若遇到空白，见 `doc/troubleshooting.md` §1（含回退 Canvas 2D 方案）
- 苍蝇/蜜蜂/甲虫：模型接入中（动画命名规则见 `ui/pet.js` 的 `CLIP_RULES`）

## 文档

- 详细对比、状态机、坐标陷阱、体积预算见 [`doc/design.md`](doc/design.md)
- 玩法系统（进化 / 喂食 / 亲密度 / 说话）见 [`doc/gameplay.md`](doc/gameplay.md)
- 阶段划分见 [`doc/roadmap.md`](doc/roadmap.md)
- 疑难排查记录见 [`doc/troubleshooting.md`](doc/troubleshooting.md)
- 模型资源规范见 [`assets/models/README.md`](assets/models/README.md)

## 联系方式
QQ：11111111
