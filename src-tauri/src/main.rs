// FlyPet 一期：Tauri 2 + Rust 状态机 + 96px 透明跟随窗 + 托盘
// 主循环（§6.1）：轮询全局指针 → 状态机 → 移动窗口 → pose/skin 事件
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod food;
mod insect;
mod platform;
mod pointer;
mod profile;

use insect::{Insect, Vec2};
use platform::{screen_of, window_half};
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};

/// 共享状态：主循环写，invoke 读
struct Shared(Mutex<Insect>);
/// 宠物档案容器（每物种独立一套，feed 命令读写当前物种档）
struct SharedProfile(Mutex<profile::Profiles>);
/// 当前物种 id（settings.json 的权威值；独立小锁供喂食/面板按物种路由，
/// 避免与 Shared / SharedProfile 交叉持锁）
struct CurrentSpecies(Mutex<String>);
/// 待播放的喂食动画参数（Rust 侧暂存，动画窗前端 invoke 取走）
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct PendingAnim {
    name: String,
    rarity: food::Rarity,
    seed: u64,
    species: String,
}
struct PendingFeedAnim(Mutex<Option<PendingAnim>>);
/// 托盘「换物种」请求：菜单回调只写请求（不碰 Shared 锁/窗口 API），
/// run_loop 每帧消费并执行 relaunch，避免主线程与主循环线程交叉持锁卡死。
struct SpeciesRequest(Mutex<Option<String>>);
/// 投喂请求：投喂结算后写入投食点（光标位置），run_loop 消费触发昆虫飞过去进食。
/// 独立于 Shared 锁，避免喂食线程与主循环线程交叉持锁。
struct FeedRequest(Mutex<Option<Vec2>>);
/// 暂停标志
static PAUSED: AtomicBool = AtomicBool::new(false);

fn species_ids() -> Vec<&'static str> {
    insect::SPECIES.iter().map(|s| s.id).collect()
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    pose: insect::Pose,
    facing: i8,
    /// 当前运动方向（世界坐标弧度，0=朝右，π/2=朝下）。飞行/滑翔时 = 落点方向，栖息时 = facing 映射
    heading: f32,
    /// 当前高度（物理像素，地面 0）。前端渲染 y 偏移 + 动态相机
    alt: f32,
    /// 当前落点贴边方位（top/bottom/left/right）。贴边时前端切侧视
    landing_edge: Option<insect::Edge>,
    /// 蝴蝶在窗口内的偏移（物理像素）：窗口 clamp 到工作区后，蝴蝶可能偏离窗口中心
    dx: f32,
    dy: f32,
    species: String,
    stage: i8,
    seed: u32,
}

#[tauri::command]
fn state(shared: State<'_, Shared>) -> StateEvent {
    let ins = shared.0.lock().unwrap();
    StateEvent {
        pose: ins.pose(),
        facing: ins.facing,
        heading: ins.heading(),
        alt: ins.alt,
        landing_edge: ins.landing_edge,
        dx: 0.0,
        dy: 0.0,
        species: ins.species_id().to_string(),
        stage: ins.stage,
        seed: ins.seed_ui(),
    }
}

// ---------- settings.json 持久化（§6.5） ----------

fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("settings.json"))
}

fn load_species(app: &AppHandle) -> String {
    // settings 里存了已删除/禁用的物种时回退到蝴蝶
    let active_ids: Vec<&str> = insect::SPECIES
        .iter()
        .filter(|s| !s.disabled)
        .map(|s| s.id)
        .collect();
    settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("species").and_then(|s| s.as_str()).map(String::from))
        .filter(|s| active_ids.contains(&s.as_str()))
        .unwrap_or_else(|| "butterfly".into())
}

fn save_species(app: &AppHandle, species: &str) {
    if let Some(p) = settings_path(app) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, json!({ "species": species }).to_string());
    }
}

fn seed_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

// ---------- 喂食系统（二期）：道具 / 档案 / 冷却 ----------

