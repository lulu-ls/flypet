// 全局指针采样（design.md §6.1 / §4.2）
// 使用 Tauri 内建 cursor_position()：macOS 走 NSEvent，Windows 走 GetCursorPos，
// 只查询坐标、不截获事件，无需辅助功能权限；返回值与窗口 set_position 同一坐标系。
use crate::insect::Vec2;
use tauri::AppHandle;

pub fn global(app: &AppHandle) -> Option<Vec2> {
    let p = app.cursor_position().ok()?;
    Some(Vec2::new(p.x as f32, p.y as f32))
}
