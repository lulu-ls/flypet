// 飞虫状态机：Spawn / Perch / Flee（design.md §5）
// 世界坐标统一使用「桌面全局物理像素，top-left origin」。
//
// 扩展点：加新物种 = 往 SPECIES 注册表加一条 SpeciesDef（参数 + Movement 策略），
// 状态机 / 主循环 / 托盘全部数据驱动，零改动。
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pose {
    Spawn,
    Perch,
    Flee,
    Glide,
    Feed,
    /// 亲密度解锁的亲近行为：飞到光标旁停靠
    Approach,
}

/// 屏幕边缘（贴边停靠用）：上下左右。蝴蝶停上去身体顺边。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    /// 沿边取一点（t∈[0,1]），距边缘带内侧
    fn sample(self, screen: &Screen, t: f32) -> (f32, f32) {
        let band = (screen.w.min(screen.h) * 0.10).max(30.0); // 边缘带宽
        match self {
            Edge::Top => (screen.x + t * screen.w, screen.y + band * 0.5),
            Edge::Bottom => (screen.x + t * screen.w, screen.y + screen.h - band * 0.5),
            Edge::Left => (screen.x + band * 0.5, screen.y + t * screen.h),
            Edge::Right => (screen.x + screen.w - band * 0.5, screen.y + t * screen.h),
        }
    }
}

/// 随机选一条边缘（等概率）
fn pick_edge(rng: &mut Rng) -> Edge {
    let edges = [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right];
    edges[(rng.next_f32() * 4.0) as usize % 4]
}

/// 落点：位置 + 是否贴边（贴哪条边）
#[derive(Clone, Copy, Debug)]
pub struct Landing {
    pub pos: Vec2,
    pub edge: Option<Edge>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
    pub fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn dist(self, o: Self) -> f32 {
        self.sub(o).len()
    }
    pub fn lerp(self, o: Self, t: f32) -> Self {
        Self::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

/// 零依赖伪随机（xorshift64），所有策略共享，避免各物种自造 RNG
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f32) / ((1u64 << 53) as f32)
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + self.next_f32() * (b - a)
    }
}

/// 行为参数（design.md §5.2，单位：物理像素 / 秒）
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub flee_radius: f32,
    pub safe_radius: f32,
    pub peak_speed: f32,
    pub flee_boost: f32,
    pub cooldown: f32,
    pub spawn_offset: f32,
    pub idle_base: f32,
}

/// 飞行路径风格：每物种定义「怎么飞」（之字幅度 / 巡航高度 / 螺旋起飞 / 变速），
/// 由 Movement 提供，路径生成器（build_flight_path）统一消费。
#[derive(Clone, Copy)]
pub struct PathStyle {
    /// 巡航高度（物理像素，地面为 0）
    pub cruise_alt: f32,
    /// 起飞螺旋转向幅度（弧度）：起飞冲点后的大角度转向，0 = 直飞
    pub takeoff_spiral: f32,
    /// 之字形巡航控制点数量
    pub zigzag: i32,
    /// 每次转向偏离「朝落点方向」的角度（弧度），越大越飘忽
    pub turn_bias: f32,
    /// 巡航高度波动（相对 cruise_alt 的倍率）
    pub alt_wobble: f32,
    /// 飞行中段速度随机（倍率）：忽快忽慢的变速感
    pub speed_wobble: f32,
}

impl Default for PathStyle {
    fn default() -> Self {
        Self {
            cruise_alt: 30.0,
            takeoff_spiral: 0.7,
            zigzag: 2,
            turn_bias: 0.5,
            alt_wobble: 0.12,
            speed_wobble: 0.18,
        }
    }
}

/// 亲近行为配置：亲密度达到一定等级后，鼠标静止时虫子主动飞近停靠。
/// 由 Movement 提供（物种个性），框架按等级查表决定是否/如何触发。
#[derive(Clone, Copy, Debug)]
pub struct ApproachRule {
    /// 光标需静止的秒数（等级越高可越短：越亲越耐不住等待）
    pub still_secs: f32,
    /// 满足静止条件后的触发概率 0~1（等级越高越主动）
    pub chance: f32,
    /// 停靠点离光标的距离（物理像素，不遮指针）
    pub dist: f32,
}

/// 运动策略：每物种实现自己的「怎么飞」。
pub trait Movement: Send + Sync {
    /// 飞行路径风格（之字 / 高度 / 螺旋 / 变速），默认保守值
    fn path_style(&self) -> PathStyle {
        PathStyle::default()
    }

    /// 亲近行为配置：按当前亲密度等级返回规则；None = 该等级下无亲近行为。
    /// 默认无亲近行为；蜻蜓等亲人的物种 override。
    fn approach_rule(&self, _level: i32) -> Option<ApproachRule> {
        None
    }

    /// 自发飞行落点偏光标的概率 0~1：苍蝇 0.5（一半概率落在光标附近环带），
    /// 蝴蝶/蜻蜓 0（默认离光标远远的）。只影响 pick_landing 的随机落点分支。
    fn cursor_landing_bias(&self) -> f32 {
        0.0
    }

    /// 是否在飞行末段进入「滑翔」姿态（蝴蝶为 true；苍蝇直落）。
    /// 滑翔 = 末段从当前速度匀减速落到落点（Glide 状态），视觉上是
    /// 飞行动画 → 减速接近 → 停止。前端对无滑翔动画的物种（蜻蜓）把
    /// glide 姿态映射回飞行动画（高频振翅减速）。
    fn has_glide(&self) -> bool {
        false
    }

    /// 落点策略（§5.3）：默认远离光标 ±30°、出界旋转重试、兜底工作区随机。
    /// `scare` 0~1：惊扰程度（鼠标靠近速度越大越惊慌），
    /// 距离随 scare 放大（越惊慌飞越远）、偏角随机越大（路径越不可预测）。
    /// 增加贴边偏好：部分落点选在屏幕边缘带内（蝴蝶爱停边缘），身体顺边。
    fn pick_landing(
        &self,
        rng: &mut Rng,
        pos: Vec2,
        cursor: Vec2,
        screen: &Screen,
        scared: bool,
        scare: f32,
        p: &Params,
    ) -> Landing {
        let away = if scared && pos.dist(cursor) > 0.001 {
            pos.sub(cursor)
        } else {
            let a = rng.range(0.0, std::f32::consts::TAU);
            Vec2::new(a.cos(), a.sin())
        };

        let short = screen.w.min(screen.h);
        // 偏光标落点（苍蝇天性贴人）：自发飞行(scared=false)时按物种概率
        // 选光标附近环带（距光标 90~230px，不遮指针）。优先于贴边偏好。
        // 惊恐逃逸(scared=true)不适用——逃跑就该离光标远。
        if !scared && self.cursor_landing_bias() > 0.0 && rng.next_f32() < self.cursor_landing_bias()
        {
            for _ in 0..24 {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let d = rng.range(90.0, 230.0);
                let c = cursor.add(Vec2::new(a.cos(), a.sin()).mul(d));
                // 屏内 + 别落在当前位置(原地打转)
                if screen.contains(c) && c.dist(pos) > 60.0 {
                    return Landing { pos: c, edge: None };
                }
            }
        }
        // 贴边偏好权重：约 45% 落点停在屏幕边缘带（宽 = 短边 10%，夹在屏内）。
        // along ∈ [0.12, 0.88]：沿边随机位置，两端各留 12% 避免贴角
        if rng.next_f32() < 0.45 {
            let edge = pick_edge(rng);
            let along = rng.range(0.12, 0.88);
            let (x, y) = edge.sample(screen, along);
            let c = Vec2::new(x, y);
            // 落点尽量离光标远，太近则放弃贴边
            if c.dist(cursor) >= p.safe_radius {
                return Landing { pos: c, edge: Some(edge) };
            }
        }

        // 距离：safe_radius ~ 短边 55%，随惊扰程度放大 + 个体随机。
        // 上下限都放宽：最短飞行也够看（≥0.9×safe_radius），最长可飞数秒（之字绕行更久）
        let rand_k = rng.range(0.9, 1.5);
        let max_d = (short * 0.55).max(p.safe_radius * (1.0 + scare * 1.6));
        let dist = rng.range(p.safe_radius, max_d) * rand_k;

        // 偏角：±30° 基础，惊扰越大偏角越大（之字形不可预测）
        let base_spread = 0.52 + scare * 0.9;
        let mut ang = away.y.atan2(away.x) + rng.range(-base_spread, base_spread);
        for _ in 0..20 {
            let c = pos.add(Vec2::new(ang.cos(), ang.sin()).mul(dist));
            if screen.contains(c) && c.dist(cursor) >= p.safe_radius {
                return Landing { pos: c, edge: None };
            }
            ang += 0.39; // 约 22.5° 旋转再试
        }

        // 兜底：工作区随机点，尽量离光标远
        for _ in 0..32 {
            let c = Vec2::new(
                screen.x + rng.next_f32() * screen.w,
                screen.y + rng.next_f32() * screen.h,
            );
            if c.dist(cursor) >= p.safe_radius {
                return Landing { pos: c, edge: None };
            }
        }
        Landing {
            pos: Vec2::new(screen.x + screen.w * 0.5, screen.y + screen.h * 0.5),
            edge: None,
        }
    }

