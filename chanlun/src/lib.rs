//! 缠论算法纯Rust库 — 零依赖, 可嵌入任意Rust项目

#[cfg(feature = "guidao")]
pub mod guidao;
#[cfg(feature = "guidao")]
#[cfg(feature = "guidao")]
#[cfg(feature = "guidao")]
#[cfg(feature = "guidao")]
#[cfg(feature = "guidao")]
pub mod zhongshu;

// ===== 数据类型 =====
#[derive(Clone, Debug)]
pub struct MergedCandle {
    pub high: f64,
    pub low: f64,
    pub high_bar_index: usize,
    pub low_bar_index: usize,
}

#[derive(Clone, Debug)]
pub struct Fractal {
    pub price: f64,
    pub is_top: bool,
    pub bar_index: usize,
    pub merged_index: usize,
    pub time: usize,
}

#[derive(Clone, Debug)]
pub struct Stroke {
    pub start_price: f64,
    pub end_price: f64,
    pub start_bar: usize,
    pub end_bar: usize,
    pub is_up: bool,
}

// ===== 缠K合并 =====
fn _is_contain(h1: f64, l1: f64, h2: f64, l2: f64) -> bool {
    (h1 >= h2 && l1 <= l2) || (h2 >= h1 && l2 <= l1)
}

pub fn process_merged_candles(highs: &[f64], lows: &[f64]) -> Vec<MergedCandle> {
    let n = highs.len();
    if n == 0 {
        return vec![];
    }
    let mut merged: Vec<MergedCandle> = Vec::with_capacity(n);
    merged.push(MergedCandle { high: highs[0], low: lows[0], high_bar_index: 0, low_bar_index: 0 });
    let mut trend: bool = false;

    for i in 1..n {
        let cur_h = highs[i];
        let cur_l = lows[i];
        let last = merged.last_mut().unwrap();

        if !_is_contain(cur_h, cur_l, last.high, last.low) {
            if cur_h > last.high && cur_l > last.low { trend = true; }
            else if cur_h < last.high && cur_l < last.low { trend = false; }
            merged.push(MergedCandle { high: cur_h, low: cur_l, high_bar_index: i, low_bar_index: i });
        } else if trend {
            let new_high = last.high.max(cur_h);
            let new_low = last.low.max(cur_l);
            if cur_h >= last.high { last.high_bar_index = i; }
            if cur_l >= last.low { last.low_bar_index = i; }
            last.high = new_high;
            last.low = new_low;
        } else {
            let new_high = last.high.min(cur_h);
            let new_low = last.low.min(cur_l);
            if cur_h <= last.high { last.high_bar_index = i; }
            if cur_l <= last.low { last.low_bar_index = i; }
            last.high = new_high;
            last.low = new_low;
        }
    }
    merged
}

// ===== 分型识别 =====
pub fn identify_fractals(merged: &[MergedCandle]) -> Vec<Fractal> {
    let mc = merged.len();
    if mc < 3 { return vec![]; }
    let mut points: Vec<Fractal> = Vec::new();
    for i in 0..mc - 2 {
        let left = &merged[i + 2];
        let mid = &merged[i + 1];
        let right = &merged[i];
        let is_top = mid.high > left.high && mid.high > right.high
            && mid.low > left.low && mid.low > right.low;
        let is_bot = mid.low < left.low && mid.low < right.low
            && mid.high < left.high && mid.high < right.high;
        if is_top {
            points.push(Fractal { price: mid.high, is_top: true, bar_index: mid.high_bar_index, merged_index: i + 1, time: mid.high_bar_index });
        } else if is_bot {
            points.push(Fractal { price: mid.low, is_top: false, bar_index: mid.low_bar_index, merged_index: i + 1, time: mid.low_bar_index });
        }
    }
    points
}

// ===== 分型过滤 =====
pub fn filter_fractals(points: &[Fractal]) -> Vec<Fractal> {
    let total = points.len();
    if total < 1 { return vec![]; }
    let mut sorted_pts: Vec<&Fractal> = points.iter().collect();
    sorted_pts.sort_by_key(|p| p.bar_index);
    let mut valid: Vec<Fractal> = vec![sorted_pts[0].clone()];
    for i in 1..total {
        let prev = valid.last().unwrap();
        let curr = sorted_pts[i];
        if curr.is_top != prev.is_top {
            valid.push(curr.clone());
        } else if curr.is_top {
            if curr.price > prev.price { *valid.last_mut().unwrap() = curr.clone(); }
        } else {
            if curr.price < prev.price { *valid.last_mut().unwrap() = curr.clone(); }
        }
    }
    valid
}

// ===== 笔处理 =====
/// Case1 有效性检查: 只扫描 last~current 之间的分型 (已被消费的旧分型不参与判断)
/// 对齐 MQL4 大段.mq4 ProcessStrokesFractals L354-384
fn check_original_valid(valid_fractals: &[Fractal], i: usize, last: &Fractal, current: &Fractal) -> bool {
    for j in (0..i).rev() {
        // 时间边界守卫: 到达 last 之前的分型即停止, 已被前面笔消费的旧分型不再干扰当前笔判断
        if valid_fractals[j].time < last.time { break; }
        if last.is_top && !current.is_top {
            if !valid_fractals[j].is_top && valid_fractals[j].price <= current.price && valid_fractals[j].price != current.price { return false; }
            if valid_fractals[j].is_top && valid_fractals[j].price >= last.price && valid_fractals[j].price != last.price { return false; }
        } else if !last.is_top && current.is_top {
            if valid_fractals[j].is_top && valid_fractals[j].price >= current.price && valid_fractals[j].price != current.price { return false; }
            if !valid_fractals[j].is_top && valid_fractals[j].price <= last.price && valid_fractals[j].price != last.price { return false; }
        }
    }
    true
}

fn check_up_stroke_case2(fractal_arr: &[Fractal], count: usize, start_idx: usize, first_low: f64, prev_high: f64) -> (bool, usize) {
    let mut end_idx = start_idx;
    let mut prev_high = prev_high;
    if start_idx + 1 >= count { return (false, end_idx); }
    for i in start_idx + 1..count {
        if !fractal_arr[i - 1].is_top && fractal_arr[i].is_top {
            let current_low = fractal_arr[i - 1].price;
            let current_high = fractal_arr[i].price;
            if current_low <= first_low { return (false, end_idx); }
            if current_high > prev_high { end_idx = i; return (true, end_idx); }
            prev_high = current_high;
        }
    }
    (false, end_idx)
}

fn check_down_stroke_case2(fractal_arr: &[Fractal], count: usize, start_idx: usize, first_high: f64, prev_low: f64) -> (bool, usize) {
    let mut end_idx = start_idx;
    let mut prev_low = prev_low;
    if start_idx + 1 >= count { return (false, end_idx); }
    for i in start_idx + 1..count {
        if fractal_arr[i - 1].is_top && !fractal_arr[i].is_top {
            let current_high = fractal_arr[i - 1].price;
            let current_low = fractal_arr[i].price;
            if current_high >= first_high { return (false, end_idx); }
            if current_low < prev_low { end_idx = i; return (true, end_idx); }
            prev_low = current_low;
        }
    }
    (false, end_idx)
}

// ===== 笔Case4 (事后修正: 直接映射线段Case4) =====
// 线段Case4: 单笔A包含后续(反向Case2段 + 同向Case2段)段对 → A以"一笔当线段"升格为线段.
// 笔Case4 同构映射: 候选笔 (last→current) 之后的分形序列若已自然形成完整"两笔结构"
//   (先反向后同向的两段虚拟笔, 各自满足类似Case2的连续新低/新高收口条件),
//   且两笔整体被 (last.price, current.price) 区间包含 (不破 last 价 / 不超 current 价)
//   → 该候选笔事后修正, 以 current 升格落笔.
// 分型层没有"已成笔"作输入 (笔正是本函数输出), 故以相邻分型对 (F_k, F_{k+1}) 为虚拟笔
//   单位: 分型序列严格交替 → 虚拟笔方向交替, 与线段层笔序列同构.
// 镜像对象 (lib.rs): check_up_segment_case4 / check_down_segment_case4 (线段层, 输入=已成笔).
fn check_up_stroke_case4(valid: &[Fractal], i: usize, low_a: f64, high_a: f64) -> bool {
    let n = valid.len();
    if i + 2 >= n { return false; } // current 之后至少还需 2 个分型才有结构可查
    // 阶段1: 找收口的反向(下行)两笔 — 镜像 check_down_segment_case2 于虚拟笔
    let mut down_end: usize = usize::MAX; // 下行收口虚拟笔起点 (顶分型索引)
    let mut up_start: usize = usize::MAX; // 上行收口虚拟笔起点 (底分型索引)
    let mut up_end: usize = usize::MAX;
    let mut k = i;
    while k + 1 < n {
        if down_end == usize::MAX {
            if valid[k].is_top {
                // 尝试以 k 为下行起点: 首个下行对 (F_k,F_{k+1}) 低点作基准, 后续下行对连续新低即收口
                let first_high = valid[k].price;
                let mut prev_low = valid[k + 1].price;
                let mut t = k + 2;
                let mut closed = false;
                while t + 1 < n {
                    if valid[t].is_top {
                        let cur_high = valid[t].price;
                        let cur_low = valid[t + 1].price;
                        if cur_high >= first_high { break; } // 高顶回试起点 → 该起点作废, 换下一下行起点
                        if cur_low < prev_low { down_end = t; closed = true; break; }
                        prev_low = cur_low;
                    }
                    t += 2;
                }
                if closed { k = down_end; } // 镜像 i = end_i: 收口后从其之后继续扫描
            }
        } else if !valid[k].is_top {
            // 阶段2: 找收口的同向(上行)两笔 — 镜像 check_up_segment_case2 于虚拟笔
            let first_low = valid[k].price;
            let mut prev_high = valid[k + 1].price;
            let mut t = k + 2;
            let mut closed = false;
            while t + 1 < n {
                if !valid[t].is_top {
                    let cur_low = valid[t].price;
                    let cur_high = valid[t + 1].price;
                    if cur_low <= first_low { break; } // 低点破坏 → 该起点作废, 换下一个上行起点
                    if cur_high > prev_high { up_start = k; up_end = t; closed = true; break; }
                    prev_high = cur_high;
                }
                t += 2;
            }
            if closed { break; }
        }
        k += 1;
    }
    if down_end == usize::MAX || up_end == usize::MAX { return false; }
    // 包含性窗口 (镜像 get_real_seg_end_price: 自 A 后继起, 含被跳过结构):
    // 真实下探最低 = 窗口 [i, up_start) 内全部下行虚拟笔低点 (首对 (F_i,F_{i+1}) 低点为初值)
    let mut real_low = valid[i + 1].price;
    let mut j = i + 1;
    while j < up_start && j + 1 < n {
        if valid[j].is_top { real_low = real_low.min(valid[j + 1].price); }
        j += 1;
    }
    if real_low <= low_a { return false; } // 不破 A 起点 (last 价) — 镜像严格 >
    // 真实反弹最高 = 窗口 [i, up_end] 内全部上行虚拟笔高点
    let mut real_high = valid[i + 1].price;
    let mut j = i + 1;
    while j <= up_end && j + 1 < n {
        if !valid[j].is_top { real_high = real_high.max(valid[j + 1].price); }
        j += 1;
    }
    if real_high > high_a { return false; } // 不超 A 终点 (current 价) — 镜像允许 =
    true
}