/// 喂食结果（给前端：气泡 / 信息窗）
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct FeedItem {
    name: String,
    rarity: food::Rarity,
}

/// feed_info：主窗数据面板查询
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct FeedInfo {
    can_feed: bool,
    /// 剩余冷却秒数（<=0 可喂）
    remaining_sec: i64,
    affinity: i32,
    affinity_level: i32,
    fed_count: u32,
    interact_count: u32,
    last_item: Option<FeedItem>,
}

/// 面板画宠物待机动画所需的外观参数
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct PetAppearance {
    species: String,
    seed: u32,
    stage: i8,
}

#[tauri::command]
fn pet_appearance(shared: State<'_, Shared>) -> PetAppearance {
    let ins = shared.0.lock().unwrap();
    PetAppearance {
        species: ins.species_id().to_string(),
        seed: ins.seed_ui(),
        stage: ins.stage,
    }
}

#[tauri::command]
fn feed_info(app: AppHandle, shared: State<'_, SharedProfile>) -> FeedInfo {
    // 按当前物种路由档案（先短锁取物种 id，释放后再锁容器）
    let sid = current_species(&app);
    let p = shared.0.lock().unwrap();
    let Some(p) = p.get(&sid) else {
        // 该物种尚无档案 → 全新默认
        return FeedInfo {
            can_feed: true,
            remaining_sec: 0,
            affinity: 0,
            affinity_level: 1,
            fed_count: 0,
            interact_count: 0,
            last_item: None,
        };
    };
    // 冷却检查：剩余 >0 时不可喂
    let remain = profile::remaining_cooldown(p.last_fed_at);
    FeedInfo {
        can_feed: remain <= 0,
        remaining_sec: remain,
        affinity: p.affinity,
        affinity_level: profile::affinity_level(p.affinity),
        fed_count: p.fed_count,
        interact_count: p.interact_count,
        last_item: p.last_item.as_ref().map(|li| FeedItem {
            name: li.name.clone(),
            rarity: li.rarity,
        }),
    }
}

/// 读当前物种 id（CurrentSpecies 短锁，克隆释放）
fn current_species(app: &AppHandle) -> String {
    app.state::<CurrentSpecies>()
        .0
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "butterfly".to_string())
}

/// 喂食核心逻辑：冷却检查 → 抽道具 → 加分 → 落盘（锁内原子）。
/// 供托盘线程与 invoke command 共用。
/// 只对「当前物种」的档案生效（各物种独立成长）。
fn do_feed(app: &AppHandle) -> FeedInfo {
    let sid = current_species(app);
    let shared = app.state::<SharedProfile>();
    let mut profiles = shared.0.lock().unwrap();
    // 冷却中：拒绝本次投喂（不抽道具、不落盘、不弹动画）。
    // last_item 返回 None 让 do_feed_and_animate 跳过动画窗。
    {
        let p = profiles.for_species(&sid);
        let remain = profile::remaining_cooldown(p.last_fed_at);
        if remain > 0 {
            return FeedInfo {
                can_feed: false,
                remaining_sec: remain,
                affinity: p.affinity,
                affinity_level: profile::affinity_level(p.affinity),
                fed_count: p.fed_count,
                interact_count: p.interact_count,
                last_item: None,
            };
        }
    }
    // 抽道具（用时间种子，独立于宠物种子）
    let mut seed = seed_now();
    let (name, rarity) = food::roll_item(&mut seed);
    // 内层块：只对当前物种档案做累加；块结束 p 的可变借用即释放，
    // 之后才能持不可变引用落盘（避免 save 时 &mut 冲突）
    let info = {
        let p = profiles.for_species(&sid); // 无档则建默认档
        let gained = rarity.affinity_gain();
        p.affinity += gained;
        p.fed_count += 1;
        p.interact_count += 1;
        p.last_fed_at = profile::now();
        p.last_item = Some(profile::LastItem {
            name: name.to_string(),
            rarity,
        });
        let stats = &mut p.feed_stats;
        stats.total += 1;
        match rarity {
            food::Rarity::Fan => stats.fan += 1,
            food::Rarity::Ling => stats.ling += 1,
            food::Rarity::Xuan => stats.xuan += 1,
            food::Rarity::Di => stats.di += 1,
            food::Rarity::Tian => stats.tian += 1,
            food::Rarity::Xian => stats.xian += 1,
            food::Rarity::Shen => stats.shen += 1,
        }
        FeedInfo {
            can_feed: true,
            remaining_sec: profile::FEED_COOLDOWN_SECS,
            affinity: p.affinity,
            affinity_level: profile::affinity_level(p.affinity),
            fed_count: p.fed_count,
            interact_count: p.interact_count,
            last_item: Some(FeedItem {
                name: name.to_string(),
                rarity,
            }),
        }
    };
    profile::save_profiles(app, &profiles);
    info
}