    /// 停留时长（秒）：默认在 idle_base 上下浮动
    fn idle_limit(&self, rng: &mut Rng, base: f32) -> f32 {
        rng.range(base * 0.8, base * 2.0)
    }

    /// fidget（栖息小挪动）频率倍率
    fn fidget_chance(&self) -> f32 {
        1.0
    }

    /// 进食滑翔距离范围（物理像素）。
    /// 蜻蜓可悬停 → 小幅；蝴蝶靠滑翔不能悬停 → 大范围。
    fn feed_glide_dist(&self) -> (f32, f32) {
        (45.0, 85.0)
    }

    /// 进食基准高度（物理像素）。
    fn feed_alt(&self) -> f32 {
        22.0
    }

    /// 进食高度波动幅度（物理像素，正弦起伏）。蝴蝶滑翔起伏明显，蜻蜓近悬停。
    fn feed_alt_wobble(&self) -> f32 {
        4.0
    }

    /// 进食是否「盘旋」（绕投食点连续转圈，朝向沿切线平滑变化，无猛掉头）。
    /// 蝴蝶 true（三维盘旋）；蜻蜓 false（左右悬停小幅移动）。
    fn feed_orbit(&self) -> bool {
        false
    }

    /// 盘旋半径范围（物理像素）。仅 feed_orbit()=true 时使用。
    fn feed_orbit_radius(&self) -> (f32, f32) {
        (60.0, 110.0)
    }
}

/// 生成飞行路径（蝴蝶/蜜蜂/甲虫共用）：从 from 到 to 的之字形巡航 + 高度 + 变速。
///
/// 设计（真实蝴蝶运动学）：
/// - **螺旋起飞**：先沿远离光标的冲点猛冲，再以 `takeoff_spiral` 大角度转向，形成逃窜的旋转感
/// - **之字形巡航**：`zigzag` 个控制点，每点相对「朝落点方向」随机偏转 `turn_bias`，忽左忽右
/// - **高度**：起飞爬升到 `cruise_alt`，中段起伏（`alt_wobble`），降落回地面
/// - **变速**：每段距离除以其目标速度（`peak * speed_wobble`），快慢交替
/// - **平滑**：chaikin 迭代平滑曲线，末段（滑翔）改用 `flight_alt` 贴地掠入
///
/// 返回的路径是「航点 + 弧长时间表」：`sample(t)` 用弧长参数化保证匀速感，
/// 且返回每段速度供前端做爬升/俯冲俯仰。
fn build_flight_path(
    from: Vec2,
    to: Vec2,
    cursor: Vec2,
    scared: bool,
    boost: f32,
    peak_speed: f32,
    screen: &Screen,
    path: &PathStyle,
    rng: &mut Rng,
) -> FlightPath {
    let away = if scared && from.dist(cursor) > 1.0 {
        from.sub(cursor)
    } else {
        let a = rng.range(0.0, std::f32::consts::TAU);
        Vec2::new(a.cos(), a.sin())
    };
    let away_ang = away.y.atan2(away.x);

    // 路径点约束到工作区内部（留边距），避免飞出屏幕导致窗口消失
    let margin = 24.0;
    let clamp = |p: Vec2| Vec2::new(
        p.x.clamp(screen.x + margin, screen.x + screen.w - margin),
        p.y.clamp(screen.y + margin, screen.y + screen.h - margin),
    );

    // 起飞冲点：沿远离光标方向 15~25% 距离处（螺旋起飞的支点）
    let dist_to = from.dist(to).max(1.0);
    let boost_dist = dist_to * rng.range(0.15, 0.25);
    let boost_dir = Vec2::new(away_ang.cos(), away_ang.sin());
    let launch = clamp(from.add(boost_dir.mul(boost_dist)));
    // 起飞加速 ramp：from→launch 之间插两个点，把「静止 → 满 boost」拆成
    // 三级递进速度（0.3 → 0.65 → 1.0）×boost，近似线性加速。否则从栖息
    // 瞬间跳到 boost×peak（蜻蜓受惊可 >800px/s）会形成肉眼可见的
    // 「突然加速」。
    let ramp_a = clamp(from.add(boost_dir.mul(boost_dist * 0.25)));
    let ramp_b = clamp(from.add(boost_dir.mul(boost_dist * 0.6)));

    // 螺旋中间点：从起飞冲点再沿螺旋角方向拐，形成受惊逃窜的旋转感
    let spiral_ang = away_ang + path.takeoff_spiral * rng.range(0.7, 1.3);
    let spiral_pt = clamp(launch.add(Vec2::new(spiral_ang.cos(), spiral_ang.sin()).mul(dist_to * rng.range(0.10, 0.18))));

    // 控制点：起点 → ramp_a → ramp_b → 起飞冲点 → 螺旋拐点 → 之字 → 落点
    let mut pts = vec![from, ramp_a, ramp_b, launch, spiral_pt];
    let base_ang = to.sub(spiral_pt).y.atan2(to.sub(spiral_pt).x);
    for i in 0..path.zigzag {
        // 转向角：交替左右偏，越大越飘忽（蝴蝶明显，甲虫几乎直线）
        let turn = if i % 2 == 0 { 1.0 } else { -1.0 } * rng.range(0.3, 1.0) * path.turn_bias;
        let ang = base_ang + turn;
        // 段长：落点方向分量 + 横向分量，形成之字
        let seg = dist_to * rng.range(0.18, 0.32);
        let lateral = dist_to * rng.range(0.08, 0.16);
        let p = clamp(spiral_pt.add(Vec2::new(ang.cos(), ang.sin()).mul(seg))
            .add(Vec2::new(ang.cos() + std::f32::consts::FRAC_PI_2, ang.sin() + std::f32::consts::FRAC_PI_2).mul(lateral)));
        pts.push(p);
    }
    pts.push(clamp(to));

    // chaikin 平滑 2 次（保留首尾端点）
    let mut sm = pts;
    for _ in 0..2 {
        let mut nxt = vec![sm[0]];
        for w in sm.windows(2) {
            let q = w[0].lerp(w[1], 0.25);
            let r = w[0].lerp(w[1], 0.75);
            nxt.push(q);
            nxt.push(r);
        }
        nxt.push(*sm.last().unwrap());
        sm = nxt;
    }

    // 弧长：每段长度 / 速度（基准 peak_speed × 段内变速权重），累积成时间表
    // 段内 ±speed_wobble 形成忽快忽慢；起飞三级 ramp（0.3→0.65→1.0）×boost
    // 近似线性加速，避免「静止 → 满 boost」的瞬间加速；降落段 ×0.8（减速掠入）
    let n = sm.len();
    let mut cum = vec![0.0f32; n];
    let mut speeds = vec![0.0f32; n - 1];
    for i in 0..n - 1 {
        let len = sm[i].dist(sm[i + 1]);
        let wob = 1.0 + rng.range(-path.speed_wobble, path.speed_wobble);
        let speed = if i == 0 {
            boost * 0.30 * wob
        } else if i == 1 {
            boost * 0.65 * wob
        } else if i == 2 {
            boost * wob
        } else if i == n - 2 {
            0.8 * wob // 降落段
        } else {
            wob
        };
        speeds[i] = speed.max(0.35);
        cum[i + 1] = cum[i] + len / (peak_speed * speeds[i]);
    }
    let total = cum.last().copied().unwrap_or(0.0).max(0.001);
    // 高度表：起点 0 → 起飞后爬升 → 中段 cruise ± alt_wobble → 落点 0
    let mut alts = vec![0.0f32; n];
    for i in 1..n - 1 {
        let tt = i as f32 / (n - 1) as f32;
        let wave = (tt * std::f32::consts::PI * 2.0 * (path.zigzag.max(1) as f32)).sin();
        let cruise = path.cruise_alt * (1.0 + wave * path.alt_wobble);
        // 起飞 0→巡航（前 25%），降落巡航→0（后 20%）
        let env = (tt / 0.25).min(1.0).min(((1.0 - tt) / 0.2).min(1.0));
        alts[i] = cruise * env.max(0.0);
    }
    alts[0] = 0.0;
    alts[n - 1] = 0.0;

    FlightPath {
        pts: sm,
        cum,
        speeds,
        alts,
        total,
        from,
        to,
    }
}

