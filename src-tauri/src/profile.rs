// 宠物档案 data.json：亲密度 / 喂食统计 / 冷却时间。
// 设计见 doc/gameplay.md §2：事件驱动写入，绝不每帧写；崩溃安全（tmp + rename）。
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

/// 喂食冷却时长：10 分钟
pub const FEED_COOLDOWN_SECS: i64 = 600;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FeedStats {
    pub total: u32,
    pub fan: u32,
    pub ling: u32,
    pub xuan: u32,
    pub di: u32,
    pub tian: u32,
    pub xian: u32,
    pub shen: u32,
}

/// 上次喂食的道具（供信息窗/气泡展示；跨重启保留）
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LastItem {
    pub name: String,
    pub rarity: crate::food::Rarity,
}

/// 档案容器：每个物种（butterfly/dragonfly/…）一套独立成长数据。
/// 存储为 data.json 的 `{ "species": { "<id>": {…}, … } }`。
/// 旧版本（无分区、data.json 直接是单个 Profile）读到时按「丢弃旧数据」处理，
/// 各物种从 Default 重新开始。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Profiles {
    #[serde(default)]
    pub species: std::collections::BTreeMap<String, Profile>,
}

impl Profiles {
    /// 取某物种档案；不存在则就地新建一个默认档（不落盘，等下次保存）
    pub fn for_species(&mut self, id: &str) -> &mut Profile {
        self.species.entry(id.to_string()).or_default()
    }

    /// 取某物种档案（只读）；无则 None
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.species.get(id)
    }
}

/// 宠物档案（与 settings.json 分离：档案是宠物状态，设置是用户偏好）
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub skin: String,
    pub stage: i8,
    pub affinity: i32,
    pub fed_count: u32,
    /// 互动次数（独立统计：喂食 +1，后续拖动/说话等 +1）
    #[serde(default)]
    pub interact_count: u32,
    /// 上次成功喂食的 unix 秒；0 = 从未喂过
    pub last_fed_at: i64,
    pub hatched_at: i64,
    pub spawned_day: String,
    pub feed_stats: FeedStats,
    /// 上次喂食的道具名 + 品级（信息窗展示）
    #[serde(default)]
    pub last_item: Option<LastItem>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            skin: "butterfly".into(),
            stage: 3,
            affinity: 0,
            fed_count: 0,
            interact_count: 0,
            last_fed_at: 0,
            hatched_at: now(),
            spawned_day: today(),
            feed_stats: FeedStats::default(),
            last_item: None,
        }
    }
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn today() -> String {
    let secs = now() as u64;
    // 简单换算为本地日期字符串 YYYY-MM-DD（不考虑时区偏移精度，够用）
    let days = secs / 86400;
    // 从 1970-01-01 起的天数 → 公历（近似，不处理闰秒）
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let leap = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if d < leap {
            break;
        }
        d -= leap;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    while d >= mdays[m] {
        d -= mdays[m];
        m += 1;
    }
    format!("{}-{:02}-{:02}", y, m + 1, d + 1)
}

fn profile_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("data.json"))
}

/// 启动时载入档案容器；缺失/损坏回退空容器。
/// 旧格式（data.json 直接是单个 Profile，无 "species" 分区键）因
/// `species` 字段带 #[serde(default)] 会解析成空容器 —— 等效「丢弃旧数据」，
/// 首次保存时以新结构覆盖。
pub fn load_profiles(app: &tauri::AppHandle) -> Profiles {
    profile_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Profiles>(&s).ok())
        .unwrap_or_default()
}

/// 崩溃安全写入：写 data.json.tmp 后 rename
pub fn save_profiles(app: &tauri::AppHandle, p: &Profiles) {
    if let Some(path) = profile_path(app) {
        let tmp = path.with_extension("json.tmp");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string(p) {
            let _ = std::fs::write(&tmp, s);
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// 亲密度等级表（累计值，约 1.6 倍曲线，doc/gameplay.md §4.1）
const AFFINITY_LEVELS: [i32; 10] = [0, 50, 150, 350, 700, 1200, 2000, 3200, 5000, 7500];

pub fn affinity_level(affinity: i32) -> i32 {
    let mut lv = 1;
    for (i, &th) in AFFINITY_LEVELS.iter().enumerate() {
        if affinity >= th {
            lv = (i + 1) as i32;
        }
    }
    lv
}

/// 剩余冷却秒数（<=0 表示可喂）
pub fn remaining_cooldown(last_fed_at: i64) -> i64 {
    (last_fed_at + FEED_COOLDOWN_SECS - now()).max(0)
}
