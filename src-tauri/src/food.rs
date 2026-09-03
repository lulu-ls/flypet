// 喂食道具系统：100 个修仙网文风格道具，7 品级。
// 品级越高越稀有、亲密度加成越多；喂食时按权重随机抽取一个。
use serde::{Deserialize, Serialize};

/// 道具品级（7 档），越稀有加成越高
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Fan,   // 凡品：普通凡人使用的物品，没有特殊力量
    Ling,  // 灵品：蕴含灵气，辅助修炼或简单特殊效果
    Xuan,  // 玄品：比灵品更强，修士较珍贵的装备
    Di,    // 地品：高阶法宝，普通修士很难获得
    Tian,  // 天品：顶级宝物，强大特殊能力
    Xian,  // 仙品：仙人级别宝物，超脱凡间
    Shen,  // 神品：神话级道具，近乎规则级别的力量
}

impl Rarity {
    /// 品级对应的亲密度加成
    pub fn affinity_gain(self) -> i32 {
        match self {
            Rarity::Fan => 2,
            Rarity::Ling => 4,
            Rarity::Xuan => 8,
            Rarity::Di => 16,
            Rarity::Tian => 32,
            Rarity::Xian => 64,
            Rarity::Shen => 128,
        }
    }

    /// 品级抽取权重：越稀有越低（约 55/25/12/5/2/0.8/0.2）
    pub fn weight(self) -> u32 {
        match self {
            Rarity::Fan => 550,
            Rarity::Ling => 250,
            Rarity::Xuan => 120,
            Rarity::Di => 50,
            Rarity::Tian => 20,
            Rarity::Xian => 8,
            Rarity::Shen => 2,
        }
    }
}

/// 品级权重 → 按权重抽取一个品级（返回品级索引）
fn roll_rarity(r: &mut u64) -> Rarity {
    let weights = [
        (Rarity::Fan, Rarity::Fan.weight()),
        (Rarity::Ling, Rarity::Ling.weight()),
        (Rarity::Xuan, Rarity::Xuan.weight()),
        (Rarity::Di, Rarity::Di.weight()),
        (Rarity::Tian, Rarity::Tian.weight()),
        (Rarity::Xian, Rarity::Xian.weight()),
        (Rarity::Shen, Rarity::Shen.weight()),
    ];
    let total: u32 = weights.iter().map(|(_, w)| w).sum();
    // 简单 xorshift 随机数（与 insect.rs 的 Rng 独立，避免耦合）
    *r ^= *r << 13;
    *r ^= *r >> 7;
    *r ^= *r << 17;
    let mut t = ((*r >> 11) as u32) % total;
    for (rarity, w) in weights {
        if t < w {
            return rarity;
        }
        t -= w;
    }
    Rarity::Fan
}