/// 喂食 + 触发动画窗（托盘线程 / feed command 共用）。
/// 结算后弹出喂食动画窗，并广播 profile-updated 让主窗刷新。
/// 同时写入投喂请求（投食点 = 当前光标位置），run_loop 消费后昆虫飞过去进食。
fn do_feed_and_animate(app: &AppHandle) -> FeedInfo {
    let info = do_feed(app);
    // 冷却中被拒绝：不弹动画、不写投食点（否则宠物仍会飞过去互动）
    if !info.can_feed {
        return info;
    }
    if let Some(item) = &info.last_item {
        // 动画窗用宠物自身的基因组种子 + 物种，保证外观与主窗一致
        let (pet_seed, pet_species) = app
            .state::<Shared>()
            .0
            .lock()
            .map(|ins| (ins.seed_ui() as u64, ins.species_id().to_string()))
            .unwrap_or((1, "butterfly".to_string()));
        show_feed_anim(app, &item.name, item.rarity, pet_seed, &pet_species);
    }
    // 投喂点 = 当前光标位置：写投喂请求，主循环消费后昆虫飞过去进食
    if let Some(pos) = pointer::global(app) {
        if let Some(req) = app.try_state::<FeedRequest>() {
            *req.0.lock().unwrap() = Some(pos);
        }
    }
    // 喂食后亲密度可能升级：把当前物种档案的新等级同步进昆虫（驱动亲近行为档位）
    let sid = current_species(app);
    let new_level = {
        let st = app.state::<SharedProfile>();
        let guard = st.0.lock().unwrap();
        guard
            .get(&sid)
            .map(|pr| profile::affinity_level(pr.affinity))
            .unwrap_or(1)
    };
    if let Some(shared) = app.try_state::<Shared>() {
        shared.0.lock().unwrap().set_affinity_level(new_level);
    }
    let _ = app.emit("profile-updated", ());
    info
}

/// feed：invoke command 包装（主窗「投喂」按钮用；托盘走独立线程）
/// 在非主线程执行（Tauri command 线程池），窗口 show 安全。
#[tauri::command]
fn feed(app: AppHandle) -> FeedInfo {
    do_feed_and_animate(&app)
}

/// 定位窗口到当前光标附近（避免叠在屏幕角落）
fn place_near_cursor(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        if let Some(pos) = pointer::global(app) {
            let (cw, ch) = {
                let s = w.outer_size().unwrap_or_default();
                (s.width as f32, s.height as f32)
            };
            let (sx, sy, sw, sh) = {
                match w.current_monitor().ok().flatten() {
                    Some(m) => {
                        let p = m.position();
                        let s = m.size();
                        (p.x as f32, p.y as f32, s.width as f32, s.height as f32)
                    }
                    None => (0.0, 0.0, 1920.0, 1080.0),
                }
            };
            let x = (pos.x - cw * 0.5).clamp(sx, sx + sw - cw);
            let y = (pos.y - ch * 0.5).clamp(sy, sy + sh - ch);
            let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
        }
    }
}