fn check_down_stroke_case4(valid: &[Fractal], i: usize, high_a: f64, low_a: f64) -> bool {
    let n = valid.len();
    if i + 2 >= n { return false; }
    // 阶段1: 找收口的反向(上行)两笔 — 镜像 check_up_segment_case2 于虚拟笔
    let mut up_end: usize = usize::MAX; // 上行收口虚拟笔起点 (底分型索引)
    let mut down_start: usize = usize::MAX; // 下行收口虚拟笔起点 (顶分型索引)
    let mut down_end: usize = usize::MAX;
    let mut k = i;
    while k + 1 < n {
        if up_end == usize::MAX {
            if !valid[k].is_top {
                let first_low = valid[k].price;
                let mut prev_high = valid[k + 1].price;
                let mut t = k + 2;
                let mut closed = false;
                while t + 1 < n {
                    if !valid[t].is_top {
                        let cur_low = valid[t].price;
                        let cur_high = valid[t + 1].price;
                        if cur_low <= first_low { break; }
                        if cur_high > prev_high { up_end = t; closed = true; break; }
                        prev_high = cur_high;
                    }
                    t += 2;
                }
                if closed { k = up_end; }
            }
        } else if valid[k].is_top {
            // 阶段2: 找收口的同向(下行)两笔 — 镜像 check_down_segment_case2 于虚拟笔
            let first_high = valid[k].price;
            let mut prev_low = valid[k + 1].price;
            let mut t = k + 2;
            let mut closed = false;
            while t + 1 < n {
                if valid[t].is_top {
                    let cur_high = valid[t].price;
                    let cur_low = valid[t + 1].price;
                    if cur_high >= first_high { break; }
                    if cur_low < prev_low { down_start = k; down_end = t; closed = true; break; }
                    prev_low = cur_low;
                }
                t += 2;
            }
            if closed { break; }
        }
        k += 1;
    }
    if up_end == usize::MAX || down_end == usize::MAX { return false; }
    // 包含性窗口 (镜像 check_down_segment_case4 的 get_real_seg_end_price 调用):
    // 真实反弹最高 = 窗口 [i, down_start) 内全部上行虚拟笔高点 (首对高点为初值)
    let mut real_high = valid[i + 1].price;
    let mut j = i + 1;
    while j < down_start && j + 1 < n {
        if !valid[j].is_top { real_high = real_high.max(valid[j + 1].price); }
        j += 1;
    }
    if real_high >= high_a { return false; } // 反弹不破 A 起点 (last 价) — 镜像严格 <
    // 真实下探最低 = 窗口 [i, down_end] 内全部下行虚拟笔低点
    let mut real_low = valid[i + 1].price;
    let mut j = i + 1;
    while j <= down_end && j + 1 < n {
        if valid[j].is_top { real_low = real_low.min(valid[j + 1].price); }
        j += 1;
    }
    if real_low < low_a { return false; } // 下探不破 A 终点 (current 价) — 镜像允许 =
    true
}
pub fn process_strokes_fractals(valid_fractals: &[Fractal], case3_enabled: bool, min_bars: usize) -> Vec<Fractal> {
    process_strokes_fractals_with_cases(valid_fractals, case3_enabled, true, min_bars)
}

/// 笔构建完整实现 (参数化: case4 可开关).
/// case3_enabled = C3 强势突破 + F1 修正开关; case4_enabled = C4 事后修正升格开关.
/// process_strokes_fractals (3参) = case4 恒开的历史行为, 输出零变化.
fn process_strokes_fractals_with_cases(valid_fractals: &[Fractal], case3_enabled: bool, case4_enabled: bool, min_bars: usize) -> Vec<Fractal> {
    let total = valid_fractals.len();
    if total < 1 { return vec![]; }
    let mut final_fractals: Vec<Fractal> = vec![valid_fractals[0].clone()];
    let mut i: usize = 1;

    while i < total {
        let last_idx = final_fractals.len() - 1;
        let last = &final_fractals[last_idx];
        let current = &valid_fractals[i];

        if last.is_top == current.is_top {
            if last.is_top { if current.price >= last.price { final_fractals[last_idx] = current.clone(); } }
            else { if current.price <= last.price { final_fractals[last_idx] = current.clone(); } }
            i += 1;
        } else {
            let bar_diff = if last.bar_index > current.bar_index { last.bar_index - current.bar_index } else { current.bar_index - last.bar_index };
            let is_up = !last.is_top && current.is_top;
            let is_down = last.is_top && !current.is_top;

            // 串行优先级 C1→C2→C3, 对齐 MQL4 大段.mq4 ProcessStrokesFractals
            let mut valid = false;
            let mut next_i = i + 1;
            let mut end_fractal_idx = i;

            // C1: 标准笔 (barDiff>=min_bars + mergedIndexDiff>1 + 区间无干扰分型)
            if bar_diff >= min_bars {
                let merged_index_diff = if last.merged_index > current.merged_index { last.merged_index - current.merged_index } else { current.merged_index - last.merged_index };
                if merged_index_diff > 1 {
                    if check_original_valid(valid_fractals, i, last, current) { valid = true; }
                }
            }

            // C2: 连续新高/新低突破 (C1未通过时尝试, 不受barDiff限制)
            if !valid {
                if is_up {
                    let (ok, end_idx) = check_up_stroke_case2(valid_fractals, total, i, last.price, current.price);
                    if ok {
                        valid = true;
                        next_i = end_idx + 1;
                        end_fractal_idx = end_idx;
                        // F1 修正: case3 先达标 (current 突破前前分型) 且 current 优于 C2 终点时,
                        // 落笔 current —— 时间优先, C2 不得用更劣终点顶替 case3 先锁定的端点
                        if case3_enabled && last_idx >= 1 {
                            let k = last_idx - 1;
                            if final_fractals[k].is_top == current.is_top
                                && current.price > final_fractals[k].price
                                && current.price > valid_fractals[end_idx].price
                            {
                                next_i = i + 1;
                                end_fractal_idx = i;
                            }
                        }
                    }
                } else if is_down {
                    let (ok, end_idx) = check_down_stroke_case2(valid_fractals, total, i, last.price, current.price);
                    if ok {
                        valid = true;
                        next_i = end_idx + 1;
                        end_fractal_idx = end_idx;
                        // F1 修正 (向下对称: current 更优 = 价格更低)
                        if case3_enabled && last_idx >= 1 {
                            let k = last_idx - 1;
                            if final_fractals[k].is_top == current.is_top
                                && current.price < final_fractals[k].price
                                && current.price < valid_fractals[end_idx].price
                            {
                                next_i = i + 1;
                                end_fractal_idx = i;
                            }
                        }
                    }
                }
            }

            // C3: 强势突破 (C1和C2都未通过时, 不受barDiff限制)
            if !valid && case3_enabled && last_idx >= 1 {
                let k = last_idx - 1;
                if final_fractals[k].is_top == current.is_top {
                    if is_up && current.price > final_fractals[k].price { valid = true; }
                    else if is_down && current.price < final_fractals[k].price { valid = true; }
                }
            }

            // C4: 事后修正升格 (直接映射线段Case4; case4_enabled 开关)
            // 终点尚未被 C1/C3/F1 锁为 current 时 (含 C2 延伸至更劣终点/三路全败),
            // 若 current 之后已自然形成完整两笔结构且整体包含于 (last.price, current.price)
            // → 候选笔 (last→current) 事后升格, 以 current 落笔 (链从 current 之后继续).
            if case4_enabled && !(valid && end_fractal_idx == i) {
                let c4_ok = if is_up {
                    check_up_stroke_case4(valid_fractals, i, last.price, current.price)
                } else if is_down {
                    check_down_stroke_case4(valid_fractals, i, last.price, current.price)
                } else {
                    false
                };
                if c4_ok {
                    valid = true;
                    next_i = i + 1;
                    end_fractal_idx = i;
                }
            }

            if valid {
                final_fractals.push(valid_fractals[end_fractal_idx].clone());
                i = next_i;
            } else { i += 1; }
        }
    }

    // ── 初始数据入口·链头前置反向笔 ──
    // 窗口起点滑动区 (final[0] 之前、被"隔位同向替换"吸收掉的分型) 内若存在与 final[0]
    // 异向的极值分型 bm, 且 (bm → final[0]) 满足标准笔条件 (C1: bar 间隔 + merged 间隔
    // + 区间无干扰), 则以 bm 为链起点前置插入 — 第一笔 = bm→final[0] (反向笔),
    // 后续链逐位不变 (前置插入后原链各端点仍是交替序列, 输出与原链完全一致).
    // 语义: 初始数据第一个反向(顶/底)分型满足分型+笔条件即可成为第一笔起点,
    // 无需其前存在可成笔的前导分型: 窗口起点滑动区内与链首异向的极值分型
    // 即可成为第一笔起点 (旧行为链起点绑定首分型同向族极值,
    // 反向分型永远没有机会成为笔起点).
    if final_fractals.len() >= 2 {
        let a0 = &final_fractals[0];
        if let Some(a0_idx) = valid_fractals
            .iter()
            .position(|f| f.bar_index == a0.bar_index && f.merged_index == a0.merged_index)
        {
            // 滑动区反向极值: valid[..a0_idx] 中与 a0 异向者取极值 (顶更高/底更低)
            let mut bm: Option<&Fractal> = None;
            for f in valid_fractals[..a0_idx].iter() {
                if f.is_top != a0.is_top {
                    let better = match bm {
                        None => true,
                        Some(cur) => if f.is_top { f.price > cur.price } else { f.price < cur.price },
                    };
                    if better { bm = Some(f); }
                }
            }
            if let Some(bm) = bm {
                let bar_diff = a0.bar_index.abs_diff(bm.bar_index);
                if bar_diff >= min_bars {
                    let mdiff = a0.merged_index.abs_diff(bm.merged_index);
                    if mdiff > 1 && check_original_valid(valid_fractals, a0_idx, bm, a0) {
                        final_fractals.insert(0, bm.clone());
                    }
                }
            }
        }
    }
    final_fractals
}
pub fn build_strokes(final_fractals: &[Fractal]) -> Vec<Stroke> {
    let total = final_fractals.len();
    if total < 2 { return vec![]; }
    let mut strokes: Vec<Stroke> = Vec::with_capacity(total - 1);
    for i in 0..total - 1 {
        let s = &final_fractals[i];
        let e = &final_fractals[i + 1];
        let is_up = !s.is_top && e.is_top;
        strokes.push(Stroke { start_price: s.price, end_price: e.price, start_bar: s.bar_index, end_bar: e.bar_index, is_up });
    }
    strokes
}

// ===== 线段 =====
pub fn check_up_segment_case2(strokes: &[Stroke], start_idx: usize) -> (bool, usize) {
    let count = strokes.len();
    if start_idx >= count || !strokes[start_idx].is_up { return (false, start_idx); }
    if start_idx + 1 >= count { return (false, start_idx); }
    let first_low = strokes[start_idx].start_price;
    let mut prev_high = strokes[start_idx].end_price;
    for i in start_idx + 1..count {
        if strokes[i].is_up {
            let cur_low = strokes[i].start_price;
            let cur_high = strokes[i].end_price;
            if cur_low <= first_low { return (false, start_idx); }
            if cur_high > prev_high { return (true, i); }
            prev_high = cur_high;
        }
    }
    (false, start_idx)
}