/// 100 个道具：名称 + 品级（按品级分组，组内等概率抽取）
const ITEM_POOL: &[(&str, Rarity)] = &[
    // ---- 凡品 30 ----
    ("露水珠", Rarity::Fan), ("糖霜晶", Rarity::Fan), ("野果干", Rarity::Fan),
    ("灵米粒", Rarity::Fan), ("菜叶卷", Rarity::Fan), ("碎灵草", Rarity::Fan),
    ("浆果", Rarity::Fan), ("蜂蜜块", Rarity::Fan), ("虫蜕壳", Rarity::Fan),
    ("花粉团", Rarity::Fan), ("苔藓丁", Rarity::Fan), ("树皮屑", Rarity::Fan),
    ("石子糖", Rarity::Fan), ("枯叶脆", Rarity::Fan), ("麦芽糖", Rarity::Fan),
    ("花蜜滴", Rarity::Fan), ("青草汁", Rarity::Fan), ("雨珠", Rarity::Fan),
    ("露草", Rarity::Fan), ("嫩芽", Rarity::Fan), ("菌丝团", Rarity::Fan),
    ("蛛丝团", Rarity::Fan), ("蚂蚁卵", Rarity::Fan), ("蝉蜕", Rarity::Fan),
    ("泥土丸", Rarity::Fan), ("沙粒糖", Rarity::Fan), ("藤蔓须", Rarity::Fan),
    ("芦根", Rarity::Fan), ("浮萍", Rarity::Fan), ("谷糠", Rarity::Fan),
    // ---- 灵品 25 ----
    ("聚气丹", Rarity::Ling), ("清心草", Rarity::Ling), ("回春叶", Rarity::Ling),
    ("养灵丸", Rarity::Ling), ("凝露丹", Rarity::Ling), ("培元散", Rarity::Ling),
    ("碧灵果", Rarity::Ling), ("紫气藤", Rarity::Ling), ("蕴灵草", Rarity::Ling),
    ("茯苓膏", Rarity::Ling), ("灵泉水", Rarity::Ling), ("赤芝", Rarity::Ling),
    ("灵芝粉", Rarity::Ling), ("月露", Rarity::Ling), ("晨露珠", Rarity::Ling),
    ("翠灵叶", Rarity::Ling), ("火灵果", Rarity::Ling), ("水灵珠", Rarity::Ling),
    ("风灵叶", Rarity::Ling), ("土灵果", Rarity::Ling), ("金线草", Rarity::Ling),
    ("银叶草", Rarity::Ling), ("灵茶", Rarity::Ling), ("百草露", Rarity::Ling),
    ("生息丹", Rarity::Ling),
    // ---- 玄品 18 ----
    ("玄晶沙", Rarity::Xuan), ("赤焰果", Rarity::Xuan), ("冰魄露", Rarity::Xuan),
    ("雷音草", Rarity::Xuan), ("风灵珠", Rarity::Xuan), ("土行丹", Rarity::Xuan),
    ("木灵髓", Rarity::Xuan), ("水韵珠", Rarity::Xuan), ("火灵髓", Rarity::Xuan),
    ("青木丹", Rarity::Xuan), ("玄冰晶", Rarity::Xuan), ("紫雷果", Rarity::Xuan),
    ("金光草", Rarity::Xuan), ("银霜露", Rarity::Xuan), ("玉髓", Rarity::Xuan),
    ("灵犀角粉", Rarity::Xuan), ("聚灵阵符", Rarity::Xuan), ("五行散", Rarity::Xuan),
    // ---- 地品 12 ----
    ("地灵果", Rarity::Di), ("天罡丹", Rarity::Di), ("龙涎草", Rarity::Di),
    ("凤羽花", Rarity::Di), ("玄黄果", Rarity::Di), ("星辰砂", Rarity::Di),
    ("太一丹", Rarity::Di), ("地火莲", Rarity::Di), ("天雷果", Rarity::Di),
    ("九转叶", Rarity::Di), ("紫金丹", Rarity::Di), ("碧水珠", Rarity::Di),
    // ---- 天品 7 ----
    ("天机丹", Rarity::Tian), ("紫府玉髓", Rarity::Tian), ("九霄雷果", Rarity::Tian),
    ("涅槃花", Rarity::Tian), ("五行灵珠", Rarity::Tian), ("太虚神水", Rarity::Tian),
    ("混沌叶", Rarity::Tian),
    // ---- 仙品 3 ----
    ("仙灵果", Rarity::Xian), ("九天玄露", Rarity::Xian), ("太乙仙丹", Rarity::Xian),
    // ---- 神品 5 ----
    ("混沌果", Rarity::Shen), ("鸿蒙紫气", Rarity::Shen), ("天道法则碎片", Rarity::Shen),
    ("宇宙本源", Rarity::Shen), ("轮回神水", Rarity::Shen),
];

/// 抽取一个道具：先按权重抽品级，再在品级内等概率抽具体道具
pub fn roll_item(seed: &mut u64) -> (&'static str, Rarity) {
    let rarity = roll_rarity(seed);
    let pool: Vec<&'static str> = ITEM_POOL
        .iter()
        .filter(|(_, r)| *r == rarity)
        .map(|(name, _)| *name)
        .collect();
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    let idx = ((*seed >> 11) as usize) % pool.len();
    (pool[idx], rarity)
}