/// 一条完整的飞行路径：航点 + 弧长时间表 + 高度表
struct FlightPath {
    pts: Vec<Vec2>,
    cum: Vec<f32>,
    speeds: Vec<f32>,
    alts: Vec<f32>,
    total: f32,
    from: Vec2,
    to: Vec2,
}

impl FlightPath {
    /// 采样：t∈[0,1] 返回 (位置, 高度, 瞬时速度 px/s)
    fn sample(&self, t: f32) -> (Vec2, f32, f32) {
        let tt = (t * self.total).min(self.total);
        // 弧长二分定位段
        let mut i = 0;
        while i < self.cum.len() - 2 && self.cum[i + 1] < tt {
            i += 1;
        }
        let seg_t = ((tt - self.cum[i]) / (self.cum[i + 1] - self.cum[i]).max(0.001)).clamp(0.0, 1.0);
        let pos = self.pts[i].lerp(self.pts[i + 1], seg_t);
        let alt = self.alts[i] + (self.alts[i + 1] - self.alts[i]) * seg_t;
        (pos, alt, self.speeds[i])
    }

    /// 末段是否进入滑翔（低空贴地）
    fn glide(&self, t: f32) -> bool {
        t > 0.8
    }
}

/// 苍蝇：高速 + 飘忽。真实家蝇飞行是「之字冲刺 + 忽快忽慢 + 高度乱跳」，
/// 整体仍朝落点推进，但路径抖动剧烈（与蝴蝶的舒展之字不同，苍蝇是急促的小幅抖拐）。
struct StraightFly;
impl Movement for StraightFly {
    fn path_style(&self) -> PathStyle {
        PathStyle {
            cruise_alt: 30.0,
            takeoff_spiral: 1.0, // 起飞急拐一下，逃窜感
            zigzag: 3,           // 3 段之字：飞行中突然变向
            turn_bias: 0.85,     // 每段 ±15°~51° 交替偏拐（急促抖拐）
            alt_wobble: 0.22,    // 高度忽高忽低
            speed_wobble: 0.38,  // 冲刺-减速节奏（家蝇标志性的 dart-pause）
        }
    }
    // 苍蝇天性贴人：自发飞行约一半概率落在光标附近环带
    fn cursor_landing_bias(&self) -> f32 {
        0.5
    }
}

/// 蝴蝶：滑翔 + 之字形巡航 + 高度起伏 + 螺旋起飞 + 变速
struct DriftFly;
impl Movement for DriftFly {
    fn path_style(&self) -> PathStyle {
        PathStyle {
            cruise_alt: 46.0,
            takeoff_spiral: 2.1, // 受惊后大角度螺旋转向，逃窜感
            zigzag: 3,           // 3 段之字，忽左忽右
            turn_bias: 0.95,     // 每段转向偏离 ±0.95 rad（约 ±54°）
            alt_wobble: 0.22,    // 高度明显起伏
            speed_wobble: 0.30,  // 忽快忽慢
        }
    }
    fn idle_limit(&self, rng: &mut Rng, base: f32) -> f32 {
        rng.range(base * 0.6, base * 1.5)
    }
    fn has_glide(&self) -> bool {
        true
    }
    // 蝴蝶不能悬停，进食靠大范围滑翔 + 明显高度起伏
    fn feed_glide_dist(&self) -> (f32, f32) {
        (130.0, 240.0)
    }
    fn feed_alt(&self) -> f32 {
        48.0
    }
    fn feed_alt_wobble(&self) -> f32 {
        22.0
    }
    // 蝴蝶进食 = 三维盘旋：绕投食点连续转圈，无猛掉头
    fn feed_orbit(&self) -> bool {
        true
    }
    fn feed_orbit_radius(&self) -> (f32, f32) {
        (70.0, 120.0)
    }
}

/// 蜘蛛等爬行类：低速直行，几乎不挪窝（占位用；蜘蛛开发中）
struct CrawlFly;
impl Movement for CrawlFly {
    fn path_style(&self) -> PathStyle {
        PathStyle {
            cruise_alt: 18.0,
            takeoff_spiral: 0.4,
            zigzag: 0,
            turn_bias: 0.2,
            alt_wobble: 0.06,
            speed_wobble: 0.08,
        }
    }
    fn fidget_chance(&self) -> f32 {
        0.4
    }
}

/// 蜻蜓：快速直飞、低空巡航、几乎不绕弯；落地用短促悬停式减速接近
/// （高频振翅减速，前端 glide 姿态映射回飞行动画），不猛冲撞停
struct DartFly;
impl Movement for DartFly {
    fn path_style(&self) -> PathStyle {
        PathStyle {
            cruise_alt: 30.0,
            takeoff_spiral: 0.9, // 蜻蜓逃窜几乎不螺旋，略偏一点就直冲
            zigzag: 1,
            turn_bias: 0.3,
            alt_wobble: 0.08,
            speed_wobble: 0.10,
        }
    }
    fn idle_limit(&self, rng: &mut Rng, base: f32) -> f32 {
        rng.range(base * 0.7, base * 1.6)
    }
    fn has_glide(&self) -> bool {
        true
    }
    // 蜻蜓 Glide 动画映射回飞行（前端 clipKeys.glide=['fly']），高频振翅减速

    // 亲近行为：蜻蜓亲人。亲密度 Lv2（≥50）起，鼠标静止一段时间后
    // 有概率主动飞到指针旁停靠。等级越高：等待越短、越主动。
    //   Lv2:  静止 30s / 概率 41%
    //   Lv6:  静止 22s / 概率 73%
    //   Lv10: 静止 14s / 概率 80%
    fn approach_rule(&self, level: i32) -> Option<ApproachRule> {
        if level < 2 {
            return None;
        }
        let lv = level.clamp(2, 10) as f32;
        Some(ApproachRule {
            still_secs: (34.0 - lv * 2.0).clamp(14.0, 34.0),
            chance: (0.25 + lv * 0.08).clamp(0.0, 0.8),
            dist: 70.0,
        })
    }
}