/// 显示喂食动画窗口：参数暂存 Rust 侧 + 发事件，动画窗前端监听事件后 invoke 取参播放。
/// 窗口只加载一次（不反复 navigate），靠事件驱动每次播放。
fn show_feed_anim(app: &AppHandle, name: &str, rarity: food::Rarity, seed: u64, species: &str) {
    if let Some(state) = app.try_state::<PendingFeedAnim>() {
        *state.0.lock().unwrap() = Some(PendingAnim {
            name: name.to_string(),
            rarity,
            seed,
            species: species.to_string(),
        });
    }
    if let Some(w) = app.get_webview_window("feed-anim") {
        place_near_cursor(app, "feed-anim");
        let _ = w.show();
        // 通知前端开始新一轮播放（前端监听 "feed-anim-start"）
        let _ = app.emit("feed-anim-start", ());
    }
}

/// 动画窗前端取走参数（一次性；取走后清空）
#[tauri::command]
fn anim_params(state: State<'_, PendingFeedAnim>) -> Option<PendingAnim> {
    state.0.lock().unwrap().take()
}

/// 动画结束：隐藏动画窗（前端播完调用）
#[tauri::command]
fn finish_anim(app: AppHandle) {
    if let Some(w) = app.get_webview_window("feed-anim") {
        let _ = w.hide();
    }
}

/// 更新托盘「投喂」项文案：冷却中显示剩余分钟，可喂时恢复「投喂」
fn update_feed_label(app: &AppHandle, feed_item: &tauri::menu::MenuItem<tauri::Wry>) {
    let remain = {
        let sid = current_species(app);
        let shared = app.state::<SharedProfile>();
        let profiles = shared.0.lock().unwrap();
        profiles
            .get(&sid)
            .map(|p| profile::remaining_cooldown(p.last_fed_at))
            .unwrap_or(0)
    };
    let text = if remain > 0 {
        format!("投喂（剩 {} 分钟）", remain / 60 + 1)
    } else {
        "投喂".to_string()
    };
    let _ = feed_item.set_text(&text);
}

fn show_panel(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // 窗口被关掉销毁后，从菜单重新建一块面板
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("main.html".into()))
        .title("FlyPet 面板")
        .inner_size(300.0, 380.0)
        .resizable(true)
        .visible(true)
        .build();
}

