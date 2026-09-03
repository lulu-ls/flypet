<div align="center">

# 🦋 FlyPet

**一只住在桌面上的小飞虫**

鼠标靠近会惊飞 · 空闲时自己起落遛弯 · 喂食攒亲密度

[![Release](https://img.shields.io/github/v/release/lulu-ls/flypet?label=%E4%B8%8B%E8%BD%BD&color=e8a33d)](https://github.com/lulu-ls/flypet/releases)
[![Build](https://github.com/lulu-ls/flypet/actions/workflows/release.yml/badge.svg)](https://github.com/lulu-ls/flypet/actions/workflows/release.yml)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)](#安装)

</div>

---

## 这是什么

FlyPet 是一只栖息在你屏幕上的桌面飞虫。它有自己的脾气和生活节奏——平时停在桌面上休息，你一靠近它就扑棱着翅膀逃走；喂它吃东西能攒亲密度，处得越熟它越愿意主动飞到你身边转圈表演。

> 目前有 **蝴蝶** / **蜻蜓** / **苍蝇** 三位住户，各自有独立的性格参数和成长档案。

## 特性

- 🦋 **仿真的飞行行为** —— 螺旋起飞、之字巡航、减速滑翔、贴边停靠，每个物种的飞行风格都不同（蝴蝶飘忽、蜻蜓笔直疾飞、苍蝇急促抖拐）
- 🖱️ **会躲人** —— 鼠标快速靠近会受惊起飞；静止不动够久，它反而会主动飞过来亲近你
- 🍯 **投喂养成** —— 投喂随机抽出七品级食物（凡 → 神），攒亲密度解锁亲近行为，每次投喂有 10 分钟冷却
- 📈 **独立成长** —— 亲密度、喂食统计、相处时长、存活时长，每个物种各自记账
- 🪶 **轻量无感** —— 透明小窗常驻桌面，不抢焦点、不进任务栏

## 安装

到 [**Releases**](https://github.com/lulu-ls/flypet/releases) 下载对应平台安装包：

| 平台 | 文件 |
|------|------|
| macOS（Apple Silicon / Intel 通用） | `FlyPet.app` 或 `.dmg` |
| Windows | `FlyPet_x.x.x_x64-setup.exe` |

> macOS 首次打开若提示无法验证开发者：系统设置 → 隐私与安全性 → 仍要打开。

## 从源码构建

**环境要求**：[Rust](https://rustup.rs)；macOS 需 Xcode Command Line Tools（`xcode-select --install`），Windows 需 MSVC 工具链与 WebView2。

```bash
# 开发运行
cd src-tauri
cargo run

# 打包安装包
cargo tauri build
```

产物位于 `src-tauri/target/release/bundle/`。

也可以直接推送 `v*` 格式的 tag，GitHub Actions 会自动构建 macOS + Windows 双平台安装包并生成 Release 草稿。

## 文档

想深入了解设计细节？看这里：

- [设计文档](doc/design.md) —— 架构、状态机、坐标陷阱
- [玩法设计](doc/gameplay.md) —— 进化 / 喂食 / 亲密度
- [开发路线](doc/roadmap.md) —— 阶段规划
- [疑难排查](doc/troubleshooting.md) —— 踩坑记录

## 联系方式

QQ：11111111

---

<div align="center">

如果 FlyPet 给你的桌面添了一点生气，点个 ⭐ 吧

</div>