pub fn check_down_segment_case2(strokes: &[Stroke], start_idx: usize) -> (bool, usize) {
    let count = strokes.len();
    if start_idx >= count || strokes[start_idx].is_up { return (false, start_idx); }
    if start_idx + 1 >= count { return (false, start_idx); }
    let first_high = strokes[start_idx].start_price;
    let mut prev_low = strokes[start_idx].end_price;
    for i in start_idx + 1..count {
        if !strokes[i].is_up {
            let cur_high = strokes[i].start_price;
            let cur_low = strokes[i].end_price;
            if cur_high >= first_high { return (false, start_idx); }
            if cur_low < prev_low { return (true, i); }
            prev_low = cur_low;
        }
    }
    (false, start_idx)
}

fn check_up_segment_case3(
    strokes: &[Stroke],
    start_idx: usize,
    seg_case2_cache: &[(bool, usize)],
    c3_seg_starts: &[bool],
) -> (bool, usize, usize) {
    // 返回 (ok, start_idx, base_idx): base_idx=基准笔索引 (回溯命中的反向笔), 供转多信号对齐
    let count = strokes.len();
    if start_idx >= count || !strokes[start_idx].is_up { return (false, start_idx, usize::MAX); }
    let high_a = strokes[start_idx].end_price;
    for i in (0..start_idx).rev() {
        if !strokes[i].is_up {
            let (ok, end_i) = seg_case2_cache[i];
            if ok {
                // 守卫: 反向 Case2 段必须完整结束在当前笔开始之前
                if end_i >= start_idx { return (false, start_idx, usize::MAX); }
                return (high_a > strokes[i].start_price, start_idx, i);
            }
            // 转空段阻断: 独立建立的 C3 段起点笔 → 以其起点价为唯一基准并停止回溯
            if c3_seg_starts[i] {
                return (high_a > strokes[i].start_price, start_idx, i);
            }
        }
    }
    (false, start_idx, usize::MAX)
}

fn check_down_segment_case3(
    strokes: &[Stroke],
    start_idx: usize,
    seg_case2_cache: &[(bool, usize)],
    c3_seg_starts: &[bool],
) -> (bool, usize, usize) {
    // 返回 (ok, start_idx, base_idx): base_idx=基准笔索引 (回溯命中的反向笔), 供转空信号对齐
    let count = strokes.len();
    if start_idx >= count || strokes[start_idx].is_up { return (false, start_idx, usize::MAX); }
    let low_a = strokes[start_idx].end_price;
    for i in (0..start_idx).rev() {
        if strokes[i].is_up {
            let (ok, end_i) = seg_case2_cache[i];
            if ok {
                // 守卫: 反向 Case2 段必须完整结束在当前笔开始之前
                if end_i >= start_idx { return (false, start_idx, usize::MAX); }
                return (low_a < strokes[i].start_price, start_idx, i);
            }
            // 转多段阻断: 独立建立的 C3 段起点笔 → 以其起点价为唯一基准并停止回溯
            if c3_seg_starts[i] {
                return (low_a < strokes[i].start_price, start_idx, i);
            }
        }
    }
    (false, start_idx, usize::MAX)
}

// ===== 线段Case4 + 端点延伸 =====
fn get_real_seg_end_price(strokes: &[Stroke], seg_start_idx: usize, is_up: bool, boundary_idx: usize) -> f64 {
    let mut end_price = strokes[seg_start_idx].end_price;
    for i in seg_start_idx + 1..boundary_idx {
        if i >= strokes.len() { break; }
        if strokes[i].is_up == is_up {
            let cand = strokes[i].end_price;
            if is_up && cand > end_price { end_price = cand; }
            else if !is_up && cand < end_price { end_price = cand; }
        }
    }
    end_price
}

fn check_up_segment_case4(strokes: &[Stroke], start_idx: usize) -> (bool, usize) {
    let count = strokes.len();
    if start_idx + 2 >= count { return (false, start_idx); }
    if !strokes[start_idx].is_up { return (false, start_idx); }
    let low_a = strokes[start_idx].start_price;
    let high_a = strokes[start_idx].end_price;

    let mut down_seg_end: i32 = -1;
    let mut up_seg_start: i32 = -1;
    let mut up_seg_end: i32 = -1;

    let mut i = start_idx + 1;
    while i < count {
        if down_seg_end < 0 {
            if !strokes[i].is_up {
                let (ok, end_i) = check_down_segment_case2(strokes, i);
                if ok { down_seg_end = end_i as i32; i = end_i; }
            }
        } else {
            if strokes[i].is_up {
                let (ok, end_i) = check_up_segment_case2(strokes, i);
                if ok { up_seg_start = i as i32; up_seg_end = end_i as i32; break; }
            }
        }
        i += 1;
    }

    if down_seg_end < 0 || up_seg_end < 0 { return (false, start_idx); }

    // BUG修复: 检查窗口覆盖 [start_idx+1, ...) 含被跳过的反向笔
    // 原实现从 down_seg_start/up_seg_start 起查, 紧邻反向笔若不满足Case2被跳过则其突破被漏检
    let real_down_low = get_real_seg_end_price(strokes, start_idx + 1, false, up_seg_start as usize);
    if real_down_low <= low_a { return (false, start_idx); }

    let real_up_high = get_real_seg_end_price(strokes, start_idx + 1, true, up_seg_end as usize + 1);
    if real_up_high > high_a { return (false, start_idx); }

    (true, start_idx)
}

fn check_down_segment_case4(strokes: &[Stroke], start_idx: usize) -> (bool, usize) {
    let count = strokes.len();
    if start_idx + 2 >= count { return (false, start_idx); }
    if strokes[start_idx].is_up { return (false, start_idx); }
    let high_a = strokes[start_idx].start_price;
    let low_a = strokes[start_idx].end_price;

    let mut up_seg_end: i32 = -1;
    let mut down_seg_start: i32 = -1;
    let mut down_seg_end: i32 = -1;

    let mut i = start_idx + 1;
    while i < count {
        if up_seg_end < 0 {
            if strokes[i].is_up {
                let (ok, end_i) = check_up_segment_case2(strokes, i);
                if ok { up_seg_end = end_i as i32; i = end_i; }
            }
        } else {
            if !strokes[i].is_up {
                let (ok, end_i) = check_down_segment_case2(strokes, i);
                if ok { down_seg_start = i as i32; down_seg_end = end_i as i32; break; }
            }
        }
        i += 1;
    }

    if up_seg_end < 0 || down_seg_end < 0 { return (false, start_idx); }

    // BUG修复: 检查窗口覆盖 [start_idx+1, ...) 含被跳过的反向笔
    // 原实现从 up_seg_start/down_seg_start 起查, 紧邻反向笔若不满足Case2被跳过则其突破被漏检
    let real_up_high = get_real_seg_end_price(strokes, start_idx + 1, true, down_seg_start as usize);
    if real_up_high >= high_a { return (false, start_idx); }

    let real_down_low = get_real_seg_end_price(strokes, start_idx + 1, false, down_seg_end as usize + 1);
    if real_down_low < low_a { return (false, start_idx); }

    (true, start_idx)
}

pub fn process_segments(strokes: &[Stroke]) -> Vec<(usize, usize, bool)> {
    // 线段层: Case2/Case3/Case4 全部输出 (Case3 保留)
    process_segments_with_cases(strokes, true, true)
}

/// 段构建主逻辑 (线段/大段共用): enable_case3=true 时正常输出 Case3 候选,
/// enable_case4=false 时关闭 Case4 事后修正候选.
/// (process_segments / process_segments_with_case3 旧签名为全开包装)
pub fn process_segments_with_cases(
    strokes: &[Stroke],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    let count = strokes.len();
    if count < 2 { return vec![]; }

    // Pre-compute Case2 cache (used by Case3)
    let mut seg_case2_cache: Vec<(bool, usize)> = Vec::with_capacity(count);
    for i in 0..count {
        if strokes[i].is_up { seg_case2_cache.push(check_up_segment_case2(strokes, i)); }
        else { seg_case2_cache.push(check_down_segment_case2(strokes, i)); }
    }

    let mut segments: Vec<(usize, usize, bool)> = Vec::new();
    let mut c3_seg_starts: Vec<bool> = vec![false; count];
    let mut i: usize = 0;
    let mut looking_for_up = true;
    // 首段向下兜底 (与高级段层同构): 窗口起点若先出现向下笔
    // (初始数据无前导向上结构), 该向下笔满足段条件 (C2/C3/C4) 也直接建成第一条向下线段,
    // 不再以"隐含向上前提"跳过; 大段层经投影复用本函数自动继承.
    let mut first = true;

    while i < count {
        let mut found = false;
        let mut best_end: usize = 0;
        let mut is_c3 = false;

        if looking_for_up && strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            // Case2: consecutive higher-highs
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            // Case3: break prev down-segment high (one-stroke segment)
            if enable_case3 {
                let (ok_c3, _, _) = check_up_segment_case3(strokes, i, &seg_case2_cache, &c3_seg_starts);
                if ok_c3 { candidates.push(i); is_c3 = true; }
            }
            // Case4: post-hoc correction (contains down+up seg pair)
            if enable_case4 {
                let (ok_c4, _) = check_up_segment_case4(strokes, i);
                if ok_c4 { candidates.push(i); }
            }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            // Case2: consecutive lower-lows
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            // Case3: break prev up-segment low (one-stroke segment)
            if enable_case3 {
                let (ok_c3, _, _) = check_down_segment_case3(strokes, i, &seg_case2_cache, &c3_seg_starts);
                if ok_c3 { candidates.push(i); is_c3 = true; }
            }
            // Case4: post-hoc correction (contains up+down seg pair)
            if enable_case4 {
                let (ok_c4, _) = check_down_segment_case4(strokes, i);
                if ok_c4 { candidates.push(i); }
            }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !strokes[i].is_up {
            // 首段向下兜底 (与高级段层同构): 尚未建立任何线段时, 向下笔
            // Case2/Case3/Case4 成立也直接作为第一条线段 — 下跌行情/初始数据起点
            // 处向上段条件永不成立时, 向下笔不再被跳过 (不再以向上段为隐含前提).
            let mut candidates: Vec<usize> = Vec::new();
            // Case2: consecutive lower-lows
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            // Case3: break prev up-segment low (one-stroke segment, 首段兜底同样支持)
            if enable_case3 {
                let (ok_c3, _, _) = check_down_segment_case3(strokes, i, &seg_case2_cache, &c3_seg_starts);
                if ok_c3 { candidates.push(i); is_c3 = true; }
            }
            // Case4: post-hoc correction (首段向下兜底同样支持, 三分支行为一致)
            if enable_case4 {
                let (ok_c4, _) = check_down_segment_case4(strokes, i);
                if ok_c4 { candidates.push(i); }
            }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i { c3_seg_starts[i] = true; }
            // 首段向下兜底建立: 不翻转 (保持 looking_for_up=true → 下一条必须是向上段),
            // 后续向下笔被跳过并由端点延伸吸收进第一条, 保证整个下跌区间只生成一个向下线段
            let first_seg_down = first && !strokes[i].is_up;
            // 记录方向 = 实际笔方向 (首段向下时 looking_for_up 仍为 true, 不能用它记录)
            segments.push((i, best_end, strokes[i].is_up));
            i = best_end + 1;
            if !first_seg_down { looking_for_up = !looking_for_up; }
            first = false;
        } else { i += 1; }
    }

    // Extend: update segment endpoint if later same-direction stroke makes new extreme
    if !segments.is_empty() {
        for s in 0..segments.len() {
            let end_si = segments[s].1;
            let is_up = segments[s].2;
            let next_start_si = if s + 1 < segments.len() { segments[s + 1].0 } else { count };

            for si in end_si + 1..next_start_si {
                if si < count && strokes[si].is_up == is_up {
                    let cand_price = strokes[si].end_price;
                    if is_up && cand_price >= strokes[segments[s].1].end_price {
                        segments[s].1 = si;
                    } else if !is_up && cand_price <= strokes[segments[s].1].end_price {
                        segments[s].1 = si;
                    }
                }
            }
        }
    }

    segments
}

