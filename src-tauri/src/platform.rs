// 屏幕与坐标差异（design.md §6.2 / §11 platform.rs）
// 所有坐标 trap 收敛在此处。v1 只保证主屏：取窗口当前所在屏，落点 clamp 到工作区。
use crate::insect::Vec2;
use tauri::WebviewWindow;

/// 工作区（物理像素，全局 top-left origin）。多屏时每台显示器一个 Screen，
/// 组成统一虚拟桌面坐标空间；宠物 pos 落在哪台屏的矩形内，就受哪台屏约束。
#[derive(Clone, Copy, Debug)]
pub struct Screen {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 该屏的缩放系数（窗口跨屏时用所在屏的 scale 计算尺寸）
    pub scale: f32,
}

impl Screen {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    /// 两屏的并集包围盒（跨屏飞行路径的中途点 clamp 在它里面，
    /// 允许途经屏间空隙；单屏场景 a==b 时退化为原屏，行为不变）
    pub fn span(a: &Screen, b: &Screen) -> Screen {
        let x0 = a.x.min(b.x);
        let y0 = a.y.min(b.y);
        let x1 = (a.x + a.w).max(b.x + b.w);
        let y1 = (a.y + a.h).max(b.y + b.h);
        Screen { x: x0, y: y0, w: x1 - x0, h: y1 - y0, scale: b.scale }
    }
}

/// 枚举所有显示器的工作区（多屏支持）。任一屏枚举失败退回当前屏单屏列表。
pub fn all_screens(win: &WebviewWindow) -> Vec<Screen> {
    let mons = win.available_monitors().unwrap_or_default();
    let mut list: Vec<Screen> = Vec::new();
    for m in &mons {
        let wa = m.work_area();
        let mut s = Screen {
            x: wa.position.x as f32,
            y: wa.position.y as f32,
            w: wa.size.width as f32,
            h: wa.size.height as f32,
            scale: m.scale_factor() as f32,
        };
        // 边距：菜单栏 / Dock 由 work_area 排除，这里再留 8×scale 侧向呼吸位
        let side = 8.0 * s.scale;
        s.x += side;
        s.w -= side * 2.0;
        if s.w > 0.0 && s.h > 0.0 {
            list.push(s);
        }
    }
    if list.is_empty() {
        list.push(screen_of(win));
    }
    list
}

/// pos 所在屏；不在任何屏内（屏间空隙 / 拔掉副屏）取中心最近屏兜底
pub fn screen_at<'a>(screens: &'a [Screen], pos: Vec2) -> &'a Screen {
    for s in screens {
        if s.contains(pos) {
            return s;
        }
    }
    // 最近屏：按到屏中心距离
    let mut best = &screens[0];
    let mut best_d = f32::MAX;
    for s in screens {
        let c = s.center();
        let d = (c.x - pos.x).powi(2) + (c.y - pos.y).powi(2);
        if d < best_d {
            best_d = d;
            best = s;
        }
    }
    best
}

/// 当前屏工作区：优先读系统 work_area（精确排除菜单栏 / Dock），失败退保守估算
pub fn screen_of(win: &WebviewWindow) -> Screen {
    let mon = win.current_monitor().ok().flatten();

    if let Some(m) = &mon {
        let wa = m.work_area();
        let mut s = Screen {
            x: wa.position.x as f32,
            y: wa.position.y as f32,
            w: wa.size.width as f32,
            h: wa.size.height as f32,
            scale: m.scale_factor() as f32,
        };
        let side = 8.0 * m.scale_factor() as f32;
        s.x += side;
        s.w -= side * 2.0;
        return s;
    }

    // fallback：保守估算
    let scale = win.scale_factor().unwrap_or(1.0) as f32;
    let mut s = Screen {
        x: 0.0,
        y: 0.0,
        w: 1440.0,
        h: 900.0,
        scale,
    };
    let top = if cfg!(target_os = "macos") { 25.0 * scale } else { 0.0 };
    let bottom = if cfg!(target_os = "windows") { 48.0 * scale } else { 0.0 };
    let side = 8.0 * scale;
    s.y += top;
    s.h -= top + bottom;
    s.x += side;
    s.w -= side * 2.0;
    s
}

/// 窗口中心 = 虫子世界坐标：返回物理像素的 (半宽, 半高)
pub fn window_half(win: &WebviewWindow, logical_w: f32, logical_h: f32) -> (i32, i32) {
    let scale = win.scale_factor().unwrap_or(1.0) as f32;
    (
        (logical_w * scale * 0.5) as i32,
        (logical_h * scale * 0.5) as i32,
    )
}