/// 物种注册表：加新物种 = 在这里加一条（参数 + 策略）
/// disabled：占位物种（模型未就绪 / 开发中），托盘菜单置灰不可选
pub struct SpeciesDef {
    pub id: &'static str,
    pub label: &'static str,
    pub params: Params,
    pub movement: &'static dyn Movement,
    pub disabled: bool,
}

pub const SPECIES: [SpeciesDef; 4] = [
    SpeciesDef {
        id: "butterfly",
        label: "蝴蝶",
        params: Params {
            flee_radius: 130.0,
            safe_radius: 240.0,
            peak_speed: 260.0,
            flee_boost: 1.1,
            cooldown: 0.3,
            spawn_offset: 18.0,
            idle_base: 8.0,
        },
        movement: &DriftFly,
        disabled: false,
    },
    SpeciesDef {
        id: "dragonfly",
        label: "蜻蜓",
        params: Params {
            flee_radius: 150.0,
            safe_radius: 240.0,
            peak_speed: 520.0, // 四翅高速，比蝴蝶快一倍
            flee_boost: 1.25,  // 起飞冲刺倍率（含 ramp 慢起，避免视觉突加速）
            cooldown: 0.4,
            spawn_offset: 18.0,
            idle_base: 6.0,
        },
        movement: &DartFly,
        disabled: false,
    },
    SpeciesDef {
        id: "fly",
        label: "苍蝇",
        params: Params {
            flee_radius: 96.0,
            safe_radius: 240.0,
            peak_speed: 460.0,
            flee_boost: 1.4,
            cooldown: 0.3,
            spawn_offset: 18.0,
            idle_base: 10.0,
        },
        movement: &StraightFly,
        disabled: false,
    },
    SpeciesDef {
        id: "spider",
        label: "蜘蛛（开发中）",
        params: Params {
            flee_radius: 110.0,
            safe_radius: 240.0,
            peak_speed: 320.0,
            flee_boost: 1.2,
            cooldown: 0.3,
            spawn_offset: 18.0,
            idle_base: 16.0,
        },
        movement: &CrawlFly,
        disabled: true,
    },
];

pub fn species_by_id(id: &str) -> Option<&'static SpeciesDef> {
    SPECIES.iter().find(|s| s.id == id)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Spawn,
    Perch,
    Flee,
    Glide,
    Feed,
    /// 亲密度亲近行为：飞到光标旁停靠
    Approach,
}

/// 栖息期间的小幅挪动（§5.1 fidget），独立于主状态的小插值
struct Fidget {
    from: Vec2,
    to: Vec2,
    t: f32,
    dur: f32,
}

pub struct Insect {
    pub pos: Vec2,
    pub facing: i8, // 1 朝右 / -1 朝左
    pub stage: i8,
    species: &'static SpeciesDef,
    state: State,
    /// 当前飞行路径（之字 + 高度 + 变速），Spawn/Flee/Glide 共用
    path: FlightPath,
    /// 沿路径进度 0~1（弧长参数化）
    t: f32,
    /// 当前高度（物理像素，地面 0）。飞行时=路径采样；滑翔=缓降；栖息=0
    pub alt: f32,
    /// 起飞瞬间高度（切入滑翔时从当前高度缓降）
    start_alt: f32,
    /// 减速接近段：切入初速（px/s）
    glide_v0: f32,
    /// 减速接近段：飞行方向（单位向量，切入位置 → 落点）
    glide_dir_x: f32,
    glide_dir_y: f32,
    /// 减速接近段：恒定减速度（px/s²，恰好落在落点且末速≈0）
    glide_dec: f32,
    /// 减速接近段计时（秒）
    glide_t: f32,
    idle: f32,
    idle_limit: f32,
    cooldown: f32,
    fidget: Option<Fidget>,
    rng: Rng,
    seed: u64,
    /// 上一帧位置（用于实时速度方向，供前端转向）
    prev_pos: Vec2,
    /// 栖息朝向（落地时保留最后的飞行方向，避免停止后固定朝右/左）
    rest_heading: f32,
    /// 平滑后的运动方向（单位向量，指数平滑）。供 heading() 使用：
    /// 消除之字拐点的航向阶跃与慢速段的方向切换，飞行轨迹更连贯。
    dir_sm: Vec2,
    /// 当前落点是否贴边（贴哪条边）。贴边时身体顺边、前端切侧视
    pub landing_edge: Option<Edge>,
    /// 当前惊扰程度 0~1（鼠标靠近速度映射），决定逃跑距离与随机性
    scare: f32,
    /// 当前亲密度等级 1~10（profile::affinity_level，启动/喂食后刷新）。
    /// 运行时数据不能放 const 物种注册表，故缓存到实例。
    affinity_level: i32,
    /// 亲近行为冷却（秒）：一次 Approach 结束后倒计时，防止鼠标静止时反复飞
    approach_cd: f32,
    /// 苍蝇「被赶走」冷却（秒）：停在鼠标旁时鼠标一动即赶走，180s 内不再亲近
    shoo_cd: f32,
    /// 是否处于「主动停在人身旁」状态（亲近/表演落地后 true）：只有此时鼠标
    /// 一动才算赶走；普通自发飞行恰落在鼠标附近不算
    shoo_armed: bool,
    /// Approach 飞行阶段：0=飞向光标旁；1=绕光标盘旋表演（bother）；2=落向旁边
    approach_phase: u8,
    /// 是否表演模式（静止 30s 触发的绕飞）：phase 1 绕小圈后落旁边
    approach_bother: bool,
    /// 表演绕圈方向（+1 顺 / -1 逆）
    approach_cw: f32,
    /// 表演计时 / 盘旋角 / 盘旋半径 / 表演时长
    approach_t: f32,
    approach_ang: f32,
    approach_radius: f32,
    approach_dur: f32,
    /// 投食点（进食目标位置，鼠标投喂处）
    feed_target: Vec2,
    /// 进食阶段：0=飞向投食点，1=滑翔进食
    feed_phase: u8,
    /// 进食段内计时（当前滑翔段）
    feed_t: f32,
    /// 进食中心点（左右滑翔围绕它）
    feed_base: Vec2,
    /// 当前滑翔方向（+1 右 / -1 左）
    feed_dir: f32,
    /// 剩余掉头次数（3~5 随机）
    feed_rounds: i32,
    /// 当前滑翔段起点
    feed_from: Vec2,
    /// 当前滑翔段终点
    feed_to: Vec2,
    /// 当前滑翔段时长（秒）
    feed_seg_dur: f32,
    /// 盘旋累计角度（rad，进食盘旋用，连续累加实现平滑转向）
    feed_angle: f32,
    /// 盘旋半径（px，当前实际值；盘旋开始时从 0 渐扩到 feed_radius_target）
    feed_radius: f32,
    /// 盘旋目标半径（px）
    feed_radius_target: f32,
    /// 盘旋角速度（rad/s，正负号决定顺/逆时针）
    feed_angular: f32,
    /// 进食总时长（盘旋模式用）
    feed_total_dur: f32,
}