/// 状态机 Case3 建段事件收集 (供 guidao.rs 转空/转多信号对齐线段 Case3).
/// 重放 process_segments_with_case3 主循环 (复用同一套 C2/C3/C4 判定函数),
/// 捕获每个经 Case3 建立的段: 返回 (触发笔 j, 基准笔 base_i, 基准段终点 end_i, 段方向 is_up).
/// 案例: 向下笔经 Case3 建段时, 基准笔可为非线段起点的内部笔,
///       其 C2 段被消费但 seg_case2_cache 仍 ok → 回溯以它为基准.
/// 算法零改动: 仅收集事件, 不影响 segments 构建.
pub fn collect_segment_case3_events(strokes: &[Stroke]) -> Vec<(usize, usize, usize, bool)> {
    collect_segment_case3_events_with_cases(strokes, true, true)
}

/// 线段 Case3 事件收集带统一 levels 参数版:
/// enable_case3/4 必须与 process_segments_with_cases 同参, 保证事件与段结构一致.
pub fn collect_segment_case3_events_with_cases(
    strokes: &[Stroke],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, usize, bool)> {
    let count = strokes.len();
    if count < 2 { return vec![]; }

    // Pre-compute Case2 cache (used by Case3)
    let mut seg_case2_cache: Vec<(bool, usize)> = Vec::with_capacity(count);
    for i in 0..count {
        if strokes[i].is_up { seg_case2_cache.push(check_up_segment_case2(strokes, i)); }
        else { seg_case2_cache.push(check_down_segment_case2(strokes, i)); }
    }

    let mut events: Vec<(usize, usize, usize, bool)> = Vec::new();
    let mut c3_seg_starts: Vec<bool> = vec![false; count];
    let mut i: usize = 0;
    let mut looking_for_up = true;
    // 与 process_segments_with_cases 首段向下兜底完全同步
    // (事件收集必须与段构建同构, 否则轨道信号会引用不存在的 C3 段)
    let mut first = true;

    while i < count {
        let mut found = false;
        let mut best_end: usize = 0;
        let mut is_c3 = false;
        let mut base_idx: usize = usize::MAX;

        if looking_for_up && strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_up_segment_case3(strokes, i, &seg_case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_up_segment_case4(strokes, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(strokes, i, &seg_case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(strokes, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !strokes[i].is_up {
            // 首段向下兜底: 与 process_segments_with_cases 首段分支完全一致
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(strokes, i, &seg_case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(strokes, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i {
                c3_seg_starts[i] = true;
                // 基准段终点: C2-ok 基准笔 → 其 C2 段终点; C3 段起点基准笔 → 一笔当线段 [base..base]
                let end_i = if seg_case2_cache[base_idx].0 { seg_case2_cache[base_idx].1 } else { base_idx };
                // 事件方向 = 实际笔方向 (首段向下时 looking_for_up 仍为 true, 不能用它记录)
                events.push((i, base_idx, end_i, strokes[i].is_up));
            }
            // 首段向下兜底建立: 不翻转 looking_for_up, 后续向下笔被跳过并由端点延伸吸收
            let first_seg_down = first && !strokes[i].is_up;
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    events
}

// ===== 大段 (投影法: 线段→Stroke, 复用 process_segments 逻辑; Case3 正常输出) =====
/// 将线段数组投影为 Stroke 数组，每个"Stroke"代表一条线段，
/// 然后直接调用已验证的 process_segments 逻辑，零逻辑偏差。
/// 线段/大段/高级段三分支一致: Case2/3/4 全部输出.
pub fn process_big_segments(strokes: &[Stroke], segs: &[(usize, usize, bool)]) -> Vec<(usize, usize, bool)> {
    process_big_segments_with_cases(strokes, segs, true, true)
}

/// 大段 (投影法) 带统一 levels 参数版 (共用线段算法 C3/C4 门).
pub fn process_big_segments_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    if segs.len() < 2 { return vec![]; }

    // 投影: 每条线段→一个"虚拟线段"(Stroke 仅是实现载体)，is_up/起点价/终点价 经 线段→笔 索引解析
    let projected: Vec<Stroke> = segs
        .iter()
        .map(|(si, ei, is_up)| Stroke {
            is_up: *is_up,
            start_price: strokes[*si].start_price,
            end_price: strokes[*ei].end_price,
            start_bar: 0,  // 算法不依赖 bar 序号
            end_bar: 0,
        })
        .collect();

    // 大段层 Case2/3/4 全部输出
    process_segments_with_cases(&projected, enable_case3, enable_case4)
}

/// 高级段状态机 Case3 建段事件收集 (供 guidao.rs 大段轨道转空/转多信号对齐高级段 Case3).
/// 重放 process_superior_segments 主循环 (复用同一套 C2/C3/C4 判定函数),
/// 捕获每个经 Case3 建立的高级段: 返回 (触发大段 j, 基准大段 base_i, 基准段终点 end_i, 段方向 is_up).
/// 与 process_superior_segments 的差异点全部对齐:
///   1. 首段向下兜底分支 (else if first && !projected[i].is_up)
///   2. 首段向下建段后不翻转 looking_for_up (保持 true → 下一条必须是向上段)
///   3. 段方向 = projected[i].is_up (非 looking_for_up, 首段向下时二者不同)
/// 算法零改动: 仅收集事件, 不影响 superior_segments 构建.
pub fn collect_superior_case3_events(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
) -> Vec<(usize, usize, usize, bool)> {
    collect_superior_case3_events_with_cases(strokes, segs, bigs, true, true)
}

/// 高级段 Case3 事件收集带统一 levels 参数版:
/// enable_case3/4 必须与 process_superior_segments_with_cases 同参, 保证事件与段结构一致.
pub fn collect_superior_case3_events_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, usize, bool)> {
    if bigs.len() < 2 { return vec![]; }

    // 投影: 与 process_superior_segments 投影代码完全一致 (大段→线段→笔 三级索引解析)
    let projected: Vec<Stroke> = bigs
        .iter()
        .map(|(si, ei, is_up)| {
            let seg_s = segs[*si];
            let seg_e = segs[*ei];
            Stroke {
                is_up: *is_up,
                start_price: strokes[seg_s.0].start_price,
                end_price: strokes[seg_e.1].end_price,
                start_bar: 0,
                end_bar: 0,
            }
        })
        .collect();

    let count = projected.len();
    let mut case2_cache: Vec<(bool, usize)> = Vec::with_capacity(count);
    for i in 0..count {
        if projected[i].is_up {
            case2_cache.push(check_up_segment_case2(&projected, i));
        } else {
            case2_cache.push(check_down_segment_case2(&projected, i));
        }
    }

    let mut events: Vec<(usize, usize, usize, bool)> = Vec::new();
    let mut c3_seg_starts: Vec<bool> = vec![false; count];
    let mut i: usize = 0;
    let mut looking_for_up = true;
    let mut first = true;

    while i < count {
        let mut found = false;
        let mut best_end: usize = 0;
        let mut is_c3 = false;
        let mut base_idx: usize = usize::MAX;

        if looking_for_up && projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_up_segment_case3(&projected, i, &case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_up_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(&projected, i, &case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !projected[i].is_up {
            // 首段向下兜底: 与 process_superior_segments 主循环分支完全一致
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(&projected, i, &case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i {
                c3_seg_starts[i] = true;
                // 基准段终点: C2-ok 基准大段 → 其 C2 段终点; C3 段起点基准 → 一个大段当高级段 [base..base]
                let end_i = if case2_cache[base_idx].0 { case2_cache[base_idx].1 } else { base_idx };
                events.push((i, base_idx, end_i, projected[i].is_up));
            }
            // 首段向下兜底建立: 不翻转 looking_for_up, 后续向下大段被跳过并由端点延伸吸收
            let first_seg_down = first && !projected[i].is_up;
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    events
}

// ===== 高级段 (投影法: 大段→虚拟大段, Case2+Case3+Case4+延伸) =====
/// 将大段数组投影为 Stroke 数组，每个"Stroke"代表一条大段，
/// 然后执行 Case2 + Case3 + Case4 线段逻辑 + 端点延伸 (高级段层含 Case4,
/// 与"大段 = 投影后完整复用线段算法(含Case4)"同构, 三分支行为一致).
/// 100% 对齐 process_segments 的 C2/C3/C4 + extension 逻辑，零偏差。
pub fn process_superior_segments(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
) -> Vec<(usize, usize, bool)> {
    process_superior_segments_with_cases(strokes, segs, bigs, true, true)
}

/// 高级段 (投影法) 带统一 levels 参数版 (C3/C4 可关).
/// 旧 3 参签名 = 全开包装, 行为与历史完全一致.
pub fn process_superior_segments_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    if bigs.len() < 2 { return vec![]; }

    // 投影: 每条大段 → 一个"虚拟大段"(Stroke 仅是实现载体)，价格经 大段→线段→笔 索引解析
    let projected: Vec<Stroke> = bigs
        .iter()
        .map(|(si, ei, is_up)| {
            // 通过 segments 解析到真实笔: big_seg[si] → seg[si].0 → strokes[].start_price
            let seg_s = segs[*si];
            let seg_e = segs[*ei];
            Stroke {
                is_up: *is_up,
                start_price: strokes[seg_s.0].start_price,
                end_price: strokes[seg_e.1].end_price,
                start_bar: 0,
                end_bar: 0,
            }
        })
        .collect();

    // Case2 + Case3 + Case4 + 端点延伸 (与 process_segments 的 C2/C3/C4+extend 完全对齐)
    let count = projected.len();

    // Pre-compute Case2 cache (also used by Case3)
    let mut case2_cache: Vec<(bool, usize)> = Vec::with_capacity(count);
    for i in 0..count {
        if projected[i].is_up {
            case2_cache.push(check_up_segment_case2(&projected, i));
        } else {
            case2_cache.push(check_down_segment_case2(&projected, i));
        }
    }

    let mut superior_segs: Vec<(usize, usize, bool)> = Vec::new();
    let mut c3_seg_starts: Vec<bool> = vec![false; count];
    let mut i: usize = 0;
    let mut looking_for_up = true;
    let mut first = true;

    while i < count {
        let mut found = false;
        let mut best_end: usize = 0;
        let mut is_c3 = false;

        if looking_for_up && projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            // Case2: consecutive higher-highs
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            // Case3: break prev down-segment high (one-stroke segment)
            let (ok_c3, _, _) = check_up_segment_case3(&projected, i, &case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }
            // Case4: post-hoc correction (contains down+up bigseg pair)
            let (ok_c4, _) = check_up_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            // Case2: consecutive lower-lows
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            // Case3: break prev up-segment low (one-stroke segment)
            let (ok_c3, _, _) = check_down_segment_case3(&projected, i, &case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }
            // Case4: post-hoc correction (contains up+down bigseg pair)
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !projected[i].is_up {
            // 首段向下兜底: 尚未建立任何高级段时,
            // 向下段 Case2/Case3 成立也直接作为第一条高级段 — 下跌行情中
            // 反弹(向上)段 Case2 永不成立(不创新高), 方向永不翻转导致向下段
            // 被跳过而无法建段; 此处允许第一条向下段直接建立。
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            // Case3: break prev up-segment low (one-stroke segment, 首段兜底同样支持)
            let (ok_c3, _, _) = check_down_segment_case3(&projected, i, &case2_cache, &c3_seg_starts);
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }
            // Case4: post-hoc correction (首段向下兜底同样支持, 三分支行为一致)
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i { c3_seg_starts[i] = true; }
            // 首段向下兜底建立: 建立后不翻转 (保持 looking_for_up=true → 下一条必须是向上段),
            // 后续向下段被跳过并由端点延伸吸收进第一条, 保证整个下跌区间只生成一个向下高级段
            let first_seg_down = first && !projected[i].is_up;
            // 记录方向 = 实际段方向 (首段向下时 looking_for_up 仍为 true, 不能用它记录)
            superior_segs.push((i, best_end, projected[i].is_up));
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    // 端点延伸 (与 process_segments L457-474 完全对齐)
    if !superior_segs.is_empty() {
        for s in 0..superior_segs.len() {
            let end_si = superior_segs[s].1;
            let is_up = superior_segs[s].2;
            let next_start_si = if s + 1 < superior_segs.len() { superior_segs[s + 1].0 } else { count };

            for si in end_si + 1..next_start_si {
                if si < count && projected[si].is_up == is_up {
                    let cand_price = projected[si].end_price;
                    if is_up && cand_price >= projected[superior_segs[s].1].end_price {
                        superior_segs[s].1 = si;
                    } else if !is_up && cand_price <= projected[superior_segs[s].1].end_price {
                        superior_segs[s].1 = si;
                    }
                }
            }
        }
    }

    superior_segs
}

// ===== 二买/二卖检测 (高级段 "//" 结构两侧大段比较) =====
/// 二买: 向下高级段终点 B (低点) → A = B 左侧最近向上大段终点,
///       C = B 右侧最近向上大段终点 (需 > A), D = C 后第一个向下大段终点 (需 > B 不创新低)
///       → 标记"二买" (文字, 不画箭头)
/// 二卖: 完全镜像 (B = 向上高级段终点, A/C = 两侧向下大段终点, D = 向上大段回试 < B)
/// 仅分支1 (// 结构), 不映射分支2 (双段回试); 每高级段最多 1 个标记

/// 解析高级段终点: 入参 = 终点所在大段索引, 经 bigs→segs→strokes 逐级穿透到笔终点 (bar, price)
///   sups[k] = (大段索引, 大段索引, bool) → bigs[ei] = (线段索引, 线段索引, bool)
///   → segs[ei] = (笔索引, 笔索引, bool) → strokes[ei] 的终点 (bar, price)
pub(crate) fn resolve_sup_end_point(
    sup_end_big_idx: usize,
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, f64)> {
    let big = *bigs.get(sup_end_big_idx)?;
    let seg = *segs.get(big.1)?;
    let st = strokes.get(seg.1)?;
    Some((st.end_bar, st.end_price))
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondMarker {
    pub bar_index: usize,
    pub price: f64,
    /// true=二买, false=二卖
    pub is_buy: bool,
}

pub fn detect_second_buy_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<SecondMarker> {
    if sups.len() < 3 { return vec![]; }

    let mut markers: Vec<SecondMarker> = Vec::new();

    for &(_, ei, is_up) in sups {
        if is_up { continue; } // 只处理向下高级段

        // B = 向下高级段终点 (低点); ei 是大段索引
        let Some((_, b_price)) = resolve_sup_end_point(ei, bigs, segs, strokes) else { continue; };

        // A = B 左侧最近向上大段终点
        let mut a_price: Option<f64> = None;
        for k in (0..ei).rev() {
            if bigs[k].2 {
                a_price = resolve_sup_end_point(k, bigs, segs, strokes).map(|(_, p)| p);
                break;
            }
        }
        if let Some(a_price) = a_price {
            // C = B 右侧最近向上大段终点 (需 > A)
            let mut c_idx: usize = 0;
            let mut c_ok = false;
            for k in ei + 1..bigs.len() {
                if bigs[k].2 {
                    if let Some((_, price)) = resolve_sup_end_point(k, bigs, segs, strokes) {
                        if price > a_price {
                            c_idx = k;
                            c_ok = true;
                        }
                    }
                    break;
                }
            }
            if c_ok {
                // D = C 之后第一个向下大段 (回试, 需 > B 不创新低) → 标记"二买"
                for k in c_idx + 1..bigs.len() {
                    if !bigs[k].2 {
                        if let Some((bar, price)) = resolve_sup_end_point(k, bigs, segs, strokes) {
                            if price > b_price {
                                let marker = SecondMarker { bar_index: bar, price, is_buy: true };
                                if !markers.contains(&marker) {
                                    markers.push(marker);
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    markers
}

pub fn detect_second_sell_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<SecondMarker> {
    if sups.len() < 3 { return vec![]; }

    let mut markers: Vec<SecondMarker> = Vec::new();

    for &(_, ei, is_up) in sups {
        if !is_up { continue; } // 只处理向上高级段

        // B = 向上高级段终点 (高点); ei 是大段索引
        let Some((_, b_price)) = resolve_sup_end_point(ei, bigs, segs, strokes) else { continue; };

        // A = B 左侧最近向下大段终点
        let mut a_price: Option<f64> = None;
        for k in (0..ei).rev() {
            if !bigs[k].2 {
                a_price = resolve_sup_end_point(k, bigs, segs, strokes).map(|(_, p)| p);
                break;
            }
        }
        if let Some(a_price) = a_price {
            // C = B 右侧最近向下大段终点 (需 < A)
            let mut c_idx: usize = 0;
            let mut c_ok = false;
            for k in ei + 1..bigs.len() {
                if !bigs[k].2 {
                    if let Some((_, price)) = resolve_sup_end_point(k, bigs, segs, strokes) {
                        if price < a_price {
                            c_idx = k;
                            c_ok = true;
                        }
                    }
                    break;
                }
            }
            if c_ok {
                // D = C 之后第一个向上大段 (回试, 需 < B 不创新高) → 标记"二卖"
                for k in c_idx + 1..bigs.len() {
                    if bigs[k].2 {
                        if let Some((bar, price)) = resolve_sup_end_point(k, bigs, segs, strokes) {
                            if price < b_price {
                                let marker = SecondMarker { bar_index: bar, price, is_buy: false };
                                if !markers.contains(&marker) {
                                    markers.push(marker);
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    markers
}

// ===== 高层API: 一键全管线计算 =====
// ===== 高层API: 一键全管线计算 =====
/// 笔构建 Case 配置: 缠论指标参数 0/3/4/5, 数字即被关闭的 case;
/// 5=关case3+4 全关 (旧值 34 兼容).
/// 默认全开 = 与 ChanlunPipeline::new 完全一致的现行行为 (参数化零回归).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrokeCases {
    pub case3: bool,
    pub case4: bool,
}

impl Default for StrokeCases {
    fn default() -> Self {
        Self { case3: true, case4: true }
    }
}

impl StrokeCases {
    /// 参数值 → 开关: 0=全开, 3=关case3, 4=关case4, 5|34=关case3+4 (全关);
    /// 未知值按全开.
    pub fn from_param(v: u8) -> Self {
        match v {
            3 => Self { case3: false, case4: true },
            4 => Self { case3: true, case4: false },
            5 | 34 => Self { case3: false, case4: false },
            _ => Self::default(),
        }
    }
}

pub struct ChanlunPipeline {
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub merged: Vec<MergedCandle>,
    pub valid_fractals: Vec<Fractal>,
    pub final_fractals: Vec<Fractal>,
    pub strokes: Vec<Stroke>,
    pub segments: Vec<(usize, usize, bool)>,
    pub big_segments: Vec<(usize, usize, bool)>,
    pub superior_segments: Vec<(usize, usize, bool)>,
}

impl ChanlunPipeline {
    pub fn new(highs: Vec<f64>, lows: Vec<f64>) -> Self {
        Self::new_with_cases(highs, lows, StrokeCases::default(), StrokeCases::default())
    }

    /// 带笔 Case 配置的管线入口.
    /// new() 委托本函数且默认全开 → 历史行为零变化 (参数化零回归).
    pub fn new_with_stroke_cases(highs: Vec<f64>, lows: Vec<f64>, cases: StrokeCases) -> Self {
        Self::new_with_cases(highs, lows, cases, StrokeCases::default())
    }

    /// 全参数管线入口 (线段/大段/高级段共用统一 levels 开关):
    /// stroke_cases = 笔 C3/C4 开关; levels_cases = 线段/大段/高级段统一 C3/C4 开关.
    /// new()/new_with_stroke_cases 委托本函数且默认全开 → 历史行为零变化.
    pub fn new_with_cases(
        highs: Vec<f64>,
        lows: Vec<f64>,
        stroke_cases: StrokeCases,
        levels_cases: StrokeCases,
    ) -> Self {
        let merged = process_merged_candles(&highs, &lows);
        let points = identify_fractals(&merged);
        let valid = filter_fractals(&points);
        let ff = process_strokes_fractals_with_cases(&valid, stroke_cases.case3, stroke_cases.case4, 4);
        let strokes = build_strokes(&ff);
        let segs = process_segments_with_cases(&strokes, levels_cases.case3, levels_cases.case4);
        let bigs = process_big_segments_with_cases(&strokes, &segs, levels_cases.case3, levels_cases.case4);
        let sups = process_superior_segments_with_cases(&strokes, &segs, &bigs, levels_cases.case3, levels_cases.case4);

        Self { highs, lows, merged, valid_fractals: valid, final_fractals: ff, strokes, segments: segs, big_segments: bigs, superior_segments: sups }
    }

    /// 从已计算好的部件组装 Pipeline, 跳过所有算法计算.
    /// 用于跨指标共享 Pipeline 结果, 避免重复跑缠K合并→分型→笔→段→大段→高级段.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        highs: Vec<f64>,
        lows: Vec<f64>,
        merged: Vec<MergedCandle>,
        valid_fractals: Vec<Fractal>,
        final_fractals: Vec<Fractal>,
        strokes: Vec<Stroke>,
        segments: Vec<(usize, usize, bool)>,
        big_segments: Vec<(usize, usize, bool)>,
        superior_segments: Vec<(usize, usize, bool)>,
    ) -> Self {
        Self { highs, lows, merged, valid_fractals, final_fractals, strokes, segments, big_segments, superior_segments }
    }

    pub fn stats(&self) -> ChanlunStats {
        ChanlunStats {
            bars: self.highs.len(),
            merged: self.merged.len(),
            fractals: self.valid_fractals.len(),
            final_fractals: self.final_fractals.len(),
            strokes: self.strokes.len(),
            segments: self.segments.len(),
            big_segments: self.big_segments.len(),
            superior_segments: self.superior_segments.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChanlunStats {
    pub bars: usize,
    pub merged: usize,
    pub fractals: usize,
    pub final_fractals: usize,
    pub strokes: usize,
    pub segments: usize,
    pub big_segments: usize,
    pub superior_segments: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_pipeline() {
        let highs = vec![10.0, 12.0, 11.0, 13.0, 12.0, 14.0, 13.0, 15.0, 14.0, 16.0];
        let lows  = vec![ 8.0,  9.0,  7.0, 10.0,  8.0, 11.0,  9.0, 12.0, 10.0, 13.0];
        let pipeline = ChanlunPipeline::new(highs, lows);
        let stats = pipeline.stats();
        assert!(stats.bars == 10);
        assert!(stats.merged >= 1);
        // 只要有输出就说明管线能跑
    }

    #[test]
    fn test_second_buy_markers() {
        // 二买 (向下高级段反转后的回试结构):
        // 向下高级段 B=笔3 终点 100; A=笔2 终点 110; C=笔4 终点 112(>A); D=笔5 终点 104(>B)
        // 预期: 1 个标记 (文字"二买"), 画在 D 终点 (笔5 的 bar, 104)
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 100.0, end_price: 112.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 112.0, end_price: 104.0, start_bar: 15, end_bar: 17, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,true),(3,3,false),(4,4,true),(5,5,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = segs.clone();

        let markers = detect_second_buy_markers(&strokes, &segs, &bigs, &sups);
        assert_eq!(markers.len(), 1, "markers={:?}", markers);
        assert_eq!(markers[0], SecondMarker { bar_index: 17, price: 104.0, is_buy: true });
    }

    #[test]
    fn test_second_buy_c_not_higher_than_a() {
        // C(108) <= A(110) → 不标记
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 108.0, end_price: 104.0, start_bar: 15, end_bar: 17, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,true),(3,3,false),(4,4,true),(5,5,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = segs.clone();

        let markers = detect_second_buy_markers(&strokes, &segs, &bigs, &sups);
        assert!(markers.is_empty(), "markers={:?}", markers);
    }

    #[test]
    fn test_second_buy_d_breaks_b() {
        // D(98) <= B(100) 破位 → 不标记
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 100.0, end_price: 112.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 112.0, end_price: 98.0, start_bar: 15, end_bar: 17, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,true),(3,3,false),(4,4,true),(5,5,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = segs.clone();

        let markers = detect_second_buy_markers(&strokes, &segs, &bigs, &sups);
        assert!(markers.is_empty(), "markers={:?}", markers);
    }

    #[test]
    fn test_second_buy_multi_big_segments() {
        // 真实结构: 向下高级段 [D1 U1 D2] (B=D2 终点 100) + 向上高级段 [U3 D1' U4]
        // A=U1 终点 110, C=U3 终点 112(>A), D=D1' 终点 104(>B) → 二买画在 D 终点
        let strokes = vec![
            Stroke { start_price: 120.0, end_price: 100.0, start_bar: 0, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 6, end_bar: 11, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 12, end_bar: 17, is_up: false }, // B
            Stroke { start_price: 100.0, end_price: 112.0, start_bar: 18, end_bar: 23, is_up: true }, // C
            Stroke { start_price: 112.0, end_price: 104.0, start_bar: 24, end_bar: 29, is_up: false }, // D
            Stroke { start_price: 104.0, end_price: 108.0, start_bar: 30, end_bar: 35, is_up: true },
            Stroke { start_price: 108.0, end_price: 100.0, start_bar: 36, end_bar: 41, is_up: false }, // 第3高级段
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,false),(1,1,true),(2,2,false),(3,3,true),(4,4,false),(5,5,true),(6,6,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 2, false), (3, 5, true), (6, 6, false)];

        let markers = detect_second_buy_markers(&strokes, &segs, &bigs, &sups);
        assert_eq!(markers.len(), 1, "markers={:?}", markers);
        assert_eq!(markers[0], SecondMarker { bar_index: 29, price: 104.0, is_buy: true });
    }

    #[test]
    fn test_second_sell_markers() {
        // 二卖镜像: 向上高级段 B=笔1 终点 110; A=笔0 终点 100; C=笔2 终点 96(<A); D=笔3 终点 104(<B)
        // 预期: 1 个标记 (文字"二卖"), 画在 D 终点 (笔3 的 bar, 104)
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 0, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 6, end_bar: 11, is_up: true },
            Stroke { start_price: 110.0, end_price: 96.0, start_bar: 12, end_bar: 17, is_up: false },
            Stroke { start_price: 96.0, end_price: 104.0, start_bar: 18, end_bar: 23, is_up: true },
            Stroke { start_price: 104.0, end_price: 98.0, start_bar: 24, end_bar: 29, is_up: false },
            Stroke { start_price: 98.0, end_price: 103.0, start_bar: 30, end_bar: 35, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,false),(1,1,true),(2,2,false),(3,3,true),(4,4,false),(5,5,true)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = segs.clone();

        let markers = detect_second_sell_markers(&strokes, &segs, &bigs, &sups);
        assert_eq!(markers.len(), 1, "markers={:?}", markers);
        assert_eq!(markers[0], SecondMarker { bar_index: 23, price: 104.0, is_buy: false });
    }

    #[test]
    fn test_second_sell_d_breaks_b() {
        // D(112) >= B(110) 破位 → 不标记
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 0, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 6, end_bar: 11, is_up: true },
            Stroke { start_price: 110.0, end_price: 96.0, start_bar: 12, end_bar: 17, is_up: false },
            Stroke { start_price: 96.0, end_price: 112.0, start_bar: 18, end_bar: 23, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,false),(1,1,true),(2,2,false),(3,3,true)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = segs.clone();

        let markers = detect_second_sell_markers(&strokes, &segs, &bigs, &sups);
        assert!(markers.is_empty(), "markers={:?}", markers);
    }

    #[test]
    fn test_second_sell_multi_big_segments() {
        // 真实结构镜像: 向上高级段 [U1 D1 U2] (B=U2 终点 110) + 向下高级段 [D3 U1' D4]
        // A=D1 终点 100, C=D3 终点 96(<A), D=U1' 终点 104(<B) → 二卖画在 D 终点
        let strokes = vec![
            Stroke { start_price: 90.0, end_price: 115.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 115.0, end_price: 100.0, start_bar: 6, end_bar: 11, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 12, end_bar: 17, is_up: true }, // B
            Stroke { start_price: 110.0, end_price: 96.0, start_bar: 18, end_bar: 23, is_up: false }, // C
            Stroke { start_price: 96.0, end_price: 104.0, start_bar: 24, end_bar: 29, is_up: true }, // D
            Stroke { start_price: 104.0, end_price: 98.0, start_bar: 30, end_bar: 35, is_up: false },
            Stroke { start_price: 98.0, end_price: 110.0, start_bar: 36, end_bar: 41, is_up: true }, // 第3高级段
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,true),(3,3,false),(4,4,true),(5,5,false),(6,6,true)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 2, true), (3, 5, false), (6, 6, true)];

        let markers = detect_second_sell_markers(&strokes, &segs, &bigs, &sups);
        assert_eq!(markers.len(), 1, "markers={:?}", markers);
        assert_eq!(markers[0], SecondMarker { bar_index: 29, price: 104.0, is_buy: false });
    }

    #[test]
    fn test_second_insufficient() {
        // sups < 3 → 空
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 112.0, start_bar: 0, end_bar: 2, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();
        let sups: Vec<(usize, usize, bool)> = segs.clone();
        assert!(detect_second_buy_markers(&strokes, &segs, &bigs, &sups).is_empty());
        assert!(detect_second_sell_markers(&strokes, &segs, &bigs, &sups).is_empty());
    }

    #[test]
    fn test_superior_segments_down_first() {
        // 回归: 下跌行情中向下高级段必须可构建 (首段允许向下)
        // 笔序列: 反弹1(110→115) 下跌1(115→100) 反弹2(100→103) 下跌2(103→95) 反弹3(95→97) 下跌3(97→90)
        // 投影后: up(110→115) down(115→100) up(100→103) down(103→95) up(95→97) down(97→90)
        // up@0 Case2 不成立(反弹未创新高) → down@1 Case3 不成立(无前 Case2 up 段) → 首段兜底仅 Case2
        // 期望: 首段向下 (1,3,false) [down1→down2 Case2] + 端点延伸 down3 → (1,5,false)
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 115.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 115.0, end_price: 100.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 103.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 103.0, end_price: 95.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 95.0, end_price: 97.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 97.0, end_price: 90.0, start_bar: 15, end_bar: 17, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,true),(3,3,false),(4,4,true),(5,5,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();

        let sups = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups, vec![(1usize, 5usize, false)], "sups={:?}", sups);
    }

    #[test]
    fn test_superior_segments_down_only() {
        // 纯下跌 (无反弹段): 向下高级段必须可构建
        // 笔: 下1(120→105) 下2(105→95) 下3(95→88), 全部向下
        // 期望: 首段 (0,1,false) [下1→下2 Case2] + 端点延伸 下3 → (0,2,false)
        let strokes = vec![
            Stroke { start_price: 120.0, end_price: 105.0, start_bar: 0, end_bar: 2, is_up: false },
            Stroke { start_price: 105.0, end_price: 95.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 95.0, end_price: 88.0, start_bar: 6, end_bar: 8, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,false),(1,1,false),(2,2,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();

        let sups = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups, vec![(0usize, 2usize, false)], "sups={:?}", sups);
    }

    #[test]
    fn test_superior_segments_up_case4() {
        // 高级段 Case4 (三分支行为一致): 单向上大段 A 包含
        // 完整的反向(下)+同向(上)大段对 → A 修正为高级段 (endIdx = 自身)
        // 序列: up(110→120) down(120→112) down(112→111) up(111→116) up(116→119)
        // down@1→down@2 Case2 (低点连创新低), up@3→up@4 Case2 (低点抬高+高点创新高)
        // 反向极值 111 > 110 (未破 A 起点), 同向极值 119 <= 120 (未破 A 终点) → Case4 成立
        // 期望: A 单大段修正为高级段 (0,0,true); 其后 D1→D2 Case2 → (1,2,false),
        // U1→U2 Case2 → (3,4,true) (Case4 修正后剩余大段正常按 Case2 建段)
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 120.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 120.0, end_price: 112.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 112.0, end_price: 111.0, start_bar: 6, end_bar: 8, is_up: false },
            Stroke { start_price: 111.0, end_price: 116.0, start_bar: 9, end_bar: 11, is_up: true },
            Stroke { start_price: 116.0, end_price: 119.0, start_bar: 12, end_bar: 14, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,false),(3,3,true),(4,4,true)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();

        let sups = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups, vec![(0usize, 0usize, true), (1usize, 2usize, false), (3usize, 4usize, true)], "sups={:?}", sups);
    }

    #[test]
    fn test_superior_segments_down_case4_first() {
        // 高级段 Case4 首段向下兜底分支: 首条向下大段 A 含完整
        // 反向(上)+同向(下)大段对 → 同样修正为高级段 (与分支1/2行为一致)
        // 序列: down(120→110) up(110→118) up(118→119) down(119→112) down(112→111)
        // up@1→up@2 Case2, down@3→down@4 Case2; 反向极值 119 < 120, 同向极值 111 >= 110
        // 期望: 首段 (0,0,false) 走 Case4; 其后 up@1 Case2 → (1,2,true);
        // down@3 Case2 → (3,4,false) (D2 已是段内部件, 无端点延伸)
        let strokes = vec![
            Stroke { start_price: 120.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: false },
            Stroke { start_price: 110.0, end_price: 118.0, start_bar: 3, end_bar: 5, is_up: true },
            Stroke { start_price: 118.0, end_price: 119.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 119.0, end_price: 112.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 112.0, end_price: 111.0, start_bar: 12, end_bar: 14, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,false),(1,1,true),(2,2,true),(3,3,false),(4,4,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();

        let sups = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups, vec![(0usize, 0usize, false), (1usize, 2usize, true), (3usize, 4usize, false)], "sups={:?}", sups);
    }

    #[test]
    fn test_superior_segments_down_case4() {
        // 高级段 Case4 非首段向下分支: 首段向上建立后, 单向下大段 B
        // 含完整反向(上)+同向(下)大段对 → B 修正为高级段
        // 序列: up(100→110) up(110→115) down(115→105) up(105→113) up(113→114) down(114→107) down(107→106)
        // 首段 up@0 Case2 → (0,1,true); B 反向极值 114 < 115, 同向极值 106 >= 105 → Case4 成立
        // 期望: (0,1,true) (2,2,false) (3,4,true) (5,6,false)
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 110.0, end_price: 115.0, start_bar: 3, end_bar: 5, is_up: true },
            Stroke { start_price: 115.0, end_price: 105.0, start_bar: 6, end_bar: 8, is_up: false },
            Stroke { start_price: 105.0, end_price: 113.0, start_bar: 9, end_bar: 11, is_up: true },
            Stroke { start_price: 113.0, end_price: 114.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 114.0, end_price: 107.0, start_bar: 15, end_bar: 17, is_up: false },
            Stroke { start_price: 107.0, end_price: 106.0, start_bar: 18, end_bar: 20, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,true),(2,2,false),(3,3,true),(4,4,true),(5,5,false),(6,6,false)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();

        let sups = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups, vec![(0usize, 1usize, true), (2usize, 2usize, false), (3usize, 4usize, true), (5usize, 6usize, false)], "sups={:?}", sups);
    }

    #[test]
    fn test_superior_segments_up_case4_rejected_when_break_high() {
        // 高级段 Case4 否定: 同向大段突破 A 终点 (real_up_high > high_a)
        // → Case4 拒绝; 但突破同时使 A 的 Case2 成立 → 走 Case2 (endIdx = 突破笔)
        // 序列: up(110→120) down(120→112) down(112→111) up(111→116) up(116→121)
        // up@3→up@4 Case2 且 up@4 终点 121 > 120 → A Case2 成立 end=4
        // 期望: (0,4,true) (走 Case2, 非 Case4)
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 120.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 120.0, end_price: 112.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 112.0, end_price: 111.0, start_bar: 6, end_bar: 8, is_up: false },
            Stroke { start_price: 111.0, end_price: 116.0, start_bar: 9, end_bar: 11, is_up: true },
            Stroke { start_price: 116.0, end_price: 121.0, start_bar: 12, end_bar: 14, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,true),(1,1,false),(2,2,false),(3,3,true),(4,4,true)];
        let bigs: Vec<(usize, usize, bool)> = segs.clone();

        let sups = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups, vec![(0usize, 4usize, true)], "sups={:?}", sups);
    }

    #[test]
    fn test_case3_output_bigseg_and_superior() {
        // 大段/高级段 Case3 正常输出 (线段/大段/高级段三分支一致)
        // 线段/大段层同构首段向下兜底 (初始数据特殊情况) — 三层行为一致.
        // 序列: down(100→90) down(90→85) up(85→105)
        // 线段层: 首段兜底 down@0→down@1 Case2 → (0,1,false); up@2 突破前置 down Case2 段起点 100
        //         → Case3 成立 → (2,2,true)
        // 大段层: 投影复用线段算法 → 同 (0,1,false),(2,2,true)
        // 高级段层: 首段兜底同构 → (0,1,false),(2,2,true)
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 90.0, start_bar: 0, end_bar: 2, is_up: false },
            Stroke { start_price: 90.0, end_price: 85.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 85.0, end_price: 105.0, start_bar: 6, end_bar: 8, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![(0,0,false),(1,1,false),(2,2,true)];
        let bigs = segs.clone();

        // 线段层: 首段兜底 Case2 + Case3 输出
        let segs_out = process_segments(&strokes);
        assert_eq!(segs_out, vec![(0usize, 1usize, false), (2usize, 2usize, true)], "segs_out={:?}", segs_out);
        // 大段层: 投影复用 → 同线段层
        let bigs_out = process_big_segments(&strokes, &segs);
        assert_eq!(bigs_out, vec![(0usize, 1usize, false), (2usize, 2usize, true)], "bigs_out={:?}", bigs_out);
        // 高级段层: 首段兜底 Case2 + Case3 输出 (不变)
        let sups_out = process_superior_segments(&strokes, &segs, &bigs);
        assert_eq!(sups_out, vec![(0usize, 1usize, false), (2usize, 2usize, true)], "sups_out={:?}", sups_out);
    }

    #[test]
    fn test_superior_case4_aligns_bigseg_case4() {
        // 高级段 Case4 与大段 Case4 对齐验证: 同一虚拟序列分别走
        // "大段层" (process_big_segments: 线段→投影→process_segments, 含原生 Case4) 与
        // "高级段层" (process_superior_segments: 大段→投影→C2/C3/C4), 两路径输入等价
        // (segs/bigs 均为恒等映射 → 投影数组 = 原始笔序列).
        // 断言1/2: 首段向上场景两路径输出必须完全一致 → 证明高级段 Case4 挂载与
        // process_segments 原生 Case4 零偏差 (同一函数/同一候选/同一排序/同一推进).
        // 断言3: 首段向下场景差异仅为首段兜底 (有意行为), 后续段仍与大段层一致.
        let mk = |strokes: Vec<Stroke>| {
            let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate()
                .map(|(i, s)| (i, i, s.is_up)).collect();
            let bigs = segs.clone();
            (strokes, segs, bigs)
        };

        // 场景1: 向上 Case4 (A 单段含 下+上 对, 同 test_superior_segments_up_case4)
        let (s1, segs1, bigs1) = mk(vec![
            Stroke { start_price: 110.0, end_price: 120.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 120.0, end_price: 112.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 112.0, end_price: 111.0, start_bar: 6, end_bar: 8, is_up: false },
            Stroke { start_price: 111.0, end_price: 116.0, start_bar: 9, end_bar: 11, is_up: true },
            Stroke { start_price: 116.0, end_price: 119.0, start_bar: 12, end_bar: 14, is_up: true },
        ]);
        let big1 = process_big_segments(&s1, &segs1);
        let sup1 = process_superior_segments(&s1, &segs1, &bigs1);
        assert_eq!(sup1, big1, "场景1(向上Case4) 两路径不一致: sup1={:?} big1={:?}", sup1, big1);

        // 场景2: 非首段向下 Case4 (B 单段含 上+下 对, 同 test_superior_segments_down_case4)
        let (s2, segs2, bigs2) = mk(vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 110.0, end_price: 115.0, start_bar: 3, end_bar: 5, is_up: true },
            Stroke { start_price: 115.0, end_price: 105.0, start_bar: 6, end_bar: 8, is_up: false },
            Stroke { start_price: 105.0, end_price: 113.0, start_bar: 9, end_bar: 11, is_up: true },
            Stroke { start_price: 113.0, end_price: 114.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 114.0, end_price: 107.0, start_bar: 15, end_bar: 17, is_up: false },
            Stroke { start_price: 107.0, end_price: 106.0, start_bar: 18, end_bar: 20, is_up: false },
        ]);
        let big2 = process_big_segments(&s2, &segs2);
        let sup2 = process_superior_segments(&s2, &segs2, &bigs2);
        assert_eq!(sup2, big2, "场景2(向下Case4) 两路径不一致: sup2={:?} big2={:?}", sup2, big2);

        // 场景3: 首段向下 (大段层同构首段兜底后, 两路径输出完全一致)
        let (s3, segs3, bigs3) = mk(vec![
            Stroke { start_price: 120.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: false },
            Stroke { start_price: 110.0, end_price: 118.0, start_bar: 3, end_bar: 5, is_up: true },
            Stroke { start_price: 118.0, end_price: 119.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 119.0, end_price: 112.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 112.0, end_price: 111.0, start_bar: 12, end_bar: 14, is_up: false },
        ]);
        let big3 = process_big_segments(&s3, &segs3);
        let sup3 = process_superior_segments(&s3, &segs3, &bigs3);
        assert_eq!(sup3, big3, "场景3(首段向下兜底) 两路径不一致: sup3={:?} big3={:?}", sup3, big3);
        assert_eq!(sup3, vec![(0usize, 0usize, false), (1usize, 2usize, true), (3usize, 4usize, false)], "sup3={:?}", sup3);
    }
    // ===== 笔Case4 (直接映射线段Case4) =====
    fn fx(price: f64, is_top: bool, idx: usize) -> Fractal {
        Fractal { price, is_top, bar_index: idx, merged_index: idx, time: idx }
    }

    #[test]
    fn test_stroke_first_reverse_initial_data_entry() {
        // (初始数据入口·链头前置反向笔): 窗口起点滑动区首个反向
        // (顶/底) 分型满足分型+笔条件 (C1) 即可成为第一笔起点 — 无需其前存在可成笔的
        // 前导分型. 模拟: 起点底分型与首个顶分型相邻过近不成笔,
        // 同向底序列一路新低, 其后的顶分型才成笔.
        // 旧行为: 链 = [低底, 顶, ...] (第一笔向上, 下跌段不画);
        // 新行为: 链头前置 (顶→低底) 向下第一笔, 后续链逐位不变.
        let v = vec![
            fx(2028.51, false, 3),  // 窗口起点底分型 (滑动区, 被同向替换吸收)
            fx(2031.22, true, 4),   // 首个顶分型 → 新链起点
            fx(2027.30, false, 6),
            fx(2029.91, true, 9),
            fx(2019.78, false, 17),
            fx(2024.72, true, 19),
            fx(2019.66, false, 20), // 同向底极值 (原链起点)
            fx(2027.46, true, 34),  // 原第一笔终点
        ];
        let out = process_strokes_fractals(&v, true, 4);
        assert_eq!(out.len(), 3, "out={:?}", out);
        assert!(out[0].is_top && out[0].bar_index == 4, "链头应为首个顶分型: out={:?}", out);
        assert!(!out[1].is_top && out[1].bar_index == 20, "第二点 = 同向底极值: out={:?}", out);
        assert!(out[2].is_top && out[2].bar_index == 34, "后续链不变: out={:?}", out);
        // 第一笔方向 = 向下 (顶4→底20)
        let strokes = build_strokes(&out);
        assert_eq!(strokes.len(), 2);
        assert!(!strokes[0].is_up && strokes[0].start_bar == 4 && strokes[0].end_bar == 20, "s0={:?}", strokes[0]);
        assert!(strokes[1].is_up && strokes[1].start_bar == 20 && strokes[1].end_bar == 34, "s1={:?}", strokes[1]);
    }

    #[test]
    fn test_stroke_first_reverse_no_insert_when_chain_starts_immediately() {
        // 安全回退: 首分型直接成笔 (滑动区为空 → 无反向极值候选) → 链头不插入, 输出零变化
        let v = vec![
            fx(90.0, false, 0), fx(100.0, true, 5), fx(95.0, false, 9),
        ];
        let out = process_strokes_fractals(&v, true, 4);
        assert_eq!(out.len(), 3, "out={:?}", out);
        assert!(!out[0].is_top && out[0].bar_index == 0);
        assert!(out[1].is_top && out[1].bar_index == 5);
        assert!(!out[2].is_top && out[2].bar_index == 9);
    }

    #[test]
    fn test_stroke_case4_up_basic() {
        // 向上候选笔 B0(90)→T1(100) (位于索引1): 其后 (T1→B1)→(T2→B2) 下行连续新低收口于
        // B2(93<94), 再 (B2→T3)→(B3→T4) 上行连续新高收口于 T4(99>98); 两笔整体 (93..99)
        // 包含于 (90,100] (不破90 不超100) → 向上Case4 成立
        let v = vec![
            fx(90.0, false, 0), fx(100.0, true, 1), fx(94.0, false, 2), fx(97.0, true, 3),
            fx(93.0, false, 4), fx(98.0, true, 5), fx(94.0, false, 6), fx(99.0, true, 7),
        ];
        assert!(check_up_stroke_case4(&v, 1, 90.0, 100.0));
        // 方向不匹配: 同序列走向下Case4 (顶起点/底终点参数反了) → 不成立
        assert!(!check_down_stroke_case4(&v, 1, 90.0, 100.0));
    }

    #[test]
    fn test_stroke_case4_up_rejected() {
        // 否定1: 后两笔突破 A 终点 (T4=101 > 100) → 不包含 → 拒绝
        let v_hi = vec![
            fx(90.0, false, 0), fx(100.0, true, 1), fx(94.0, false, 2), fx(97.0, true, 3),
            fx(93.0, false, 4), fx(98.0, true, 5), fx(94.0, false, 6), fx(101.0, true, 7),
        ];
        assert!(!check_up_stroke_case4(&v_hi, 1, 90.0, 100.0));
        // 否定2: 后两笔跌破 A 起点 (B2=88 < 90) → 不包含 → 拒绝
        let v_lo = vec![
            fx(90.0, false, 0), fx(100.0, true, 1), fx(94.0, false, 2), fx(97.0, true, 3),
            fx(88.0, false, 4), fx(98.0, true, 5), fx(94.0, false, 6), fx(99.0, true, 7),
        ];
        assert!(!check_up_stroke_case4(&v_lo, 1, 90.0, 100.0));
    }

    #[test]
    fn test_stroke_case4_down_basic() {
        // 向下候选笔 T0(110)→B1(100) (位于索引1): 其后 (B1→T2)→(B2→T3)→(B3→T4) 上行连续
        // 新高收口于 T4(108>106), 再 (T4→B4)→(T5→B5) 下行连续新低收口于 B5(100.5<101.5);
        // 两笔整体 (100.5..108) 包含于 [100,110) (不破110 不下100) → 向下Case4 成立
        let v = vec![
            fx(110.0, true, 0), fx(100.0, false, 1), fx(107.0, true, 2), fx(103.0, false, 3),
            fx(106.0, true, 4), fx(101.0, false, 5), fx(108.0, true, 6), fx(101.5, false, 7),
            fx(107.0, true, 8), fx(100.5, false, 9),
        ];
        assert!(check_down_stroke_case4(&v, 1, 110.0, 100.0));
    }

    #[test]
    fn test_stroke_case4_down_rejected() {
        // 否定1: 反弹突破 A 起点 (T4=110.5 ≥ 110) → 拒绝 (镜像线段向下Case4: 严格 <)
        let v_hi = vec![
            fx(110.0, true, 0), fx(100.0, false, 1), fx(107.0, true, 2), fx(103.0, false, 3),
            fx(106.0, true, 4), fx(101.0, false, 5), fx(110.5, true, 6), fx(101.5, false, 7),
            fx(107.0, true, 8), fx(100.5, false, 9),
        ];
        assert!(!check_down_stroke_case4(&v_hi, 1, 110.0, 100.0));
        // 否定2: 下行跌破 A 终点 (B5=98.5 < 100) → 拒绝 (镜像: < 即拒)
        let v_lo = vec![
            fx(110.0, true, 0), fx(100.0, false, 1), fx(107.0, true, 2), fx(103.0, false, 3),
            fx(106.0, true, 4), fx(101.0, false, 5), fx(108.0, true, 6), fx(99.0, false, 7),
            fx(107.0, true, 8), fx(98.5, false, 9),
        ];
        assert!(!check_down_stroke_case4(&v_lo, 1, 110.0, 100.0));
    }

    #[test]
    fn test_stroke_case4_no_close_no_fire() {
        // 无收口结构: 下行无连续新低 (B2=96 未低于 B1=94, 且后续无更低底) → 无完整两笔 → 不升格
        // (与线段Case4 需成对 Case2 段同构: 仅相邻对不足以作证)
        let v = vec![
            fx(90.0, false, 0), fx(100.0, true, 1), fx(94.0, false, 2), fx(97.0, true, 3),
            fx(96.0, false, 4), fx(98.0, true, 5),
        ];
        assert!(!check_up_stroke_case4(&v, 1, 90.0, 100.0));
    }

    #[test]
    fn test_stroke_case4_process_up_promotion() {
        // 端到端: 候选 (B0→T1) barDiff=1 不满足C1; C2 棘轮下移收口于更劣终点 T3(98<100,
        // F1 需前前顶被突破, 首笔无前顶不触发) → C4 事后升格以 T1 落笔; 其后链自然衔接:
        // T1→B2(93) C2收口 → B2→T4(99) C2收口 → 最终 [B0(90), T1(100), B2(93), T4(99)]
        let v = vec![
            fx(90.0, false, 0), fx(100.0, true, 1), fx(94.0, false, 2), fx(97.0, true, 3),
            fx(93.0, false, 4), fx(98.0, true, 5), fx(94.0, false, 6), fx(99.0, true, 7),
            fx(95.0, false, 8),
        ];
        let ff = process_strokes_fractals(&v, true, 4);
        let got: Vec<(f64, bool)> = ff.iter().map(|x| (x.price, x.is_top)).collect();
        assert_eq!(got, vec![(90.0, false), (100.0, true), (93.0, false), (99.0, true)], "ff={:?}", got);
    }

    #[test]
    fn test_stroke_case4_process_down_promotion() {
        // 端到端向下镜像: 候选 (T0→B1) C2 棘轮上移收口于更劣终点 B3(101>100) → C4 事后升格
        // 以 B1 落笔; 其后链: B1→T4(108) C2收口 → T4→B5(100.5) C2收口 → 最终
        // [T0(110), B1(100), T4(108), B5(100.5)]
        let v = vec![
            fx(110.0, true, 0), fx(100.0, false, 1), fx(107.0, true, 2), fx(103.0, false, 3),
            fx(106.0, true, 4), fx(101.0, false, 5), fx(108.0, true, 6), fx(101.5, false, 7),
            fx(107.0, true, 8), fx(100.5, false, 9),
        ];
        let ff = process_strokes_fractals(&v, true, 4);
        let got: Vec<(f64, bool)> = ff.iter().map(|x| (x.price, x.is_top)).collect();
        assert_eq!(got, vec![(110.0, true), (100.0, false), (108.0, true), (100.5, false)], "ff={:?}", got);
    }

    #[test]
    fn test_stroke_case4_param_off_keeps_old_chain() {
        // 参数化回归: case4 关闭 → 与历史行为一致 (C2 收于 T3(98), T4(99) 顶替换 → [B0,T4]),
        // 即笔Case4 升格前旧链 (90→99 单笔, 100 高点被吞)
        let v = vec![
            fx(90.0, false, 0), fx(100.0, true, 1), fx(94.0, false, 2), fx(97.0, true, 3),
            fx(93.0, false, 4), fx(98.0, true, 5), fx(94.0, false, 6), fx(99.0, true, 7),
            fx(95.0, false, 8),
        ];
        let ff = process_strokes_fractals_with_cases(&v, true, false, 4);
        let got: Vec<(f64, bool)> = ff.iter().map(|x| (x.price, x.is_top)).collect();
        assert_eq!(got, vec![(90.0, false), (99.0, true)], "ff={:?}", got);
    }

    #[test]
    fn test_stroke_case3_param_off() {
        // 参数化: case3 关闭 → C3 强势突破不触发. 序列: B0(90)→T1(96) C1成笔(barDiff=5),
        // B1(88) 破前底 90 但仅当 case3 开启才 C3 落笔
        let v = vec![
            fx(90.0, false, 0), fx(96.0, true, 5), fx(88.0, false, 6),
        ];
        // merged_index=idx 时 C1 的 mergedIndexDiff=1 不满足 → 必须拉开 merged 间距
        let v2: Vec<Fractal> = v.iter().enumerate()
            .map(|(k, f)| Fractal { price: f.price, is_top: f.is_top, bar_index: f.bar_index, merged_index: k * 2, time: f.time })
            .collect();
        let on = process_strokes_fractals_with_cases(&v2, true, true, 4);
        let got_on: Vec<f64> = on.iter().map(|x| x.price).collect();
        assert_eq!(got_on, vec![90.0, 96.0, 88.0], "on={:?}", got_on);
        let off = process_strokes_fractals_with_cases(&v2, false, true, 4);
        let got_off: Vec<f64> = off.iter().map(|x| x.price).collect();
        assert_eq!(got_off, vec![90.0, 96.0], "off={:?}", got_off);
    }

    #[test]
    fn test_stroke_cases_param_mapping() {
        // 参数值 0/3/4/5 → 开关映射; 旧值 34 兼容; 未知值按全开
        assert_eq!(StrokeCases::from_param(0), StrokeCases { case3: true, case4: true });
        assert_eq!(StrokeCases::from_param(3), StrokeCases { case3: false, case4: true });
        assert_eq!(StrokeCases::from_param(4), StrokeCases { case3: true, case4: false });
        assert_eq!(StrokeCases::from_param(5), StrokeCases { case3: false, case4: false });
        assert_eq!(StrokeCases::from_param(34), StrokeCases { case3: false, case4: false });
        assert_eq!(StrokeCases::from_param(99), StrokeCases::default());
    }

    #[test]
    fn test_levels_case4_off_segment() {
        // 统一 levels 关 case4: 单笔A(100→120) 含后续(下+上)段对 → 全开时
        // C4 单笔段 [0,0] 以最早终点胜出; 关 case4 → 走 C2 延伸段 [0,6] (历史行为)
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 120.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 120.0, end_price: 112.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 112.0, end_price: 116.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 116.0, end_price: 110.0, start_bar: 9, end_bar: 11, is_up: false },
            Stroke { start_price: 110.0, end_price: 115.0, start_bar: 12, end_bar: 14, is_up: true },
            Stroke { start_price: 115.0, end_price: 113.0, start_bar: 15, end_bar: 17, is_up: false },
            Stroke { start_price: 113.0, end_price: 119.0, start_bar: 18, end_bar: 20, is_up: true },
        ];
        let on = process_segments_with_cases(&strokes, true, true);
        assert_eq!(on[0], (0usize, 0usize, true), "on={:?}", on);
        let off = process_segments_with_cases(&strokes, true, false);
        assert_eq!(off[0], (0usize, 6usize, true), "off={:?}", off);
    }

    #[test]
    fn test_levels_case3_off_segment() {
        // 统一 levels 关 case3: down(112→98) 跌破前 up C2 段起点 100 → 全开时
        // C3 一笔当线段 [3,3]; 关 case3 → 无 C3 → 段链止于 up C2 [0,2]
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 2, is_up: true },
            Stroke { start_price: 110.0, end_price: 105.0, start_bar: 3, end_bar: 5, is_up: false },
            Stroke { start_price: 105.0, end_price: 112.0, start_bar: 6, end_bar: 8, is_up: true },
            Stroke { start_price: 112.0, end_price: 98.0, start_bar: 9, end_bar: 11, is_up: false },
        ];
        let on = process_segments_with_cases(&strokes, true, true);
        assert_eq!(on, vec![(0usize, 2usize, true), (3usize, 3usize, false)], "on={:?}", on);
        let off = process_segments_with_cases(&strokes, false, true);
        assert_eq!(off, vec![(0usize, 2usize, true)], "off={:?}", off);
    }

}