fn main() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            // 点关闭只隐藏面板，不销毁，托盘「面板」才能再次打开
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let win = app
                .get_webview_window("insect")
                .expect("missing insect window");

            // v1 强制点击穿透（§6.4）
            let _ = win.set_ignore_cursor_events(true);

            let handle = app.handle().clone();
            let cursor = pointer::global(&handle).unwrap_or(Vec2::new(400.0, 400.0));
            let screen = screen_of(&win);
            let species0 = load_species(&handle);
            // 档案容器（每物种一套）；当前物种的档案等级灌入昆虫
            let profiles0 = profile::load_profiles(&handle);
            let level0 = profiles0
                .get(&species0)
                .map(|pr| profile::affinity_level(pr.affinity))
                .unwrap_or(1);
            let mut insect0 = Insect::new(cursor, &screen, seed_now(), &species0);
            insect0.set_affinity_level(level0);
            app.manage(Shared(Mutex::new(insect0)));
            app.manage(SharedProfile(Mutex::new(profiles0)));
            app.manage(CurrentSpecies(Mutex::new(species0.clone())));
            app.manage(PendingFeedAnim(Mutex::new(None)));
            app.manage(SpeciesRequest(Mutex::new(None)));
            app.manage(FeedRequest(Mutex::new(None)));

            build_tray(app.handle(), &species0)?;

            // feed-anim 动画窗点击穿透（纯展示）；main 是正式窗口不穿透
            if let Some(w) = app.get_webview_window("feed-anim") {
                let _ = w.set_ignore_cursor_events(true);
            }

            // 初始状态同步给前端
            let _ = handle.emit(
                "state",
                StateEvent {
                    pose: insect::Pose::Spawn,
                    facing: 1,
                    heading: 0.0,
                    alt: 0.0,
                    landing_edge: None,
                    dx: 0.0,
                    dy: 0.0,
                    species: species0,
                    stage: 3,
                    seed: ((seed_now() >> 32) ^ (seed_now() & 0xFFFFFFFF)) as u32,
                },
            );

            std::thread::spawn(move || run_loop(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            feed_info,
            feed,
            pet_appearance,
            anim_params,
            finish_anim
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---------- 托盘（§6.5）：显示 / 暂停 / 换皮肤 / 退出 ----------

fn build_tray(app: &AppHandle, current_species: &str) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
    let pause_i = CheckMenuItem::with_id(app, "pause", "暂停", true, false, None::<&str>)?;
    let feed_i = MenuItem::with_id(app, "feed", "投喂", true, None::<&str>)?;
    let feed_info_i = MenuItem::with_id(app, "feed_info", "面板", true, None::<&str>)?;

    // 换物种子菜单：遍历注册表，disabled（开发中）项置灰不可选
    let mut items = Vec::new();
    for s in insect::SPECIES.iter() {
        items.push(CheckMenuItem::with_id(
            app,
            s.id,
            s.label,
            !s.disabled,              // enabled
            !s.disabled && current_species == s.id, // checked（禁用项不勾选）
            None::<&str>,
        )?);
    }
    let submenu_refs: Vec<&dyn tauri::menu::IsMenuItem<_>> =
        items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<_>).collect();
    let species_menu = Submenu::with_items(app, "换物种", true, &submenu_refs)?;

    let devtools_i = MenuItem::with_id(app, "devtools", "开发者工具", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_i,
            &pause_i,
            &species_menu,
            &sep,
            &feed_i,
            &feed_info_i,
            &sep2,
            &devtools_i,
            &sep3,
            &quit_i,
        ],
    )?;

    let pause1 = pause_i.clone();
    let items1: Vec<_> = items.iter().cloned().collect();
    let feed1 = feed_i.clone();
    // 启动时初始化「投喂」文案
    update_feed_label(app, &feed_i);

    TrayIconBuilder::with_id("flypet-tray")
        .icon(
            tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))
                .expect("bad tray icon"),
        )
        .tooltip("FlyPet")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(w) = app.get_webview_window("insect") {
                        let _ = w.show();
                    }
                }
                "pause" => {
                    let now = !PAUSED.load(Ordering::Relaxed);
                    PAUSED.store(now, Ordering::Relaxed);
                    let _ = pause1.set_checked(now);
                    if let Some(w) = app.get_webview_window("insect") {
                        let _ = w.eval(&format!("window.__paused = {}", now));
                    }
                }
                "feed" => {
                    // 喂食处理移到独立线程：菜单回调在主线程，直接操作窗口（w.show 等
                    // 同步 AppKit 调用）会与主线程事件循环互锁导致卡死。
                    let app2 = app.clone();
                    let feed2 = feed1.clone();
                    std::thread::spawn(move || {
                        do_feed_and_animate(&app2);
                        update_feed_label(&app2, &feed2);
                    });
                }
                "feed_info" => {
                    // 菜单回调在主线程，窗口 show 放到独立线程，避免与 AppKit 互锁卡死
                    let app2 = app.clone();
                    std::thread::spawn(move || show_panel(&app2));
                }
                "devtools" => {
                    if let Some(w) = app.get_webview_window("insect") {
                        let _ = w.open_devtools();
                    }
                }
                "quit" => app.exit(0),
                _ => {
                    let id = event.id().as_ref();
                    if species_ids().contains(&id) {
                        // 菜单回调不碰 Shared 锁 / 窗口 API：只持久化、更新勾选、
                        // 写换物种请求，由 run_loop 在无锁窗口操作间隙消费执行，
                        // 避免 macOS 主线程与主循环线程持锁互锁卡死。
                        save_species(app, id);
                        // 同步「当前物种」：喂食/面板按它路由到对应档案
                        if let Some(cur) = app.try_state::<CurrentSpecies>() {
                            *cur.0.lock().unwrap() = id.to_string();
                        }
                        for it in &items1 {
                            let _ = it.set_checked(it.id().as_ref() == id);
                        }
                        if let Some(req) = app.try_state::<SpeciesRequest>() {
                            *req.0.lock().unwrap() = Some(id.to_string());
                        }
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

// ---------- 主循环（§6.1） ----------

fn run_loop(app: AppHandle) {
    let win = app.get_webview_window("insect").expect("missing window");
    let (half_w, half_h) = window_half(&win, 140.0, 160.0);

    let mut shown = false;
    let mut last = Instant::now();
    let mut last_log = Instant::now();
    let mut last_heading_emit = Instant::now();
    // 光标速度平滑估计（用于惊扰程度）
    let mut cursor_pos = Vec2::new(0.0, 0.0);
    let mut cursor_speed = 0.0f32;
    // 光标静止计时（秒）：帧间位移很小则累加，动了清零（亲密度亲近行为用）
    let mut cursor_still = 0.0f32;
    // 窗口内偏移平滑（低通）：窗口 clamp 到屏边时 off 突变，平滑后前端不会瞬移
    let mut smooth_off_x = 0.0f32;
    let mut smooth_off_y = 0.0f32;

    loop {
        // 暂停：冻在原地，重置计时防止恢复时 dt 跳变
        if PAUSED.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(120));
            last = Instant::now();
            continue;
        }

        let dt = last.elapsed().as_secs_f32().min(0.05);
        last = Instant::now();

        // 先取光标/屏幕/缩放（AppKit 调用，必须在无 Rust 锁状态下进行，否则与主线程
        // 菜单回调等锁互锁 → macOS 转圈卡死）
        let screen = screen_of(&win);
        let scale_factor = win.scale_factor().unwrap_or(1.0);
        let cursor = pointer::global(&app).unwrap_or_else(|| {
            app.state::<Shared>().0.lock().unwrap().pos
        });

        // ---- 锁内：纯状态机计算（不碰任何窗口/AppKit API）----
        let frame = {
            let shared = app.state::<Shared>();
            let mut ins = shared.0.lock().unwrap();

            // 消费托盘「换物种」请求：relaunch 立即按新物种参数起飞
            let mut species_changed = false;
            let mut switched_to: Option<String> = None;
            if let Some(req) = app.try_state::<SpeciesRequest>() {
                let mut slot = req.0.lock().unwrap();
                if let Some(id) = slot.take() {
                    if ins.set_species(&id) {
                        ins.relaunch(cursor, &screen);
                        species_changed = true;
                        switched_to = Some(id);
                    } else if let Some(cur) = app.try_state::<CurrentSpecies>() {
                        // 切换被拒（理论上 UI 已置灰，防御性回滚）
                        *cur.0.lock().unwrap() = ins.species_id().to_string();
                    }
                }
            }

            // 消费投喂请求：昆虫飞向投食点进食（左右往复）
            let mut feed_triggered = false;
            if let Some(req) = app.try_state::<FeedRequest>() {
                let mut slot = req.0.lock().unwrap();
                if let Some(pos) = slot.take() {
                    ins.start_feed(pos, &screen);
                    feed_triggered = true;
                }
            }

            // 光标速度 → 惊扰程度（0~1）：靠近越快越惊慌
            if cursor_pos.x != 0.0 || cursor_pos.y != 0.0 {
                let inst = cursor.dist(cursor_pos) / dt.max(0.001);
                cursor_speed = cursor_speed * 0.6 + inst * 0.4; // 平滑
                // 静止计时：速度低于 ~10px/s 视为停住（含鼠标微抖容差）
                if inst < 10.0 {
                    cursor_still += dt;
                } else {
                    cursor_still = 0.0;
                }
            }
            cursor_pos = cursor;
            let scare = (cursor_speed / 600.0).clamp(0.0, 1.0);
            let still_secs = cursor_still;
            // 光标是否移动中（>12px/s 视为在动）：苍蝇「赶走」检测用
            let cursor_active = cursor_speed > 12.0;

            let pose_changed = ins.update(cursor, &screen, dt, scare, still_secs, cursor_active)
                .is_some()
                || species_changed
                || feed_triggered;

            // 窗口目标位置（clamp 到工作区），纯计算
            let win_w = 140.0 * scale_factor as f32;
            let win_h = 160.0 * scale_factor as f32;
            let wx = (ins.pos.x as f32 - half_w as f32).clamp(screen.x, screen.x + screen.w - win_w);
            let wy = (ins.pos.y as f32 - half_h as f32).clamp(screen.y, screen.y + screen.h - win_h);

            // 窗口内偏移平滑（低通）：窗口 clamp 到屏边时 off 突变，平滑后前端不会瞬移
            let k = (dt * 8.0).min(1.0);
            smooth_off_x += ((ins.pos.x - (wx + win_w * 0.5)) - smooth_off_x) * k;
            smooth_off_y += ((ins.pos.y - (wy + win_h * 0.5)) - smooth_off_y) * k;
            let off_x = smooth_off_x;
            let off_y = smooth_off_y;

            // 是否要发 state 事件（进食左右往复时 heading 持续变化，也需 30Hz 推送）
            let flying = matches!(
                ins.pose(),
                insect::Pose::Flee
                    | insect::Pose::Glide
                    | insect::Pose::Spawn
                    | insect::Pose::Feed
                    | insect::Pose::Approach
            );
            let heading_changed = flying && last_heading_emit.elapsed().as_millis() >= 33; // ~30Hz
            let emit_state = pose_changed || heading_changed;
            if pose_changed {
                println!("EVENT pose={:?} heading={:.2} facing={}", ins.pose(), ins.heading(), ins.facing);
            }
            let h = ins.heading();
            let state = StateEvent {
                pose: ins.pose(),
                facing: ins.facing,
                heading: h,
                alt: ins.alt,
                landing_edge: ins.landing_edge,
                dx: off_x,
                dy: off_y,
                species: ins.species_id().to_string(),
                stage: ins.stage,
                seed: ins.seed_ui(),
            };
            // 锁在此块结束时释放（ins/shared drop）
            (
                emit_state,
                state,
                wx,
                wy,
                scale_factor,
                ins.is_perched(),
                ins.pos,
                ins.flee_radius(),
                cursor,
                switched_to,
            )
        };

        // ---- 锁外：AppKit / 事件 API（主线程或子线程均可安全调用）----
        let (emit_state, state, wx, wy, scale_factor, perched, pos, flee_radius, cursor, switched_to) =
            frame;
        // 换物种后：把新物种档案的亲密度等级同步进昆虫（亲近行为档位立即切换）
        if let Some(sid) = &switched_to {
            let lv = {
                let st = app.state::<SharedProfile>();
                let guard = st.0.lock().unwrap();
                guard
                    .get(sid)
                    .map(|pr| profile::affinity_level(pr.affinity))
                    .unwrap_or(1)
            };
            if let Some(shared) = app.try_state::<Shared>() {
                shared.0.lock().unwrap().set_affinity_level(lv);
            }
        }
        let _ = win.set_position(PhysicalPosition::new(wx as i32, wy as i32));
        if emit_state {
            last_heading_emit = Instant::now();
            let _ = app.emit("state", &state);
        }
        // 第一帧定位完成后再显示，避免闪现在初始坐标（§10）
        if !shown {
            let _ = win.show();
            shown = true;
        }
        // 空闲降频：栖息且光标远时降到 ~25Hz
        let busy = !(perched && pos.dist(cursor) > flee_radius * 2.0);
        // 调试：每秒输出位置，便于截图定位
        if last_log.elapsed().as_millis() > 1000 {
            last_log = Instant::now();
            println!(
                "POS {:.0} {:.0} scale {:.1} pose {:?}",
                pos.x,
                pos.y,
                scale_factor,
                state.pose
            );
        }

        std::thread::sleep(Duration::from_millis(if busy { 16 } else { 40 }));
    }
}
