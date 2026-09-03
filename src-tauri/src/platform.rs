// 屏幕与坐标差异（design.md §6.2 / §11 platform.rs）
// 所有坐标 trap 收敛在此处。v1 只保证主屏：取窗口当前所在屏，落点 clamp 到工作区。
use crate::insect::Vec2;
use tauri::WebviewWindow;

/// 工作区（物理像素，全局 top-left origin）
#[derive(Clone, Copy, Debug)]
pub struct Screen {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Screen {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }
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
