# 模型资源规范

## 目录职责

| 目录 | 放什么 | 是否打进 .app |
|------|--------|--------------|
| `assets/models/` | 原始工作文件：`.blend` `.fbx` 高模、原始尺寸贴图 | ❌ 不打包 |
| `ui/models/` | 运行时的 `.glb`（唯一真源） | ✅ 自动打包 |
| `ui-lab/models` | → `../ui/models` 的软链，供实验室页面访问同一份文件 | — |

**一处存放、两处生效**：模型只放 `ui/models/fly.glb`，
实验室通过软链读取，正式应用由 Tauri 从 `ui/` 整目录嵌入。不要拷贝副本。

## 命名约定（决定能否自动绑定）

模型内节点名包含以下关键词即可自动识别（大小写不敏感）：

| 部位 | 推荐命名 | 也接受 |
|------|---------|--------|
| 身体 | `body` | `thorax` `torso` |
| 腹部 | `abdomen` | `belly` `abd` |
| 头 | `head` | — |
| 左翅 | `wing_L` | `wing_left` `wing.001` |
| 右翅 | `wing_R` | `wing_right` `wing.002` |
| 腿 ×6 | `leg_FL` … | 名字含 `leg` 即可 |

找不到翅膀也能运行，只是没有振翅。翅膀轴心会自动校正到翅根，
无需在建模软件里手动摆轴心。

## 格式：优先 glb，不要直接给 fbx

- **`.glb`（推荐）**：单文件，贴图内嵌，three.js 直接加载
- **`.fbx`**：浏览器端需要 FBXLoader（体积大、加载慢），建议先转换：

```bash
# 方式一：Blender 命令行
/Applications/Blender.app/Contents/MacOS/Blender -b -P - <<'EOF'
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.import_scene.fbx(filepath="assets/models/fly/fly.fbx")
bpy.ops.export_scene.gltf(filepath="ui/models/fly.glb", export_format='GLB')
EOF

# 方式二：fbx2gltf（需先 brew install fbx2gltf 或 npm i -g fbx2gltf）
fbx2gltf -i assets/models/fly/fly.fbx -o ui/models/fly.glb
```

## 动画：不需要在文件里烘焙

姿态、振翅、搓手、爬行全部由 `ui-lab/model.html` 的代码驱动。
**请勿导出烘焙动画**，除非是想要播放的固定演出（如破壳）。

若文件里已含动画片段，加载后页面会列出片段名，
届时告诉我哪些要保留播放、哪些要忽略。

## 体积预算

桌面宠物常驻后台，建议 **3k–10k 三角面、贴图 ≤1024**、单文件 ≤2 MB。