impl Insect {
    fn movement(&self) -> &'static dyn Movement {
        self.species.movement
    }

    /// 从光标附近出生，立刻飞向第一落点（§5.4）
    pub fn new(cursor: Vec2, screen: &Screen, seed: u64, species_id: &str) -> Self {
        let def = species_by_id(species_id).unwrap_or(&SPECIES[0]);
        let p = def.params;
        let off = Vec2::new(p.spawn_offset, -p.spawn_offset * 0.5);
        let start = cursor.add(off);
        let mut ins = Self {
            pos: start,
            facing: 1,
            stage: 3,
            species: def,
            state: State::Spawn,
            path: FlightPath {
                pts: vec![start, start],
                cum: vec![0.0, 1.0],
                speeds: vec![1.0],
                alts: vec![0.0, 0.0],
                total: 1.0,
                from: start,
                to: start,
            },
            t: 0.0,
            alt: 0.0,
            start_alt: 0.0,
            glide_v0: 0.0,
            glide_dir_x: 1.0,
            glide_dir_y: 0.0,
            glide_dec: 0.0,
            glide_t: 0.0,
            idle: 0.0,
            idle_limit: 10.0,
            cooldown: 0.0,
            fidget: None,
            rng: Rng::new(seed),
            seed,
            prev_pos: start,
            rest_heading: 0.0,
            dir_sm: Vec2::new(1.0, 0.0),
            landing_edge: None,
            scare: 0.0,
            affinity_level: 1,
            approach_cd: 0.0,
            shoo_cd: 0.0,
            shoo_armed: false,
            approach_phase: 0,
            approach_bother: false,
            approach_cw: 1.0,
            approach_t: 0.0,
            approach_ang: 0.0,
            approach_radius: 60.0,
            approach_dur: 3.0,
            feed_target: start,
            feed_phase: 0,
            feed_t: 0.0,
            feed_base: start,
            feed_dir: 1.0,
            feed_rounds: 0,
            feed_from: start,
            feed_to: start,
            feed_seg_dur: 2.5,
            feed_angle: 0.0,
            feed_radius: 0.0,
            feed_radius_target: 90.0,
            feed_angular: 1.6,
            feed_total_dur: 6.0,
        };
        ins.take_off(cursor, screen, true);
        ins
    }

    pub fn seed_ui(&self) -> u32 {
        ((self.seed >> 32) ^ (self.seed & 0xFFFFFFFF)) as u32
    }

    pub fn species_id(&self) -> &'static str {
        self.species.id
    }

    pub fn pose(&self) -> Pose {
        match self.state {
            State::Spawn => Pose::Spawn,
            State::Perch => Pose::Perch,
            State::Flee => Pose::Flee,
            State::Glide => Pose::Glide,
            State::Feed => Pose::Feed,
            State::Approach => Pose::Approach,
        }
    }

    /// 当前运动方向（世界坐标弧度）。飞行/滑翔 = 平滑后的速度方向；
    /// 栖息 = facing 映射（0 朝右 / π 朝左）。
    pub fn heading(&self) -> f32 {
        match self.state {
            State::Spawn | State::Flee | State::Glide | State::Feed | State::Approach => {
                // 平滑方向向量：旧实现「位移过小回退整体路径方向」会在慢速段
                // （起飞 ramp / 变速低谷）与实时方向来回切换，表现为航向突然
                // 左右甩头。指数平滑对拐点做圆滑过渡，全程无方向阶跃。
                if self.dir_sm.len() > 1e-4 {
                    self.dir_sm.y.atan2(self.dir_sm.x)
                } else {
                    self.path.to.sub(self.path.from).y.atan2(
                        self.path.to.sub(self.path.from).x,
                    )
                }
            }
            State::Perch => self.rest_heading,
        }
    }

    pub fn is_perched(&self) -> bool {
        self.state == State::Perch
    }

    pub fn flee_radius(&self) -> f32 {
        self.species.params.flee_radius
    }

    /// 换物种；返回是否真的变了（便于调用方决定是否持久化 / 发事件）。
    /// disabled（开发中）物种不可切换。
    pub fn set_species(&mut self, id: &str) -> bool {
        match species_by_id(id) {
            Some(def) if !def.disabled && def.id != self.species.id => {
                self.species = def;
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)] // 三期进化使用
    pub fn set_stage(&mut self, stage: i8) {
        self.stage = stage.clamp(1, 4);
    }

    /// 刷新亲密度等级（1~10）。启动与每次喂食后由主循环调用，
    /// 驱动亲近行为等档位判定。
    pub fn set_affinity_level(&mut self, level: i32) {
        self.affinity_level = level.clamp(1, 10);
    }

    /// 每帧推进；返回 Some 表示 pose 变化，需要通知前端。
    /// `scare` 0~1：当前惊扰程度（由主循环根据光标靠近速度算好传入）。
    /// `still_secs`：光标已静止秒数（环境感知，主循环累加），用于亲近行为。
    /// `cursor_active`：光标当前是否在移动（赶走苍蝇用，主循环传 cursor_speed 阈值）。
    pub fn update(
        &mut self,
        cursor: Vec2,
        screen: &Screen,
        dt: f32,
        scare: f32,
        still_secs: f32,
        cursor_active: bool,
    ) -> Option<Pose> {
        let pose = self.step(cursor, screen, dt, scare, still_secs, cursor_active);
        self.note_heading(dt);
        pose
    }

    /// 帧末刷新平滑方向向量（仅飞行态；栖息态朝向由 rest_heading 决定，
    /// fidget 挪动不算运动方向）。指数平滑：拐点/变速处航向圆滑过渡，
    /// 时间常数约 0.1s，肉眼无滞后感。
    fn note_heading(&mut self, dt: f32) {
        if self.state == State::Perch {
            return;
        }
        let d = self.pos.sub(self.prev_pos);
        let n = d.len();
        if n > 0.5 {
            let k = (dt * 10.0).clamp(0.0, 0.5);
            let target = Vec2::new(d.x / n, d.y / n);
            let mut s = self.dir_sm.lerp(target, k);
            let l = s.len();
            if l > 1e-4 {
                s = Vec2::new(s.x / l, s.y / l);
            }
            self.dir_sm = s;
        }
    }

    fn step(
        &mut self,
        cursor: Vec2,
        screen: &Screen,
        dt: f32,
        scare: f32,
        still_secs: f32,
        cursor_active: bool,
    ) -> Option<Pose> {
        self.scare = scare;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.approach_cd = (self.approach_cd - dt).max(0.0);
        self.shoo_cd = (self.shoo_cd - dt).max(0.0);
        let p = self.species.params;
        match self.state {
            State::Spawn | State::Flee => {
                // 弧长参数化推进（变速已烘焙进时间表）
                self.t = (self.t + dt / self.path.total).min(1.0);
                let (pos, alt, _sp) = self.path.sample(self.t);
                self.prev_pos = self.pos;
                self.pos = pos;
                self.alt = alt;
                // 减速接近：末段切入 Glide（蝴蝶滑翔 / 蜻蜓悬停式减速）
                if self.state == State::Flee
                    && self.movement().has_glide()
                    && self.path.glide(self.t)
                {
                    self.start_alt = self.alt; // 从当前高度开始缓降
                    // 物理匀减速模型：v0 = 当前路径段速度（不会像 easeOut 路径
                    // 映射那样初速爆表），方向朝落点（路径末段本就朝落点收敛，
                    // 转向角小）。减速度 = v0²/(2D) 恰好落在落点、末速≈0。
                    let sp_now = _sp.max(0.2); // sample 返回的段速度倍率
                    let v0 = (self.species.params.peak_speed * sp_now).min(2000.0);
                    let d = self.pos.dist(self.path.to);
                    if d < 8.0 || v0 < 60.0 {
                        // 已贴落点 / 速度过低：直接落地，不进入滑翔
                        self.land();
                        return Some(Pose::Perch);
                    }
                    self.glide_v0 = v0;
                    self.glide_dir_x = (self.path.to.x - self.pos.x) / d;
                    self.glide_dir_y = (self.path.to.y - self.pos.y) / d;
                    self.glide_dec = (v0 * v0 / (2.0 * d)).clamp(150.0, 6000.0);
                    self.glide_t = 0.0;
                    self.state = State::Glide;
                    return Some(Pose::Glide);
                }
                if self.t >= 1.0 {
                    // 到达落点：鼠标还在附近 → 不落地，继续飞（连飞）
                    if self.pos.dist(cursor) < p.flee_radius {
                        self.take_off(cursor, screen, true);
                        return Some(Pose::Flee);
                    }
                    self.land();
                    return Some(Pose::Perch);
                }
                None
            }
            State::Glide => {
                // 减速接近段：物理匀减速（切入 v0 → 0），方向 = 切入速度方向。
                // 减速距离按投影算，速度降到 ~0 即停（落点附近）。每帧位移
                // 线性收敛 —— 高频振翅的「减速接近停止」。
                self.glide_t += dt;
                let v = (self.glide_v0 - self.glide_dec * self.glide_t).max(0.0);
                let vx = self.glide_dir_x * v;
                let vy = self.glide_dir_y * v;
                self.prev_pos = self.pos;
                self.pos = Vec2::new(self.pos.x + vx * dt, self.pos.y + vy * dt);
                // 高度从 start_alt 缓降到地面（滑翔贴地感）
                let e_h = (self.glide_t / 0.4).min(1.0);
                self.alt = self.start_alt * (1.0 - e_h);
                // 停止：速度趋零（匀减速到终点），或已贴近落点
                let near_land = self.pos.dist(self.path.to) < 10.0;
                let done = v <= 2.0 || near_land;
                if done {
                    // 到达落点：鼠标还在附近 → 不落地，继续飞（连飞）
                    if self.pos.dist(cursor) < p.flee_radius {
                        self.take_off(cursor, screen, true);
                        return Some(Pose::Flee);
                    }
                    self.land();
                    return Some(Pose::Perch);
                }
                None
            }
            State::Approach => {
                if self.approach_phase == 0 {
                    // 亲近飞行：沿路径飞到光标旁
                    self.t = (self.t + dt / self.path.total).min(1.0);
                    let (pos, alt, _sp) = self.path.sample(self.t);
                    self.prev_pos = self.pos;
                    self.pos = pos;
                    self.alt = alt;
                    if self.t >= 1.0 {
                        if self.approach_bother {
                            // 表演模式：进 phase 1 —— 绕光标小圈飞舞
                            self.approach_phase = 1;
                            self.approach_t = 0.0;
                            self.approach_cw = if self.rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
                            self.approach_ang = self.rng.range(0.0, std::f32::consts::TAU);
                            self.approach_radius = self.rng.range(45.0, 70.0);
                            self.approach_dur = self.rng.range(2.5, 4.0);
                            return None;
                        }
                        // 普通亲近：直接落在光标旁（信任期，避免立刻惊飞）
                        self.land();
                        self.cooldown = 8.0;
                        self.idle_limit = self.idle_limit.max(10.0);
                        self.shoo_armed = self.species.id == "fly"; // 苍蝇主动停人身旁
                        return Some(Pose::Perch);
                    }
                    None
                } else if self.approach_phase == 1 {
                    // 绕光标小圈飞舞（hover 表演）
                    self.approach_t += dt;
                    self.approach_ang += self.approach_cw * 2.6 * dt;
                    // 以光标为圆心，半径为 approach_radius 的水平圆，高度约 40px 起伏
                    let a = self.approach_ang;
                    let wob = (self.approach_t * 6.0).sin() * 6.0;
                    let nx = (cursor.x + a.cos() * self.approach_radius)
                        .clamp(screen.x + 24.0, screen.x + screen.w - 24.0);
                    let ny = (cursor.y + a.sin() * self.approach_radius * 0.8)
                        .clamp(screen.y + 24.0, screen.y + screen.h - 24.0);
                    self.prev_pos = self.pos;
                    self.pos = Vec2::new(nx, ny);
                    self.alt = 40.0 + wob;
                    if self.approach_t >= self.approach_dur {
                        self.approach_phase = 2;
                        self.approach_t = 0.0;
                    }
                    None
                } else {
                    // phase 2：从 hover 高度垂直落下到旁边（0.35s 下沉）
                    self.approach_t += dt;
                    let e = (self.approach_t / 0.35).min(1.0);
                    self.prev_pos = self.pos;
                    self.alt = 40.0 * (1.0 - e);
                    if e >= 1.0 {
                        self.land();
                        self.cooldown = 8.0; // 信任期：鼠标一动由赶走逻辑处理
                        self.idle_limit = self.idle_limit.max(10.0);
                        self.shoo_armed = self.species.id == "fly"; // 表演落地：主动停人身旁
                        return Some(Pose::Perch);
                    }
                    None
                }
            }
            State::Feed => {
                if self.feed_phase == 0 {
                    // 阶段 0：飞向投食点（正常速度，非惊飞）
                    self.t = (self.t + dt / self.path.total).min(1.0);
                    let (pos, alt, _sp) = self.path.sample(self.t);
                    self.prev_pos = self.pos;
                    self.pos = pos;
                    self.alt = alt;
                    if self.t >= 1.0 {
                        // 到达投食点，进入进食阶段
                        self.feed_phase = 1;
                        self.feed_base = self.pos;
                        if self.movement().feed_orbit() {
                            // 盘旋模式（蝴蝶）：绕投食点连续转圈
                            // 半径先按屏边距离 clamp：越界后 x/y 会被推到屏边再反向，
                            // 形成原地折返。若屏边余量小于最小半径，把轨道中心往屏内
                            // 平移（余量 + rmin），保证最小半径的圆也完整在屏内。
                            let (rmin, rmax) = self.movement().feed_orbit_radius();
                            let m = 30.0; // 屏幕边界留边
                            let cx = self.pos.x;
                            let cy = self.pos.y;
                            let rx = (screen.x + screen.w - cx - m).min(cx - screen.x - m);
                            let ry = (screen.y + screen.h - cy - m).min(cy - screen.y - m);
                            // y 方向轨道是 x 的 0.7（y = r·0.7），按此换算
                            let rmax_fit = rx.min(ry / 0.7);
                            if rmax_fit < rmin {
                                // 屏边太近：把 base 往屏幕中心平移，留出 rmin 的净空
                                // （用 min/max 手动 clamp，避免超小屏上 clamp 下限>上限 panic）
                                let lo_x = screen.x + m + rmin;
                                let hi_x = screen.x + screen.w - m - rmin;
                                let lo_y = screen.y + m + rmin;
                                let hi_y = screen.y + screen.h - m - rmin;
                                let cx = if hi_x > lo_x { cx.clamp(lo_x, hi_x) } else { (cx - screen.x).min(screen.w) + screen.x };
                                let cy = if hi_y > lo_y { cy.clamp(lo_y, hi_y) } else { (cy - screen.y).min(screen.h) + screen.y };
                                self.feed_base = Vec2::new(cx, cy);
                            }
                            // 此时中心到各屏边 ≥ m+rmin，可用半径上限 = min(rmax, rmin + 新余量)
                            let rx = (screen.x + screen.w - self.feed_base.x - m)
                                .min(self.feed_base.x - screen.x - m);
                            let ry = (screen.y + screen.h - self.feed_base.y - m)
                                .min(self.feed_base.y - screen.y - m);
                            let rmax_fit = rx.min(ry / 0.7);
                            let rmax = rmax.min(rmax_fit.max(rmin));
                            self.feed_radius_target = rmin + self.rng.next_f32() * (rmax - rmin).max(0.0);
                            self.feed_radius = 0.0; // 从 0 渐扩到目标半径（无瞬移）
                            // 角速度 1.4~2.4 rad/s（一圈约 2.6~4.5s），随机顺/逆时针
                            self.feed_angular =
                                self.rng.range(1.4, 2.4) * if self.rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
                            self.feed_angle = if self.feed_base.x != self.pos.x
                                || self.feed_base.y != self.pos.y
                            {
                                // 起始角度 = 当前 pos 相对新轨道中心的方向（盘旋起点连续不跳变）
                                let d = self.pos.sub(self.feed_base);
                                d.y.atan2(d.x)
                            } else {
                                self.rng.range(0.0, std::f32::consts::TAU)
                            };
                            self.feed_total_dur = self.rng.range(6.0, 9.0); // 转 6~9 秒
                            self.feed_t = 0.0;
                        } else {
                            // 往复模式（蜻蜓）：左右滑翔 + 掉头
                            self.feed_dir = if self.rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
                            self.feed_rounds = 3 + (self.rng.next_f32() * 3.0) as i32; // 3~5 次掉头
                            self.begin_feed_seg(screen);
                        }
                        return Some(Pose::Feed);
                    }
                    None
                } else if self.movement().feed_orbit() {
                    // 阶段 1 盘旋（蝴蝶）：绕投食点转圈，朝向沿切线平滑连续。
                    // 每帧先按当前 angle 记录上帧位置，再积分角度算出本帧位置：
                    // prev_pos→pos 的位移 = 相邻两帧的圆上点 = 真实切线方向，
                    // heading() 才能拿到连续切线（不会退化为总落点方向猛甩头），
                    // 且位置与角度严格同步（同一个 angle 积分）。
                    self.feed_t += dt;
                    // 半径从 0 渐扩到目标（约 0.6s ease），盘旋从投食点自然散开，无瞬移
                    let grow = (self.feed_t / 0.6).min(1.0);
                    let r = self.feed_radius_target * (grow * grow * (3.0 - 2.0 * grow));
                    self.feed_radius = r;
                    let a0 = self.feed_angle;
                    let bx = self.feed_base.x;
                    let by = self.feed_base.y;
                    let p0x = bx + a0.cos() * r;
                    let p0y = by + a0.sin() * r * 0.7;
                    self.feed_angle += self.feed_angular * dt;
                    let a = self.feed_angle;
                    let wob = self.movement().feed_alt_wobble();
                    // 三维转圈：水平圆周 + 高度随角度起伏（上下来回飘）
                    let alt_now = self.movement().feed_alt()
                        + (a * 1.5).sin() * wob;
                    self.prev_pos = Vec2::new(p0x, p0y);
                    self.pos = Vec2::new(bx + a.cos() * r, by + a.sin() * r * 0.7);
                    self.alt = alt_now;
                    if self.feed_t >= self.feed_total_dur {
                        self.land();
                        return Some(Pose::Perch);
                    }
                    None
                } else {
                    // 阶段 1 往复（蜻蜓）：轻微滑翔 2~3 秒 → 掉头 → 反向滑翔，重复 3~5 次
                    self.feed_t += dt;
                    let k = (self.feed_t / self.feed_seg_dur).min(1.0);
                    let e = k * k * (3.0 - 2.0 * k); // smoothstep：滑翔减速感
                    self.prev_pos = self.pos;
                    self.pos = self.feed_from.lerp(self.feed_to, e);
                    // 高度：物种基准高度 + 正弦起伏（蝴蝶起伏明显、蜻蜓近悬停）
                    let wob = self.movement().feed_alt_wobble();
                    let bob = (self.feed_t * std::f32::consts::TAU * 0.5).sin() * wob;
                    let base_alt = self.movement().feed_alt();
                    self.pos.y = (self.pos.y + bob)
                        .clamp(screen.y + 20.0, screen.y + screen.h - 20.0);
                    self.alt = base_alt + bob;
                    if self.feed_t >= self.feed_seg_dur {
                        // 本段滑翔结束：掉头，或结束
                        self.feed_rounds -= 1;
                        if self.feed_rounds <= 0 {
                            self.land();
                            return Some(Pose::Perch);
                        }
                        self.feed_dir = -self.feed_dir; // 掉头
                        self.begin_feed_seg(screen);
                    }
                    None
                }
            }
            State::Perch => {
                self.idle += dt;
                let near = self.pos.dist(cursor) < p.flee_radius;
                let is_fly = self.species.id == "fly";

                // 苍蝇「赶走」检测：主动停在光标旁（亲近/表演落地），光标一动即
                // 惊飞，进入 3 分钟冷却（期间不亲近、不表演）。普通自发飞行落点
                // 恰在鼠标附近不算（shoo_armed=false）。
                if is_fly
                    && self.shoo_armed
                    && self.shoo_cd <= 0.0
                    && self.pos.dist(cursor) < 140.0
                    && cursor_active
                {
                    self.take_off(cursor, screen, true);
                    self.shoo_cd = 180.0;
                    self.shoo_armed = false;
                    self.approach_cd = self.approach_cd.max(20.0);
                    return Some(Pose::Flee);
                }

                // 亲近待命：抑制 idle 起飞（等静止攒够 / 掷骰），蜻蜓与苍蝇共用。
                // 苍蝇无亲密度门槛（天性），蜻蜓按亲和等级。
                let mut approach_pending = false;
                if !near && self.approach_cd <= 0.0 && self.shoo_cd <= 0.0 {
                    if is_fly {
                        // 苍蝇：光标在视野内(别贴脸)就考虑亲近——随机靠近无需静止，
                        // 表演绕飞需光标静止够久。不无条件待命：平时让它正常 idle
                        // 飞走（自发落点偏光标，保持「绕着人起落」的活感），只有
                        // 表演门槛临近时才抑制 idle 等表演。
                        let d_to_cursor = self.pos.dist(cursor);
                        if d_to_cursor > 120.0 {
                            if still_secs >= 30.0 {
                                // 静止 30s：表演绕飞（必触发一次）
                                let rule = ApproachRule { still_secs: 30.0, chance: 1.0, dist: 70.0 };
                                self.start_approach(cursor, screen, rule, true);
                                return Some(Pose::Approach);
                            } else if self.rng.next_f32() < dt / 25.0 {
                                // 随机靠近（平均 ~25s 一次机会）：普通飞近停靠
                                let rule = ApproachRule { still_secs: 0.0, chance: 1.0, dist: 90.0 };
                                self.start_approach(cursor, screen, rule, false);
                                return Some(Pose::Approach);
                            }
                            // 表演门槛临近（30s）：待命等表演，别先 idle 飞走
                            if still_secs >= 25.0 {
                                approach_pending = true;
                            }
                        }
                    } else if still_secs >= 6.0 {
                        if let Some(rule) = self.movement().approach_rule(self.affinity_level) {
                            let d_to_cursor = self.pos.dist(cursor);
                            if d_to_cursor > rule.dist + 40.0 {
                                if still_secs >= rule.still_secs && self.rng.next_f32() < rule.chance {
                                    self.start_approach(cursor, screen, rule, false);
                                    return Some(Pose::Approach);
                                }
                                // 未掷中/未到门槛：待命（不因 idle 飞走）
                                approach_pending = true;
                            }
                        }
                    }
                }
                // 起飞条件：光标进入警戒半径（冷却已过），或停留超时（§5.1）。
                // 亲近待命中不因 idle 飞走（等鼠标静止攒够 / 掷骰）。
                if (near && self.cooldown <= 0.0) || (self.idle >= self.idle_limit && !approach_pending) {
                    self.take_off(cursor, screen, near);
                    return Some(Pose::Flee);
                }
                // fidget：低概率小幅挪动，增加活感（§5.1 / §5.2 fidgetChance）
                if let Some(f) = &mut self.fidget {
                    f.t = (f.t + dt / f.dur).min(1.0);
                    let e = f.t * f.t * (3.0 - 2.0 * f.t); // smoothstep
                    self.pos = f.from.lerp(f.to, e);
                    if f.t >= 1.0 {
                        self.fidget = None;
                    }
                } else if self.rng.next_f32() < dt * 0.08 * self.movement().fidget_chance() {
                    // 平均约 12s 挪一次（甲虫更懒）
                    let a = self.rng.range(0.0, std::f32::consts::TAU);
                    let d = self.rng.range(3.0, 9.0);
                    let to = Vec2::new(
                        (self.pos.x + a.cos() * d).clamp(screen.x + 4.0, screen.x + screen.w - 4.0),
                        (self.pos.y + a.sin() * d).clamp(screen.y + 4.0, screen.y + screen.h - 4.0),
                    );
                    self.fidget = Some(Fidget {
                        from: self.pos,
                        to,
                        t: 0.0,
                        dur: self.rng.range(0.3, 0.7),
                    });
                    // 挪动方向决定朝向
                    if to.x != self.pos.x {
                        self.facing = if to.x >= self.pos.x { 1 } else { -1 };
                    }
                }
                None
            }
        }
    }

    /// 落地：进入栖息。贴边时身体顺边（前端切侧视），否则保留「最后一段的
    /// 实际速度方向」——之字末段 / 盘旋切线才是落地朝向，用路径整体方向
    /// （起飞→落点直线）会歪（盘旋完落地尤其明显）。
    fn land(&mut self) {
        let p = self.species.params;
        self.state = State::Perch;
        self.alt = 0.0;
        if let Some(e) = self.landing_edge {
            // 顺边：水平边朝右/左（随机），竖直边朝下/上（随机）
            let flip = if self.rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
            self.rest_heading = match e {
                Edge::Top | Edge::Bottom => if flip > 0.0 { 0.0 } else { std::f32::consts::PI },
                Edge::Left | Edge::Right => if flip > 0.0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 },
            };
            self.facing = if flip > 0.0 { 1 } else { -1 };
        } else {
            // 用平滑方向（与飞行中 heading() 同源）：末段慢速滑入时
            // 位移过小会退化为整体路径方向，落地瞬间航向突变的观感
            let d = self.dir_sm;
            self.rest_heading = if d.len() > 1e-4 {
                d.y.atan2(d.x)
            } else {
                self.path.to.sub(self.path.from).y.atan2(
                    self.path.to.sub(self.path.from).x,
                )
            };
        }
        self.cooldown = p.cooldown;
        self.idle = 0.0;
        self.idle_limit = self.movement().idle_limit(&mut self.rng, p.idle_base);
        self.shoo_armed = false; // 普通落地解除「停在人身旁」标记（亲近落地会再置 true）
        self.fidget = None; // 防御：确保落地不带任何残留插值（双保险）
    }

    fn take_off(&mut self, cursor: Vec2, screen: &Screen, scared: bool) {
        // 清除进行中的 fidget：否则落地恢复插值时 pos 会闪现回起飞点附近
        self.fidget = None;
        let landing = self.movement().pick_landing(
            &mut self.rng,
            self.pos,
            cursor,
            screen,
            scared,
            self.scare,
            &self.species.params,
        );
        let to = landing.pos;
        // 记录贴边：落地时身体顺边（前端切侧视）
        self.landing_edge = landing.edge;
        // 生成之字 + 高度 + 变速路径（起飞冲点方向由 scared 决定）
        // 受惊越猛，起飞冲段越快（flee_boost * scare 加成）。
        // scare 加成放缓（0.25 而非 0.4），避免高惊扰时蜻蜓 boost 飙到 >2×peak
        let boost = if scared {
            self.species.params.flee_boost * (1.0 + self.scare * 0.25)
        } else {
            1.0
        };
        self.path = build_flight_path(
            self.pos,
            to,
            cursor,
            scared,
            boost,
            self.species.params.peak_speed,
            screen,
            &self.movement().path_style(),
            &mut self.rng,
        );
        self.t = 0.0;
        self.alt = 0.0;
        self.start_alt = 0.0;
        self.facing = if to.x >= self.pos.x { 1 } else { -1 };
        self.state = State::Flee;
    }

    /// 强制起飞（托盘切换物种后调用）：让宠物从旧姿态立刻以新物种的参数起飞
    pub fn relaunch(&mut self, cursor: Vec2, screen: &Screen) {
        self.take_off(cursor, screen, false);
    }

    /// 开始一段进食滑翔：从当前位置向 feed_dir 方向滑翔一段随机距离（2~3 秒）
    fn begin_feed_seg(&mut self, screen: &Screen) {
        self.feed_from = self.pos;
        let (dmin, dmax) = self.movement().feed_glide_dist();
        let dist = self.rng.range(dmin, dmax);
        let tx = self.feed_base.x + self.feed_dir * dist;
        self.feed_to = Vec2::new(
            tx.clamp(screen.x + 20.0, screen.x + screen.w - 20.0),
            self.feed_base.y,
        );
        self.feed_seg_dur = self.rng.range(2.0, 3.0); // 轻微滑翔 2~3 秒
        self.feed_t = 0.0;
    }

    /// 投喂触发：生成飞向投食点的路径，切到 Feed 状态（先飞过去，再左右往复进食）
    pub fn start_feed(&mut self, pos: Vec2, screen: &Screen) {
        // 清除进行中的 fidget（同 take_off，防落地闪现）
        self.fidget = None;
        // 投食点 clamp 到工作区内部（留边距，避免贴边飞出）
        let margin = 24.0;
        let target = Vec2::new(
            pos.x.clamp(screen.x + margin, screen.x + screen.w - margin),
            pos.y.clamp(screen.y + margin, screen.y + screen.h - margin),
        );
        self.feed_target = target;
        self.feed_phase = 0;
        self.feed_t = 0.0;
        // 进食是新落点（普通栖息），清掉旧落点的贴边标记，避免进食完沿用旧贴边
        self.landing_edge = None;
        // 飞向投食点：不惊飞、正常速度、之字巡航
        self.path = build_flight_path(
            self.pos,
            target,
            target,
            false,
            1.0,
            self.species.params.peak_speed,
            screen,
            &self.movement().path_style(),
            &mut self.rng,
        );
        self.t = 0.0;
        self.alt = 0.0;
        self.start_alt = 0.0;
        self.facing = if target.x >= self.pos.x { 1 } else { -1 };
        self.state = State::Feed;
    }

    /// 亲近行为：飞到光标旁停靠。`bother=true` 时目标为光标正上方附近，
    /// 到达后进入绕飞表演再落旁边（苍蝇静止 30s 触发）；false 则直接落旁边。
    fn start_approach(&mut self, cursor: Vec2, screen: &Screen, rule: ApproachRule, bother: bool) {
        let margin = 24.0;
        self.approach_bother = bother;
        self.approach_phase = 0;
        let toward = self.pos.sub(cursor); // 光标 → 虫
        let tl = toward.len();
        // 默认落在「光标背离虫的一侧」的 dist 处：视觉上是绕到面前停住
        let (ux, uy) = if tl > 1.0 {
            (toward.x / tl, toward.y / tl)
        } else {
            (1.0f32, 0.0f32)
        };
        // 目标：普通模式 = 光标旁 dist；表演模式 = 光标上方附近(到达即绕飞)
        let tgt_dist = if bother { 30.0 } else { rule.dist };
        let mut target = Vec2::new(
            cursor.x + ux * tgt_dist,
            cursor.y + uy * tgt_dist,
        );
        for i in 1..8 {
            if screen.contains(target) {
                break;
            }
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            target = Vec2::new(
                cursor.x + a.cos() * tgt_dist,
                cursor.y + a.sin() * tgt_dist,
            );
        }
        target = Vec2::new(
            target.x.clamp(screen.x + margin, screen.x + screen.w - margin),
            target.y.clamp(screen.y + margin, screen.y + screen.h - margin),
        );
        self.landing_edge = None; // 亲近停靠是普通栖息，非贴边
        // 正常巡航飞过去（scared=false），比 Flee 温和
        self.path = build_flight_path(
            self.pos,
            target,
            cursor,
            false,
            1.0,
            self.species.params.peak_speed * 0.8, // 略慢：亲近不是逃命
            screen,
            &self.movement().path_style(),
            &mut self.rng,
        );
        self.t = 0.0;
        self.alt = 0.0;
        self.start_alt = 0.0;
        self.facing = if target.x >= self.pos.x { 1 } else { -1 };
        // 冷却：这次亲近后一段时间内不再触发（防鼠标静止时反复飞）
        self.approach_cd = if bother { 60.0 } else { 20.0 };
        self.state = State::Approach;
    }
}

use crate::platform::Screen;
