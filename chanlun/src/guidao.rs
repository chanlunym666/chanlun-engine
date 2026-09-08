//! 笔轨道 (Stroke Band) + 线段轨道 (Segment Band) + 大段轨道 (Big Segment Band)
//! 自应用层下沉至基础库, feature = "guidao" 控制编译.
//! 全部三级轨道 (笔+线段+大段) Case1-4 + 追踪.

use crate::{Fractal, MergedCandle, Stroke, process_merged_candles, resolve_sup_end_point, SecondMarker};
use crate::zhongshu::BigSegThirdMarker;

/// A single band point (上轨 or 下轨).
#[derive(Debug, Clone)]
pub struct BandPoint {
    pub value: f64,
    pub bar_index: usize,
}

/// Minimal stroke representation for track-following.
#[derive(Debug, Clone, Copy)]
pub struct StrokeForTracking {
    pub high: f64,
    pub low: f64,
    pub bar_index: usize,
}

/// Turn signal detected from segment Case2 analysis.
#[derive(Debug, Clone)]
pub struct TurnSignal {
    /// First stroke index of the Case2 segment (the stroke that triggered Case2).
    pub stroke_idx: usize,
    /// Last stroke index of the Case2 segment.
    pub end_stroke_idx: usize,
    /// True = 转空 (up segment start broken downward), False = 转多 (down segment start broken upward).
    pub is_turn_short: bool,
    /// Case3 触发笔 j (经一笔当线段升格的笔). V 判定的唯一锚点.
    pub trigger_stroke_idx: usize,
    /// Max price over segment stroke range.
    pub a_high: f64,
    /// Min price over segment stroke range.
    pub a_low: f64,
}

// ===== Case1/Case2: 顶底背离 + 1.618扩张 =====

/// Case1 upper band (上轨): triple-top divergence on up-strokes.
pub fn calc_upper_band_case1(
    final_fractals: &[Fractal],
) -> (Vec<BandPoint>, Vec<StrokeForTracking>) {
    let total = final_fractals.len();
    if total < 3 {
        return (vec![], vec![]);
    }

    let mut upward_strokes: Vec<StrokeForTracking> = Vec::new();
    for i in 1..total {
        let prev = &final_fractals[i - 1];
        let curr = &final_fractals[i];
        if !prev.is_top && curr.is_top {
            upward_strokes.push(StrokeForTracking {
                high: curr.price,
                low: prev.price,
                bar_index: curr.bar_index,
            });
        }
    }

    let up_count = upward_strokes.len();
    if up_count < 3 {
        return (vec![], vec![]);
    }

    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..up_count {
        let first_high = upward_strokes[i - 2].high;
        let second_high = upward_strokes[i - 1].high;
        let third_high = upward_strokes[i].high;

        if third_high < first_high && second_high < first_high {
            band_points.push(BandPoint {
                value: first_high,
                bar_index: upward_strokes[i - 2].bar_index,
            });
        }
    }

    (band_points, upward_strokes)
}

/// Case2 upper band (上轨): 1.618 expansion from down-strokes.
pub fn calc_upper_band_case2(
    final_fractals: &[Fractal],
) -> Vec<BandPoint> {
    let total = final_fractals.len();
    if total < 3 {
        return vec![];
    }

    let mut down_strokes: Vec<StrokeForTracking> = Vec::new();
    for i in 1..total {
        let prev = &final_fractals[i - 1];
        let curr = &final_fractals[i];
        if prev.is_top && !curr.is_top {
            down_strokes.push(StrokeForTracking {
                high: prev.price,
                low: curr.price,
                bar_index: prev.bar_index,
            });
        }
    }

    let down_count = down_strokes.len();
    if down_count < 2 {
        return vec![];
    }

    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 1..down_count {
        let first_high = down_strokes[i - 1].high;
        let first_low = down_strokes[i - 1].low;
        let second_high = down_strokes[i].high;
        let second_low = down_strokes[i].low;

        let first_space = first_high - first_low;
        let second_space = second_high - second_low;

        if second_space >= first_space * 1.618 && first_high > second_high {
            band_points.push(BandPoint {
                value: first_high,
                bar_index: down_strokes[i - 1].bar_index,
            });
        }
    }

    band_points
}

/// Case1 lower band (下轨): triple-bottom divergence on down-strokes.
pub fn calc_lower_band_case1(
    final_fractals: &[Fractal],
) -> (Vec<BandPoint>, Vec<StrokeForTracking>) {
    let total = final_fractals.len();
    if total < 3 {
        return (vec![], vec![]);
    }

    let mut downward_strokes: Vec<StrokeForTracking> = Vec::new();
    for i in 1..total {
        let prev = &final_fractals[i - 1];
        let curr = &final_fractals[i];
        if prev.is_top && !curr.is_top {
            downward_strokes.push(StrokeForTracking {
                high: prev.price,
                low: curr.price,
                bar_index: curr.bar_index,
            });
        }
    }

    let down_count = downward_strokes.len();
    if down_count < 3 {
        return (vec![], vec![]);
    }

    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..down_count {
        let first_low = downward_strokes[i - 2].low;
        let second_low = downward_strokes[i - 1].low;
        let third_low = downward_strokes[i].low;

        if third_low > first_low && second_low > first_low {
            band_points.push(BandPoint {
                value: first_low,
                bar_index: downward_strokes[i - 2].bar_index,
            });
        }
    }

    (band_points, downward_strokes)
}

/// Case2 lower band (下轨): 1.618 expansion from up-strokes.
pub fn calc_lower_band_case2(
    final_fractals: &[Fractal],
) -> Vec<BandPoint> {
    let total = final_fractals.len();
    if total < 3 {
        return vec![];
    }

    let mut up_strokes2: Vec<StrokeForTracking> = Vec::new();
    for i in 1..total {
        let prev = &final_fractals[i - 1];
        let curr = &final_fractals[i];
        if !prev.is_top && curr.is_top {
            up_strokes2.push(StrokeForTracking {
                high: curr.price,
                low: prev.price,
                bar_index: curr.bar_index,
            });
        }
    }

    let up_count = up_strokes2.len();
    if up_count < 2 {
        return vec![];
    }

    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 1..up_count {
        let first_high = up_strokes2[i - 1].high;
        let first_low = up_strokes2[i - 1].low;
        let second_high = up_strokes2[i].high;
        let second_low = up_strokes2[i].low;

        let first_space = first_high - first_low;
        let second_space = second_high - second_low;

        if second_space >= first_space * 1.618 && first_low < second_low {
            band_points.push(BandPoint {
                value: first_low,
                bar_index: up_strokes2[i - 1].bar_index,
            });
        }
    }

    band_points
}

// ===== 追踪 =====

/// Track-following: scan between band points for strokes that break through.
pub fn apply_tracking(
    band_points: &[BandPoint],
    strokes: &[StrokeForTracking],
    is_upper: bool,
) -> Vec<BandPoint> {
    if band_points.is_empty() || strokes.is_empty() {
        return band_points.to_vec();
    }

    let mut sorted: Vec<BandPoint> = band_points.to_vec();
    sorted.sort_by_key(|p| p.bar_index);

    let mut final_points: Vec<BandPoint> = Vec::new();

    for idx in 0..sorted.len() {
        let mut current_value = sorted[idx].value;
        let mut current_time = sorted[idx].bar_index;

        let next_cond_time = if idx + 1 < sorted.len() {
            sorted[idx + 1].bar_index
        } else {
            usize::MAX
        };

        final_points.push(BandPoint {
            value: current_value,
            bar_index: current_time,
        });

        loop {
            let mut found = false;
            for stroke in strokes {
                let st = stroke.bar_index;
                let sv = if is_upper { stroke.high } else { stroke.low };

                if st > current_time && st < next_cond_time {
                    if is_upper && sv > current_value {
                        current_value = sv;
                        current_time = st;
                        final_points.push(BandPoint {
                            value: current_value,
                            bar_index: current_time,
                        });
                        found = true;
                        break;
                    } else if !is_upper && sv < current_value {
                        current_value = sv;
                        current_time = st;
                        final_points.push(BandPoint {
                            value: current_value,
                            bar_index: current_time,
                        });
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                break;
            }
        }
    }

    // Post-tracking dedup (保留原始逻辑, 仅添加诊断日志)
    final_points.sort_by_key(|p| p.bar_index);

    let mut deduped: Vec<BandPoint> = Vec::new();
    if !final_points.is_empty() {
        deduped.push(final_points[0].clone());
        for i in 1..final_points.len() {
            if final_points[i].value == final_points[i - 1].value {
                continue;
            }
            let last = deduped.last().unwrap();
            if final_points[i].bar_index == last.bar_index {
                if is_upper && final_points[i].value > last.value {
                    log::warn!(
                        "[轨道去重-BUG] 上轨 bar={} 同bar跳过高值: last={:.4} cur={:.4} (保留了低值!)",
                        last.bar_index, last.value, final_points[i].value
                    );
                    continue;
                }
                if !is_upper && final_points[i].value < last.value {
                    log::warn!(
                        "[轨道去重-BUG] 下轨 bar={} 同bar跳过最低值: last={:.4} cur={:.4} (保留了高值!)",
                        last.bar_index, last.value, final_points[i].value
                    );
                    continue;
                }
            }
            deduped.push(final_points[i].clone());
        }
    }

    deduped
}

// ===== Case3: 转多/转空 =====

/// Detect turn signals (转多/转空) from segment Case2 analysis.
pub fn detect_turn_signals(
    strokes: &[Stroke],
    segments: &[(usize, usize, bool)],
    _highs: &[f64],
    _lows: &[f64],
) -> Vec<TurnSignal> {
    // 对齐线段 Case3 (状态机一笔当线段, lib.rs collect_segment_case3_events):
    // 转空 = 向下笔经 Case3 升格为向下线段 (触发笔终点价 < 回溯基准向上笔起点价),
    // 转多 = 向上笔经 Case3 升格为向上线段 (对称).
    // 基准笔 = 回溯最近的 Case2-ok 笔或 C3 段起点笔, 不必是线段起点
    // (基准笔既可为线段起点, 也可为段内部笔).
    // 守卫: 基准的 C2 段须完整结束于触发笔开始之前, 否则无事件 → 无转空.
    let events = crate::collect_segment_case3_events(strokes);
    let mut turn_signals: Vec<TurnSignal> = Vec::new();

    for (j, base, end_i, is_up) in events {
        if base >= strokes.len() || end_i >= strokes.len() { continue; }
        // 唯一性: e = 状态机终态"包含 base 的段"终点 (含端点延伸, 与线段状态机唯一对齐);
        // 兜底重放 end_i (base 的 C2 段终点). 方向一致性 + e < j 防御.
        let e = segments
            .iter()
            .find(|&&(s, t, dir)| s <= base && base <= t && dir == strokes[base].is_up && t < j)
            .map(|&(_, t, _)| t)
            .unwrap_or(end_i);
        let e = e.min(strokes.len() - 1);
        let mut a_high = f64::NEG_INFINITY;
        let mut a_low = f64::INFINITY;
        for k in base..=e {
            let s = &strokes[k];
            a_high = a_high.max(s.start_price).max(s.end_price);
            a_low = a_low.min(s.start_price).min(s.end_price);
        }
        turn_signals.push(TurnSignal {
            stroke_idx: base,
            end_stroke_idx: e,
            is_turn_short: !is_up, // 向下段 C3 建立 → 转空; 向上段 C3 建立 → 转多
            trigger_stroke_idx: j,
            a_high,
            a_low,
        });
    }

    turn_signals
}

/// Detect V-signals from turn signals (反弹确认).
/// 唯一性判定 (与"一笔当线段"Case3 升格同构, 零独立比价):
/// 转空信号 (触发笔 j) 之后, 状态机紧邻升格 j+1 为反向线段 (j+1 是下一个 Case3 事件
/// 且方向相反) ⟺ V多; 转多信号 j 之后 j+1 紧邻升格为向下线段 ⟺ V空.
/// 实例: 转多 j 后 j+1 未反向升格 (其后一路向上) → 无 V空.
pub fn detect_v_signals(
    turn_signals: &[TurnSignal],
    strokes: &[Stroke],
    _highs: &[f64],
    _lows: &[f64],
) -> Vec<TurnSignal> {
    let stroke_count = strokes.len();
    if turn_signals.is_empty() || stroke_count < 2 {
        return vec![];
    }

    // 触发笔 j → 事件方向 映射 (j+1 紧邻反向升格判定用)
    let trigger_map: std::collections::HashMap<usize, bool> = turn_signals
        .iter()
        .map(|ts| (ts.trigger_stroke_idx, ts.is_turn_short))
        .collect();

    let mut v_signals: Vec<TurnSignal> = Vec::new();
    for ts in turn_signals {
        let j = ts.trigger_stroke_idx;
        if j + 1 >= stroke_count {
            continue;
        }
        match trigger_map.get(&(j + 1)) {
            Some(&next_short) if next_short != ts.is_turn_short => {
                // j+1 紧邻反向升格 → V 成立 (V空: 转多后反破; V多: 转空后反破)
                let mut v = ts.clone();
                v.stroke_idx = j;
                v.end_stroke_idx = j + 1;
                v_signals.push(v);
            }
            _ => {}
        }
    }

    v_signals
}

// ===== Case3/Case4 band point extractors =====

/// Case3 upper band (上轨): from 转空 turn signals.
/// 点 = 基准段终点价 @ 终点时间 (值与时间同点, 状态机唯一).
pub fn calc_upper_band_case3(
    turn_signals: &[TurnSignal],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    for ts in turn_signals {
        if ts.is_turn_short {
            if let Some(s) = strokes.get(ts.end_stroke_idx) {
                band_points.push(BandPoint {
                    value: s.end_price,
                    bar_index: s.end_bar,
                });
            }
        }
    }
    band_points
}

/// Case3 lower band (下轨): from 转多 turn signals.
/// 点 = 基准段终点价 @ 终点时间 (值与时间同点, 状态机唯一).
pub fn calc_lower_band_case3(
    turn_signals: &[TurnSignal],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    for ts in turn_signals {
        if !ts.is_turn_short {
            if let Some(s) = strokes.get(ts.end_stroke_idx) {
                band_points.push(BandPoint {
                    value: s.end_price,
                    bar_index: s.end_bar,
                });
            }
        }
    }
    band_points
}

/// Case4 upper band (上轨): from V空 signals (转多 → V空).
/// 点 = 转多触发笔 j 终点 (V形顶点, 状态机唯一锚点).
pub fn calc_upper_band_case4(
    v_signals: &[TurnSignal],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    if v_signals.is_empty() {
        return band_points;
    }
    for vs in v_signals {
        if !vs.is_turn_short {
            let first_idx = vs.trigger_stroke_idx;
            if let Some(s) = strokes.get(first_idx) {
                if s.is_up {
                    band_points.push(BandPoint {
                        value: s.end_price,
                        bar_index: s.end_bar,
                    });
                }
            }
        }
    }
    band_points
}

/// Case4 lower band (下轨): from V多 signals (转空 → V多).
/// 点 = 转空触发笔 j 终点 (V形底点, 状态机唯一锚点).
pub fn calc_lower_band_case4(
    v_signals: &[TurnSignal],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    if v_signals.is_empty() {
        return band_points;
    }
    for vs in v_signals {
        if vs.is_turn_short {
            let first_idx = vs.trigger_stroke_idx;
            if let Some(s) = strokes.get(first_idx) {
                if !s.is_up {
                    band_points.push(BandPoint {
                        value: s.end_price,
                        bar_index: s.end_bar,
                    });
                }
            }
        }
    }
    band_points
}

// ===== 入口: 笔轨道 =====

/// 轨道级通用: 抬升/下压点落点重定位到分型确认 bar (笔/线段/大段三级轨道).
///
/// tracking 抬升点 bar_index = 轨点(分型高点) bar, 延伸线在当根接顶 -> 突破检测自指失效
/// (边界案例: 抬升点与确认 bar 同价时不抬升).
/// 改为分型独立算法的确认 bar: identify_fractals 右往左扫描
/// (right=merged[i] / mid=merged[i+1] / left=merged[i+2]), Fractal.merged_index = i+1 = mid 序号,
/// 分型在 left (最晚合并K线) 出现时确认 -> confirm_bar = merged[merged_index+1] 的
/// high_bar_index (顶分型/上轨) 或 low_bar_index (底分型/下轨)
/// (含包含关系处理, 确认偏移 1..12 不等, 非机械 +2).
/// 效果: 延伸线在分型确认前不接顶, 突破检测正常触发; 分型成立 bar 处 K线极值必然
/// 低于/高于轨值 (mid.high > left.high / mid.low < left.low), 天然不穿越, 无需额外机制.
fn relocate_band_lift_points(
    band: Vec<BandPoint>,
    raw: &[BandPoint],
    merged: &[MergedCandle],
    fractals: &[Fractal],
    is_upper: bool,
) -> Vec<BandPoint> {
    let raw_bars: std::collections::HashSet<usize> = raw.iter().map(|p| p.bar_index).collect();
    // 分型 -> 分型确认 bar (上轨只查顶分型, 下轨只查底分型)
    let mut confirm: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for f in fractals {
        if f.is_top == is_upper && f.merged_index + 1 < merged.len() {
            let cb = if is_upper {
                merged[f.merged_index + 1].high_bar_index
            } else {
                merged[f.merged_index + 1].low_bar_index
            };
            confirm.insert(f.bar_index, cb);
        }
    }
    let mut out: Vec<BandPoint> = Vec::with_capacity(band.len());
    for p in band {
        if raw_bars.contains(&p.bar_index) {
            out.push(p); // 原始轨点不动
        } else if let Some(&cb) = confirm.get(&p.bar_index) {
            out.push(BandPoint { value: p.value, bar_index: cb }); // 抬升/下压点 -> 分型确认 bar
        } else {
            out.push(p); // 无分型映射, 保底不动
        }
    }
    out.sort_by_key(|p| p.bar_index);
    // 同 bar 保留极值 (上轨高值 / 下轨低值)
    let mut deduped: Vec<BandPoint> = Vec::with_capacity(out.len());
    for p in out {
        if let Some(last) = deduped.last_mut() {
            if last.bar_index == p.bar_index {
                if (is_upper && p.value > last.value) || (!is_upper && p.value < last.value) {
                    *last = p;
                }
                continue;
            }
        }
        deduped.push(p);
    }
    deduped
}

/// Compute stroke-level bands (笔轨道) — Case1/2/3/4 + tracking.
/// Returns (upper_band, lower_band).
pub fn compute_stroke_bands(
    final_fractals: &[Fractal],
    strokes: &[Stroke],
    segments: &[(usize, usize, bool)],
    highs: &[f64],
    lows: &[f64],
) -> (Vec<BandPoint>, Vec<BandPoint>) {
    // Case1 + Case2
    let (upper_c1, up_strokes) = calc_upper_band_case1(final_fractals);
    let upper_c2 = calc_upper_band_case2(final_fractals);
    let (lower_c1, down_strokes) = calc_lower_band_case1(final_fractals);
    let lower_c2 = calc_lower_band_case2(final_fractals);

    // Case3
    let turn_signals = detect_turn_signals(strokes, segments, highs, lows);
    let upper_c3 = calc_upper_band_case3(&turn_signals, strokes);
    let lower_c3 = calc_lower_band_case3(&turn_signals, strokes);

    // Case4
    let v_signals = detect_v_signals(&turn_signals, strokes, highs, lows);
    let upper_c4 = calc_upper_band_case4(&v_signals, strokes);
    let lower_c4 = calc_lower_band_case4(&v_signals, strokes);

    // Merge + track
    let mut upper_raw = upper_c1;
    upper_raw.extend(upper_c2);
    upper_raw.extend(upper_c3);
    upper_raw.extend(upper_c4);
    upper_raw.sort_by_key(|p| p.bar_index);
    let upper_band = apply_tracking(&upper_raw, &up_strokes, true);
    // 笔轨道抬升/下压点落点 = 分型确认 bar (延伸线不接顶/不触底语义; 笔上下轨, 线段/大段不镜像)
    let merged = process_merged_candles(highs, lows);
    let upper_band = relocate_band_lift_points(upper_band, &upper_raw, &merged, final_fractals, true);

    let mut lower_raw = lower_c1;
    lower_raw.extend(lower_c2);
    lower_raw.extend(lower_c3);
    lower_raw.extend(lower_c4);
    lower_raw.sort_by_key(|p| p.bar_index);
    let lower_band = apply_tracking(&lower_raw, &down_strokes, false);
    let lower_band = relocate_band_lift_points(lower_band, &lower_raw, &merged, final_fractals, false);

    (upper_band, lower_band)
}

// ===== 线段轨道 (Segment Band) =====

/// Segment-level turn signal: 线段经大段状态机 Case3 升格 (一个线段当大段).
#[derive(Debug, Clone)]
pub struct SegmentTurnSignal {
    pub seg_idx: usize,
    pub end_seg_idx: usize,
    pub is_turn_short: bool,
    pub a_high: f64,
    pub a_low: f64,
    /// Case3 触发线段 j (经一个线段当大段升格的线段). V 判定的唯一锚点.
    pub trigger_seg_idx: usize,
}

/// Segment Case1 upper band: triple-top divergence on up-segments.
pub fn calc_seg_upper_band_case1(
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> (Vec<BandPoint>, Vec<StrokeForTracking>) {
    let seg_count = segments.len();
    if seg_count < 3 { return (vec![], vec![]); }
    let mut up_segs: Vec<StrokeForTracking> = Vec::new();
    for &(start_idx, end_idx, is_up) in segments {
        if is_up {
            up_segs.push(StrokeForTracking {
                high: strokes[end_idx].end_price,
                low: strokes[start_idx].start_price,
                bar_index: strokes[end_idx].end_bar,
            });
        }
    }
    let up_count = up_segs.len();
    if up_count < 3 { return (vec![], up_segs); }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..up_count {
        if up_segs[i].high < up_segs[i - 2].high && up_segs[i - 1].high < up_segs[i - 2].high {
            band_points.push(BandPoint { value: up_segs[i - 2].high, bar_index: up_segs[i - 2].bar_index });
        }
    }
    (band_points, up_segs)
}

/// Segment Case1 lower band: triple-bottom divergence on down-segments.
pub fn calc_seg_lower_band_case1(
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> (Vec<BandPoint>, Vec<StrokeForTracking>) {
    let seg_count = segments.len();
    if seg_count < 3 { return (vec![], vec![]); }
    let mut down_segs: Vec<StrokeForTracking> = Vec::new();
    for &(start_idx, end_idx, is_up) in segments {
        if !is_up {
            down_segs.push(StrokeForTracking {
                high: strokes[start_idx].start_price,
                low: strokes[end_idx].end_price,
                bar_index: strokes[end_idx].end_bar,
            });
        }
    }
    let down_count = down_segs.len();
    if down_count < 3 { return (vec![], down_segs); }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..down_count {
        if down_segs[i].low > down_segs[i - 2].low && down_segs[i - 1].low > down_segs[i - 2].low {
            band_points.push(BandPoint { value: down_segs[i - 2].low, bar_index: down_segs[i - 2].bar_index });
        }
    }
    (band_points, down_segs)
}

/// Segment Case2 upper band: 1.618 expansion from down-segments.
pub fn calc_seg_upper_band_case2(
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let seg_count = segments.len();
    if seg_count < 2 { return vec![]; }
    let mut down_segs: Vec<(f64, f64, usize)> = Vec::new();
    for &(start_idx, end_idx, is_up) in segments {
        if !is_up {
            down_segs.push((strokes[start_idx].start_price, strokes[end_idx].end_price, strokes[start_idx].start_bar));
        }
    }
    if down_segs.len() < 2 { return vec![]; }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 1..down_segs.len() {
        let (fh, fl, fb) = down_segs[i - 1];
        let (sh, sl, _) = down_segs[i];
        if (sh - sl) >= (fh - fl) * 1.618 && fh > sh {
            band_points.push(BandPoint { value: fh, bar_index: fb });
        }
    }
    band_points
}

/// Segment Case2 lower band: 1.618 expansion from up-segments.
pub fn calc_seg_lower_band_case2(
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let seg_count = segments.len();
    if seg_count < 2 { return vec![]; }
    let mut up_segs: Vec<(f64, f64, usize)> = Vec::new();
    for &(start_idx, end_idx, is_up) in segments {
        if is_up {
            up_segs.push((strokes[end_idx].end_price, strokes[start_idx].start_price, strokes[end_idx].end_bar));
        }
    }
    if up_segs.len() < 2 { return vec![]; }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 1..up_segs.len() {
        let (fh, fl, fb) = up_segs[i - 1];
        let (sh, sl, _) = up_segs[i];
        if (sh - sl) >= (fh - fl) * 1.618 && fl < sl {
            band_points.push(BandPoint { value: fl, bar_index: fb });
        }
    }
    band_points
}

/// Detect segment-level turn signals (转多/转空) from big_segment Case3 analysis.
/// 对齐大段 Case3 (状态机"一个线段当大段", lib.rs collect_segment_case3_events):
/// 转空 = 向下线段经 Case3 升格为向下大段 (触发线段终点价 < 回溯基准向上线段起点价),
/// 转多 = 向上线段经 Case3 升格为向上大段 (对称).
/// 投影与 process_big_segments 完全一致 → 大段 Case3 事件零漂移复用.
/// 基准线段 = 回溯最近的 Case2-ok 线段或 C3 段起点线段, 不必是大段起点.
pub fn detect_segment_turn_signals(
    segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    _highs: &[f64],
    _lows: &[f64],
) -> Vec<SegmentTurnSignal> {
    let n_segments = segments.len();
    if n_segments < 2 { return vec![]; }

    let projected: Vec<Stroke> = segments.iter().map(|(si, ei, is_up)| Stroke {
        is_up: *is_up, start_price: strokes[*si].start_price,
        end_price: strokes[*ei].end_price, start_bar: 0, end_bar: 0,
    }).collect();
    let events = crate::collect_segment_case3_events(&projected);
    let mut turn_signals: Vec<SegmentTurnSignal> = Vec::new();

    for (j, base, end_i, is_up) in events {
        if base >= n_segments || end_i >= n_segments { continue; }
        // 唯一性: e = 大段状态机终态"包含 base 的段"终点 (含端点延伸, 与大段状态机唯一对齐);
        // 兜底重放 end_i (base 的 C2 段终点). 方向一致性 + e < j 防御.
        let e = big_segments
            .iter()
            .find(|&&(s, t, dir)| s <= base && base <= t && dir == segments[base].2 && t < j)
            .map(|&(_, t, _)| t)
            .unwrap_or(end_i);
        let e = e.min(n_segments - 1);
        let mut a_high = f64::NEG_INFINITY;
        let mut a_low = f64::INFINITY;
        for k in base..=e {
            let (si, ei, _) = segments[k];
            let sp = strokes[si].start_price;
            let ep = strokes[ei].end_price;
            a_high = a_high.max(sp).max(ep);
            a_low = a_low.min(sp).min(ep);
        }
        turn_signals.push(SegmentTurnSignal {
            seg_idx: base,
            end_seg_idx: e,
            is_turn_short: !is_up, // 向下段 C3 建立 → 转空; 向上段 C3 建立 → 转多
            a_high,
            a_low,
            trigger_seg_idx: j,
        });
    }

    turn_signals
}

/// Detect segment-level V-signals from segment turn signals (反弹确认).
/// 唯一性判定 (与"一个线段当大段"Case3 升格同构, 零独立比价):
/// 转空信号 (触发段 j) 之后, 大段状态机紧邻升格 j+1 为反向大段 (j+1 是下一个
/// 大段 Case3 事件且方向相反) ⟺ V多; 转多信号 j 之后 j+1 紧邻向下升格 ⟺ V空.
pub fn detect_segment_v_signals(
    seg_turn_signals: &[SegmentTurnSignal],
    _segments: &[(usize, usize, bool)],
    _strokes: &[Stroke],
    _highs: &[f64],
    _lows: &[f64],
) -> Vec<SegmentTurnSignal> {
    if seg_turn_signals.is_empty() { return vec![]; }

    // 触发段 j → 转信号方向 映射 (j+1 紧邻反向升格判定用)
    let trigger_map: std::collections::HashMap<usize, bool> = seg_turn_signals
        .iter()
        .map(|ts| (ts.trigger_seg_idx, ts.is_turn_short))
        .collect();

    let mut v_signals: Vec<SegmentTurnSignal> = Vec::new();
    for ts in seg_turn_signals {
        let j = ts.trigger_seg_idx;
        match trigger_map.get(&(j + 1)) {
            Some(&next_short) if next_short != ts.is_turn_short => {
                // j+1 紧邻反向升格 → V 成立 (V空: 转多后反破; V多: 转空后反破)
                let mut v = ts.clone();
                v.seg_idx = j;
                v.end_seg_idx = j + 1;
                v_signals.push(v);
            }
            _ => {}
        }
    }

    v_signals
}

/// Segment Case3 upper band: from segment 转空 signals.
/// 点 = 大段状态机终态段终点价 @ 终点时间 (值与时间同点, 状态机唯一).
pub fn calc_seg_upper_band_case3(
    seg_turn_signals: &[SegmentTurnSignal],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    for ts in seg_turn_signals {
        if ts.is_turn_short {
            let end_idx = ts.end_seg_idx;
            if end_idx < segments.len() {
                let (_, ei, _) = segments[end_idx];
                if let Some(s) = strokes.get(ei) {
                    band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                }
            }
        }
    }
    band_points
}

/// Segment Case3 lower band: from segment 转多 signals.
/// 点 = 大段状态机终态段终点价 @ 终点时间 (值与时间同点, 状态机唯一).
pub fn calc_seg_lower_band_case3(
    seg_turn_signals: &[SegmentTurnSignal],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    for ts in seg_turn_signals {
        if !ts.is_turn_short {
            let end_idx = ts.end_seg_idx;
            if end_idx < segments.len() {
                let (_, ei, _) = segments[end_idx];
                if let Some(s) = strokes.get(ei) {
                    band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                }
            }
        }
    }
    band_points
}

/// Segment Case4 upper band: from segment V空 signals (转多 → V空).
/// 点 = 转多触发段 j 终点 (V形顶点, 状态机唯一锚点).
pub fn calc_seg_upper_band_case4(
    seg_v_signals: &[SegmentTurnSignal],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    if seg_v_signals.is_empty() { return band_points; }
    for vs in seg_v_signals {
        if !vs.is_turn_short {
            let j = vs.trigger_seg_idx;
            if j < segments.len() && segments[j].2 {
                let (_, ei, _) = segments[j];
                if let Some(s) = strokes.get(ei) {
                    band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                }
            }
        }
    }
    band_points
}

/// Segment Case4 lower band: from segment V多 signals (转空 → V多).
/// 点 = 转空触发段 j 终点 (V形底点, 状态机唯一锚点).
pub fn calc_seg_lower_band_case4(
    seg_v_signals: &[SegmentTurnSignal],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    if seg_v_signals.is_empty() { return band_points; }
    for vs in seg_v_signals {
        if vs.is_turn_short {
            let j = vs.trigger_seg_idx;
            if j < segments.len() && !segments[j].2 {
                let (_, ei, _) = segments[j];
                if let Some(s) = strokes.get(ei) {
                    band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                }
            }
        }
    }
    band_points
}

// ===== 入口: 线段轨道 =====

/// Compute segment-level bands (线段轨道) — Case1/2/3/4 + tracking.
/// Returns (upper_band, lower_band).
pub fn compute_segment_bands(
    segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
    final_fractals: &[Fractal],
) -> (Vec<BandPoint>, Vec<BandPoint>) {
    let (seg_upper_c1, seg_up) = calc_seg_upper_band_case1(segments, strokes);
    let seg_upper_c2 = calc_seg_upper_band_case2(segments, strokes);
    let (seg_lower_c1, seg_down) = calc_seg_lower_band_case1(segments, strokes);
    let seg_lower_c2 = calc_seg_lower_band_case2(segments, strokes);
    let seg_turn_signals = detect_segment_turn_signals(segments, big_segments, strokes, highs, lows);
    let seg_upper_c3 = calc_seg_upper_band_case3(&seg_turn_signals, segments, strokes);
    let seg_lower_c3 = calc_seg_lower_band_case3(&seg_turn_signals, segments, strokes);
    let seg_v_signals = detect_segment_v_signals(&seg_turn_signals, segments, strokes, highs, lows);
    let seg_upper_c4 = calc_seg_upper_band_case4(&seg_v_signals, segments, strokes);
    let seg_lower_c4 = calc_seg_lower_band_case4(&seg_v_signals, segments, strokes);

    let mut upper_raw = seg_upper_c1; upper_raw.extend(seg_upper_c2); upper_raw.extend(seg_upper_c3); upper_raw.extend(seg_upper_c4);
    upper_raw.sort_by_key(|p| p.bar_index);
    let upper_band = apply_tracking(&upper_raw, &seg_up, true);
    let mut lower_raw = seg_lower_c1; lower_raw.extend(seg_lower_c2); lower_raw.extend(seg_lower_c3); lower_raw.extend(seg_lower_c4);
    lower_raw.sort_by_key(|p| p.bar_index);
    let lower_band = apply_tracking(&lower_raw, &seg_down, false);
    // 线段轨道抬升/下压点落点 = 分型确认 bar (镜像笔轨道)
    let merged = process_merged_candles(highs, lows);
    let upper_band = relocate_band_lift_points(upper_band, &upper_raw, &merged, final_fractals, true);
    let lower_band = relocate_band_lift_points(lower_band, &lower_raw, &merged, final_fractals, false);
    (upper_band, lower_band)
}

// ===== 大段轨道 (Big Segment Band) =====

/// Big-segment turn signal (大段转多/转空).
#[derive(Debug, Clone)]
pub struct BigSegmentTurnSignal {
    pub seg_idx: usize,
    pub end_seg_idx: usize,
    pub is_turn_short: bool,
    pub a_high: f64,
    pub a_low: f64,
    /// Case3 触发大段 j (经一个大段当高级段升格的大段). V 判定的唯一锚点.
    pub trigger_seg_idx: usize,
}

/// Big Segment Case1 upper band: triple-top divergence on up big_segments.
pub fn calc_bigseg_upper_band_case1(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> (Vec<BandPoint>, Vec<StrokeForTracking>) {
    let bs_count = big_segments.len();
    if bs_count < 3 { return (vec![], vec![]); }
    let mut up_big: Vec<StrokeForTracking> = Vec::new();
    for &(start_seg, end_seg, is_up) in big_segments {
        if is_up {
            if let (Some(&(_, end_stroke, _)), Some(&(start_stroke_idx, _, _))) =
                (segments.get(end_seg), segments.get(start_seg))
            {
                if let (Some(end_s), Some(start_s)) = (strokes.get(end_stroke), strokes.get(start_stroke_idx)) {
                    up_big.push(StrokeForTracking { high: end_s.end_price, low: start_s.start_price, bar_index: end_s.end_bar });
                }
            }
        }
    }
    let up_count = up_big.len();
    if up_count < 3 { return (vec![], up_big); }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..up_count {
        if up_big[i].high < up_big[i - 2].high && up_big[i - 1].high < up_big[i - 2].high {
            band_points.push(BandPoint { value: up_big[i - 2].high, bar_index: up_big[i - 2].bar_index });
        }
    }
    (band_points, up_big)
}

/// Big Segment Case1 lower band: triple-bottom divergence on down big_segments.
pub fn calc_bigseg_lower_band_case1(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> (Vec<BandPoint>, Vec<StrokeForTracking>) {
    let bs_count = big_segments.len();
    if bs_count < 3 { return (vec![], vec![]); }
    let mut down_big: Vec<StrokeForTracking> = Vec::new();
    for &(start_seg, end_seg, is_up) in big_segments {
        if !is_up {
            if let (Some(&(_, end_stroke, _)), Some(&(start_stroke_idx, _, _))) =
                (segments.get(end_seg), segments.get(start_seg))
            {
                if let (Some(end_s), Some(start_s)) = (strokes.get(end_stroke), strokes.get(start_stroke_idx)) {
                    down_big.push(StrokeForTracking { high: start_s.start_price, low: end_s.end_price, bar_index: end_s.end_bar });
                }
            }
        }
    }
    let down_count = down_big.len();
    if down_count < 3 { return (vec![], down_big); }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..down_count {
        if down_big[i].low > down_big[i - 2].low && down_big[i - 1].low > down_big[i - 2].low {
            band_points.push(BandPoint { value: down_big[i - 2].low, bar_index: down_big[i - 2].bar_index });
        }
    }
    (band_points, down_big)
}

/// Big Segment Case2 upper band: 1.618 expansion from down big_segments.
pub fn calc_bigseg_upper_band_case2(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let bs_count = big_segments.len();
    if bs_count < 2 { return vec![]; }
    let mut down_big: Vec<(f64, f64, usize)> = Vec::new();
    for &(start_seg, end_seg, is_up) in big_segments {
        if !is_up {
            if let (Some(&(start_stroke_idx, _, _)), Some(&(_, end_stroke, _))) =
                (segments.get(start_seg), segments.get(end_seg))
            {
                if let (Some(start_s), Some(end_s)) = (strokes.get(start_stroke_idx), strokes.get(end_stroke)) {
                    down_big.push((start_s.start_price, end_s.end_price, start_s.start_bar));
                }
            }
        }
    }
    let down_count = down_big.len();
    if down_count < 2 { return vec![]; }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 1..down_count {
        let (first_high, first_low, first_bar) = down_big[i - 1];
        let (second_high, second_low, _) = down_big[i];
        let first_space = first_high - first_low;
        let second_space = second_high - second_low;
        if second_space >= first_space * 1.618 && first_high > second_high {
            band_points.push(BandPoint { value: first_high, bar_index: first_bar });
        }
    }
    band_points
}

/// Big Segment Case2 lower band: 1.618 expansion from up big_segments.
pub fn calc_bigseg_lower_band_case2(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let bs_count = big_segments.len();
    if bs_count < 2 { return vec![]; }
    let mut up_big: Vec<(f64, f64, usize)> = Vec::new();
    for &(start_seg, end_seg, is_up) in big_segments {
        if is_up {
            if let (Some(&(start_stroke_idx, _, _)), Some(&(_, end_stroke, _))) =
                (segments.get(start_seg), segments.get(end_seg))
            {
                if let (Some(start_s), Some(end_s)) = (strokes.get(start_stroke_idx), strokes.get(end_stroke)) {
                    up_big.push((end_s.end_price, start_s.start_price, end_s.end_bar));
                }
            }
        }
    }
    let up_count = up_big.len();
    if up_count < 2 { return vec![]; }
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 1..up_count {
        let (first_high, first_low, first_bar) = up_big[i - 1];
        let (second_high, second_low, _) = up_big[i];
        let first_space = first_high - first_low;
        let second_space = second_high - second_low;
        if second_space >= first_space * 1.618 && first_low < second_low {
            band_points.push(BandPoint { value: first_low, bar_index: first_bar });
        }
    }
    band_points
}

/// Detect big-segment turn signals (大段转多/转空) from superior_segment Case3 analysis.
/// 对齐高级段 Case3 (状态机"一个大段当高级段", lib.rs collect_superior_case3_events):
/// 转空 = 向下大段经 Case3 升格为向下高级段 (触发大段终点价 < 回溯基准向上大段起点价),
/// 转多 = 向上大段经 Case3 升格为向上高级段 (对称).
/// 重放函数与 process_superior_segments 完全对齐 (含首段向下兜底分支) → 事件零漂移.
/// 基准大段 = 回溯最近的 Case2-ok 大段或 C3 段起点大段, 不必是高级段起点.
pub fn detect_big_seg_turn_signals(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_segments: &[(usize, usize, bool)],
) -> Vec<BigSegmentTurnSignal> {
    let n_bigs = big_segments.len();
    if n_bigs < 2 { return vec![]; }

    let events = crate::collect_superior_case3_events(strokes, segments, big_segments);
    let mut turn_signals: Vec<BigSegmentTurnSignal> = Vec::new();

    for (j, base, end_i, is_up) in events {
        if base >= n_bigs || end_i >= n_bigs { continue; }
        // 唯一性: e = 高级段状态机终态"包含 base 的段"终点 (含端点延伸, 与高级段状态机唯一对齐);
        // 兜底重放 end_i (base 的 C2 段终点). 方向一致性 + e < j 防御.
        let e = superior_segments
            .iter()
            .find(|&&(s, t, dir)| s <= base && base <= t && dir == big_segments[base].2 && t < j)
            .map(|&(_, t, _)| t)
            .unwrap_or(end_i);
        let e = e.min(n_bigs - 1);
        let mut a_high = f64::NEG_INFINITY;
        let mut a_low = f64::INFINITY;
        for k in base..=e {
            let (bs, be, _) = big_segments[k];
            let sp = strokes[segments[bs].0].start_price;
            let ep = strokes[segments[be].1].end_price;
            a_high = a_high.max(sp).max(ep);
            a_low = a_low.min(sp).min(ep);
        }
        turn_signals.push(BigSegmentTurnSignal {
            seg_idx: base,
            end_seg_idx: e,
            is_turn_short: !is_up, // 向下段 C3 建立 → 转空; 向上段 C3 建立 → 转多
            a_high,
            a_low,
            trigger_seg_idx: j,
        });
    }

    turn_signals
}

/// Big Segment Case3 upper band: from 转空 signals.
/// 点 = 高级段状态机终态大段 e 终点价 @ 终点时间 (值与时间同点, 状态机唯一).
pub fn calc_bigseg_upper_band_case3(
    turn_signals: &[BigSegmentTurnSignal],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    for ts in turn_signals {
        if ts.is_turn_short {
            let end_idx = ts.end_seg_idx;
            if end_idx < big_segments.len() {
                let (_, e_seg, _) = big_segments[end_idx];
                if e_seg < segments.len() {
                    let (_, e_stroke, _) = segments[e_seg];
                    if let Some(s) = strokes.get(e_stroke) {
                        band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                    }
                }
            }
        }
    }
    band_points
}

/// Big Segment Case3 lower band: from 转多 signals.
/// 点 = 高级段状态机终态大段 e 终点价 @ 终点时间 (值与时间同点, 状态机唯一).
pub fn calc_bigseg_lower_band_case3(
    turn_signals: &[BigSegmentTurnSignal],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    for ts in turn_signals {
        if !ts.is_turn_short {
            let end_idx = ts.end_seg_idx;
            if end_idx < big_segments.len() {
                let (_, e_seg, _) = big_segments[end_idx];
                if e_seg < segments.len() {
                    let (_, e_stroke, _) = segments[e_seg];
                    if let Some(s) = strokes.get(e_stroke) {
                        band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                    }
                }
            }
        }
    }
    band_points
}

/// Detect big-segment V-signals from turn signals (反弹确认).
/// 唯一性判定 (与"一个大段当高级段"Case3 升格同构, 零独立比价):
/// 转空信号 (触发大段 j) 之后, 高级段状态机紧邻升格 j+1 为反向高级段
/// (j+1 是下一个高级段 Case3 事件且方向相反) ⟺ V多; 转多信号 j 之后 j+1 向下升格 ⟺ V空.
pub fn detect_big_v_signals(
    big_turn_signals: &[BigSegmentTurnSignal],
) -> Vec<BigSegmentTurnSignal> {
    if big_turn_signals.is_empty() { return vec![]; }

    // 触发大段 j → 转信号方向 映射 (j+1 紧邻反向升格判定用)
    let trigger_map: std::collections::HashMap<usize, bool> = big_turn_signals
        .iter()
        .map(|ts| (ts.trigger_seg_idx, ts.is_turn_short))
        .collect();

    let mut v_signals: Vec<BigSegmentTurnSignal> = Vec::new();
    for ts in big_turn_signals {
        let j = ts.trigger_seg_idx;
        match trigger_map.get(&(j + 1)) {
            Some(&next_short) if next_short != ts.is_turn_short => {
                // j+1 紧邻反向升格 → V 成立 (V空: 转多后反破; V多: 转空后反破)
                let mut v = ts.clone();
                v.seg_idx = j;
                v.end_seg_idx = j + 1;
                v_signals.push(v);
            }
            _ => {}
        }
    }

    v_signals
}

/// Big Segment Case4 upper band: from V空 signals (转多 → V空).
/// 点 = 转多触发大段 j 终点 (V形顶点, 状态机唯一锚点).
pub fn calc_bigseg_upper_band_case4(
    big_v_signals: &[BigSegmentTurnSignal],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    if big_v_signals.is_empty() { return band_points; }
    for vs in big_v_signals {
        if !vs.is_turn_short {
            let j = vs.trigger_seg_idx;
            if j < big_segments.len() && big_segments[j].2 {
                let (_, e_seg, _) = big_segments[j];
                if e_seg < segments.len() {
                    let (_, e_stroke, _) = segments[e_seg];
                    if let Some(s) = strokes.get(e_stroke) {
                        band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                    }
                }
            }
        }
    }
    band_points
}

/// Big Segment Case4 lower band: from V多 signals (转空 → V多).
/// 点 = 转空触发大段 j 终点 (V形底点, 状态机唯一锚点).
pub fn calc_bigseg_lower_band_case4(
    big_v_signals: &[BigSegmentTurnSignal],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BandPoint> {
    let mut band_points: Vec<BandPoint> = Vec::new();
    if big_v_signals.is_empty() { return band_points; }
    for vs in big_v_signals {
        if vs.is_turn_short {
            let j = vs.trigger_seg_idx;
            if j < big_segments.len() && !big_segments[j].2 {
                let (_, e_seg, _) = big_segments[j];
                if e_seg < segments.len() {
                    let (_, e_stroke, _) = segments[e_seg];
                    if let Some(s) = strokes.get(e_stroke) {
                        band_points.push(BandPoint { value: s.end_price, bar_index: s.end_bar });
                    }
                }
            }
        }
    }
    band_points
}

// ===== 入口: 大段轨道 =====

/// Compute big-segment bands (大段轨道) — Case1-4 + tracking.
/// Returns (upper_tracked, lower_tracked, upper_raw, lower_raw).
pub fn compute_bigseg_bands(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_segments: &[(usize, usize, bool)],
    final_fractals: &[Fractal],
    highs: &[f64],
    lows: &[f64],
) -> (Vec<BandPoint>, Vec<BandPoint>, Vec<BandPoint>, Vec<BandPoint>) {
    let (bigseg_upper_c1, bigseg_up) = calc_bigseg_upper_band_case1(big_segments, segments, strokes);
    let bigseg_upper_c2 = calc_bigseg_upper_band_case2(big_segments, segments, strokes);
    let (bigseg_lower_c1, bigseg_down) = calc_bigseg_lower_band_case1(big_segments, segments, strokes);
    let bigseg_lower_c2 = calc_bigseg_lower_band_case2(big_segments, segments, strokes);
    let bigseg_turn_signals = detect_big_seg_turn_signals(big_segments, segments, strokes, superior_segments);
    let bigseg_upper_c3 = calc_bigseg_upper_band_case3(&bigseg_turn_signals, big_segments, segments, strokes);
    let bigseg_lower_c3 = calc_bigseg_lower_band_case3(&bigseg_turn_signals, big_segments, segments, strokes);
    let big_v_signals = detect_big_v_signals(&bigseg_turn_signals);
    let bigseg_upper_c4 = calc_bigseg_upper_band_case4(&big_v_signals, big_segments, segments, strokes);
    let bigseg_lower_c4 = calc_bigseg_lower_band_case4(&big_v_signals, big_segments, segments, strokes);
    let mut upper_raw = bigseg_upper_c1; upper_raw.extend(bigseg_upper_c2); upper_raw.extend(bigseg_upper_c3); upper_raw.extend(bigseg_upper_c4);
    upper_raw.sort_by_key(|p| p.bar_index);
    let upper_band = apply_tracking(&upper_raw, &bigseg_up, true);
    let mut lower_raw = bigseg_lower_c1; lower_raw.extend(bigseg_lower_c2); lower_raw.extend(bigseg_lower_c3); lower_raw.extend(bigseg_lower_c4);
    lower_raw.sort_by_key(|p| p.bar_index);
    let lower_band = apply_tracking(&lower_raw, &bigseg_down, false);
    // 大段轨道抬升/下压点落点 = 分型确认 bar (镜像笔/线段轨道)
    let merged = process_merged_candles(highs, lows);
    let upper_band = relocate_band_lift_points(upper_band, &upper_raw, &merged, final_fractals, true);
    let lower_band = relocate_band_lift_points(lower_band, &lower_raw, &merged, final_fractals, false);
    (upper_band, lower_band, upper_raw, lower_raw)
}

// ===== 轨道突破/跌破检测 =====

/// Find the third downward segment bar_index for a given Case1 BandPoint.
pub fn find_case1_third_seg_bar(bp: &BandPoint, seg_down: &[StrokeForTracking]) -> usize {
    for i in 2..seg_down.len() {
        let first_low = seg_down[i - 2].low;
        let second_low = seg_down[i - 1].low;
        let third_low = seg_down[i].low;
        if third_low > first_low && second_low > first_low {
            if seg_down[i - 2].bar_index == bp.bar_index && (seg_down[i - 2].low - bp.value).abs() < 1e-10 {
                return seg_down[i].bar_index;
            }
        }
    }
    usize::MAX
}

/// Find the third upward segment bar_index for a given Case1 BandPoint.
pub fn find_case1_third_up_seg_bar(bp: &BandPoint, seg_up: &[StrokeForTracking]) -> usize {
    for i in 2..seg_up.len() {
        let first_high = seg_up[i - 2].high;
        let second_high = seg_up[i - 1].high;
        let third_high = seg_up[i].high;
        if third_high < first_high && second_high < first_high {
            if seg_up[i - 2].bar_index == bp.bar_index && (seg_up[i - 2].high - bp.value).abs() < 1e-10 {
                return seg_up[i].bar_index;
            }
        }
    }
    usize::MAX
}

/// A band break marker: 大段轨道被突破/跌破的箭头标记.
#[derive(Debug, Clone, PartialEq)]
pub struct SecondPlusOneMarker {
    pub bar_index: usize,
    pub price: f64,
    /// true=2+N买, false=2+N卖
    pub is_buy: bool,
    /// 序号: 1=2+1, 2=2+2, 3=2+3 ...
    pub order: u32,
}

/// 2+N买: 遍历向下高级段终点, 定位其后首个大段下轨Case1三底背离第三底 F,
/// C = 高级段终点后首个向上大段终点价(high), C > F → 在 F 标记 2+1买 (order=1);
/// 之后向下大段终点 X 满足 min(U1,U2) > X > 高级段终点价且新向下高级段未成立 → 2+2, 2+3, ...
pub fn detect_bigseg_cf_buy_markers(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    bigseg_lower_c1: &[BandPoint],
    down_big: &[StrokeForTracking],
    up_big: &[StrokeForTracking],
) -> Vec<SecondPlusOneMarker> {
    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_big, is_up)) in sups.iter().enumerate() {
        if is_up { continue; } // 只处理向下高级段
        let (dbe, d_price) = match resolve_sup_end_point(end_big, bigs, segs, strokes) {
            Some(v) => v,
            None => continue,
        };
        let first_c1 = match bigseg_lower_c1.iter().find(|bp| bp.bar_index >= dbe) {
            Some(c) => c,
            None => continue,
        };
        // [窗口守卫] 轨点须落在 [高级段终点, 下一个同向(向下)高级段终点) 窗口内;
        // 跨段共享轨点 → 相同标记重复产出 (7614 类), 跳过该高级段
        let next_down_sup_guard = sups[k + 1..].iter().find(|s| !s.2)
            .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
        if let Some((nb, _)) = next_down_sup_guard {
            if first_c1.bar_index >= nb { continue; }
        }
        let third_bar = find_case1_third_seg_bar(first_c1, down_big);
        if third_bar == usize::MAX { continue; }
        let f_price = match down_big.iter().find(|s| s.bar_index == third_bar) {
            Some(s) => s.low,
            None => continue,
        };
        let c_price = match up_big.iter().find(|s| s.bar_index > dbe) {
            Some(s) => s.high,
            None => continue,
        };
        if c_price > f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
            // 递推: 参考高点 = min(U1, U2) 固定;
            // 新向下高级段终点 bar 为界; 满足 min(U1,U2) > X > 高级段终点价 → 依次标记 2+2, 2+3, ...
            let second_up = up_big.iter().filter(|s| s.bar_index > dbe).nth(1);
            let ref_high = second_up.map(|s| c_price.min(s.high));
            let next_down_sup_end = sups[k + 1..].iter().find(|s| !s.2)
                .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
            if let Some(rh) = ref_high {
                let mut order = 2;
                for x in down_big.iter().filter(|s| s.bar_index > third_bar) {
                    if let Some((b, _)) = next_down_sup_end {
                        if x.bar_index >= b { break; }
                    }
                    if rh > x.low && x.low > d_price {
                        markers.push(SecondPlusOneMarker { bar_index: x.bar_index, price: x.low, is_buy: true, order });
                        order += 1;
                    }
                }
            }
        }
    }
    markers
}

/// 2+N卖: 完全镜像 (高级段终点 = 向上高级段终点; F = 大段上轨Case1三顶背离第三顶;
/// C = 高级段终点后首个向下大段终点价(low), C < F → 在 F 标记 2+1卖 (order=1);
/// 之后向上大段终点 X 满足 max(C',C'+1) < X < 高级段终点价且新向上高级段未成立 → 2+2, 2+3, ...
pub fn detect_bigseg_cf_sell_markers(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    bigseg_upper_c1: &[BandPoint],
    up_big: &[StrokeForTracking],
    down_big: &[StrokeForTracking],
) -> Vec<SecondPlusOneMarker> {
    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_big, is_up)) in sups.iter().enumerate() {
        if !is_up { continue; } // 只处理向上高级段
        let (ube, d_price) = match resolve_sup_end_point(end_big, bigs, segs, strokes) {
            Some(v) => v,
            None => continue,
        };
        let first_c1 = match bigseg_upper_c1.iter().find(|bp| bp.bar_index >= ube) {
            Some(c) => c,
            None => continue,
        };
        // [窗口守卫] 轨点须落在 [高级段终点, 下一个同向(向上)高级段终点) 窗口内;
        // 跨段共享轨点 → 相同标记重复产出 (7614 类), 跳过该高级段
        let next_up_sup_guard = sups[k + 1..].iter().find(|s| s.2)
            .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
        if let Some((nb, _)) = next_up_sup_guard {
            if first_c1.bar_index >= nb { continue; }
        }
        let third_bar = find_case1_third_up_seg_bar(first_c1, up_big);
        if third_bar == usize::MAX { continue; }
        let f_price = match up_big.iter().find(|s| s.bar_index == third_bar) {
            Some(s) => s.high,
            None => continue,
        };
        let c_price = match down_big.iter().find(|s| s.bar_index > ube) {
            Some(s) => s.low,
            None => continue,
        };
        if c_price < f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
            // 递推 (镜像): 参考低点 = max(C', C'+1) 固定;
            // 新向上高级段终点 bar 为界; 满足 max(C',C'+1) < X < 高级段终点价 → 依次标记 2+2, 2+3, ...
            let second_down = down_big.iter().filter(|s| s.bar_index > ube).nth(1);
            let ref_low = second_down.map(|s| c_price.max(s.low));
            let next_up_sup_end = sups[k + 1..].iter().find(|s| s.2)
                .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
            if let Some(rl) = ref_low {
                let mut order = 2;
                for x in up_big.iter().filter(|s| s.bar_index > third_bar) {
                    if let Some((b, _)) = next_up_sup_end {
                        if x.bar_index >= b { break; }
                    }
                    if rl < x.high && x.high < d_price {
                        markers.push(SecondPlusOneMarker { bar_index: x.bar_index, price: x.high, is_buy: false, order });
                        order += 1;
                    }
                }
            }
        }
    }
    markers
}

/// 买/卖点撤回过滤器结果
pub struct RetreatFilterResult {
    pub second_markers: Vec<SecondMarker>,
    pub third_markers: Vec<BigSegThirdMarker>,
    pub plus_markers: Vec<SecondPlusOneMarker>,
}

/// 高级段区间 [起点bar, 终点bar] (首大段首线段首笔起点 → 末大段末线段末笔终点)
fn sup_range(
    sup: (usize, usize, bool),
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, usize)> {
    let (sb, eb, _) = sup;
    let big = *bigs.get(sb)?;
    let seg = *segs.get(big.0)?;
    let st = strokes.get(seg.0)?;
    let start_bar = st.start_bar;
    let end_bar = resolve_sup_end_point(eb, bigs, segs, strokes)?.0;
    Some((start_bar, end_bar))
}

/// 买/卖点撤回过滤器 ("∨"结构):
/// 每个向下高级段成立 → 所有买点(二买/2+N买/三买)撤回到其前驱向上高级段内
/// 最后一个买点 (bar 最大; X 及之前的保留, 之后的删除; 区间内无买点 → 撤回范围内全删);
/// 每个向上高级段成立 → 卖点完全镜像 (撤回到前驱向下高级段内最后一个卖点).
/// ⚠ 事件时序: 撤回只删除 bar ≤ 触发高级段自身终点bar 的标记, 其后区间内的标记
/// (成立后才产生) 保留 — 避免误删最新结构内买点.
/// ⚠ 逐事件: 所有成立事件按序应用 (非只最后一个);
/// 区间内无买点时仅删除前驱区间终点之后、触发段自身终点之前的标记,
/// 更早区间的标记不受本次撤回影响.
/// ⚠ cut 恒 = 触发高级段自身终点bar: 事件时序 — 只撤回
/// 该段成立时已存在的标记; 其后反向段未成立且未创新低阶段内的标记成立
/// 更晚, 不撤回. 删除 "最后高级段 → cut=MAX" 特例
/// (该特例会把未确认阶段内二买/2+1买全量误删).
pub fn apply_sup_retreat_filter(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    mut second_markers: Vec<SecondMarker>,
    mut third_markers: Vec<BigSegThirdMarker>,
    mut plus_markers: Vec<SecondPlusOneMarker>,
) -> RetreatFilterResult {
    // 买点撤回: 每个向下高级段成立 (索引>0 才有前驱向上高级段) 都触发撤回
    for d_idx in (1..sups.len()).filter(|&i| !sups[i].2) {
        if let Some((sb, eb)) = sup_range(sups[d_idx - 1], bigs, segs, strokes) {
            let mut buy_bars: Vec<usize> = Vec::new();
            for m in second_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in third_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in plus_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            // cut: 触发高级段自身终点bar — 事件时序, 只撤回该段成立时已存在的买点;
            // 其后区间 (未确认阶段) 的买点成立更晚, 不撤回
            let cut = sup_range(sups[d_idx], bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match buy_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => {
                    second_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    third_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    plus_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                }
                None => {
                    // 前驱区间内无买点 → 撤回范围内 (前驱区间终点之后..cut) 全删, 更早区间保留
                    second_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    third_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    plus_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                }
            }
        }
    }
    // 卖点镜像: 每个向上高级段成立 (索引>0) → 撤回到前驱向下高级段区间内最后卖点
    for u_idx in (1..sups.len()).filter(|&i| sups[i].2) {
        if let Some((sb, eb)) = sup_range(sups[u_idx - 1], bigs, segs, strokes) {
            let mut sell_bars: Vec<usize> = Vec::new();
            for m in second_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in third_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in plus_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            let cut = sup_range(sups[u_idx], bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match sell_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => {
                    second_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    third_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    plus_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                }
                None => {
                    // 前驱区间内无卖点 → 撤回范围内全删, 更早区间保留 (镜像)
                    second_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    third_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    plus_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                }
            }
        }
    }
    RetreatFilterResult { second_markers, third_markers, plus_markers }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bigseg_cf_buy_marker_trigger() {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false }, // 向下高级段终点
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 10, end_bar: 16, is_up: true }, // C=108
            Stroke { start_price: 108.0, end_price: 98.0, start_bar: 16, end_bar: 22, is_up: false },  // 第1底
            Stroke { start_price: 98.0, end_price: 106.0, start_bar: 22, end_bar: 28, is_up: true },
            Stroke { start_price: 106.0, end_price: 101.0, start_bar: 28, end_bar: 34, is_up: false }, // 第2底
            Stroke { start_price: 101.0, end_price: 104.0, start_bar: 34, end_bar: 40, is_up: true },
            Stroke { start_price: 104.0, end_price: 103.0, start_bar: 40, end_bar: 46, is_up: false }, // 第3底 F=103
            Stroke { start_price: 103.0, end_price: 109.0, start_bar: 46, end_bar: 52, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false)]; // 向下高级段, 终点大段=1
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 108.0, low: 98.0, bar_index: 22 },
            StrokeForTracking { high: 106.0, low: 101.0, bar_index: 34 },
            StrokeForTracking { high: 104.0, low: 103.0, bar_index: 46 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 108.0, low: 100.0, bar_index: 16 },
            StrokeForTracking { high: 106.0, low: 98.0, bar_index: 28 },
            StrokeForTracking { high: 104.0, low: 101.0, bar_index: 40 },
            StrokeForTracking { high: 109.0, low: 103.0, bar_index: 52 },
        ];
        let bigseg_lower_c1 = vec![BandPoint { value: 98.0, bar_index: 22 }]; // 三底背离(98→101→103)
        let markers = detect_bigseg_cf_buy_markers(&sups, &bigs, &segs, &strokes, &bigseg_lower_c1, &down_big, &up_big);
        assert_eq!(markers.len(), 1, "markers={:?}", markers);
        assert_eq!(markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
    }

    /// 2+1买: C<=F 不触发 — 高级段终点后首个向上大段终点 C=102 未高于第三底 F=103.
    #[test]
    fn test_bigseg_cf_buy_no_trigger_c_le_f() {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false },
            Stroke { start_price: 100.0, end_price: 102.0, start_bar: 10, end_bar: 16, is_up: true }, // C=102 <= F=103
            Stroke { start_price: 102.0, end_price: 98.0, start_bar: 16, end_bar: 22, is_up: false },
            Stroke { start_price: 98.0, end_price: 106.0, start_bar: 22, end_bar: 28, is_up: true },
            Stroke { start_price: 106.0, end_price: 101.0, start_bar: 28, end_bar: 34, is_up: false },
            Stroke { start_price: 101.0, end_price: 104.0, start_bar: 34, end_bar: 40, is_up: true },
            Stroke { start_price: 104.0, end_price: 103.0, start_bar: 40, end_bar: 46, is_up: false },
            Stroke { start_price: 103.0, end_price: 109.0, start_bar: 46, end_bar: 52, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false)];
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 102.0, low: 98.0, bar_index: 22 },
            StrokeForTracking { high: 106.0, low: 101.0, bar_index: 34 },
            StrokeForTracking { high: 104.0, low: 103.0, bar_index: 46 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 102.0, low: 100.0, bar_index: 16 },
            StrokeForTracking { high: 106.0, low: 98.0, bar_index: 28 },
            StrokeForTracking { high: 104.0, low: 101.0, bar_index: 40 },
            StrokeForTracking { high: 109.0, low: 103.0, bar_index: 52 },
        ];
        let bigseg_lower_c1 = vec![BandPoint { value: 98.0, bar_index: 22 }];
        let markers = detect_bigseg_cf_buy_markers(&sups, &bigs, &segs, &strokes, &bigseg_lower_c1, &down_big, &up_big);
        assert!(markers.is_empty(), "markers={:?}", markers);
    }

    /// 2+1买: 高级段终点后无大段下轨Case1点 → 不触发.
    #[test]
    fn test_bigseg_cf_buy_no_case1_after_detect() {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false },
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 10, end_bar: 16, is_up: true },
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false)];
        let markers = detect_bigseg_cf_buy_markers(&sups, &bigs, &segs, &strokes, &[], &[], &[]);
        assert!(markers.is_empty(), "markers={:?}", markers);
    }

    /// 2+1卖: C'<F' 触发 — 向上高级段终点(bar=10)后首个向下大段终点 C'=102
    /// 低于三顶背离(112→108→107)第三顶 F'=107 → 在 F'(bar=46) 标记 2+1卖.
    #[test]
    fn test_bigseg_cf_sell_marker_trigger() {
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 0, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 5, end_bar: 10, is_up: true }, // 向上高级段终点
            Stroke { start_price: 110.0, end_price: 102.0, start_bar: 10, end_bar: 16, is_up: false }, // C'=102
            Stroke { start_price: 102.0, end_price: 112.0, start_bar: 16, end_bar: 22, is_up: true },  // 第1顶
            Stroke { start_price: 112.0, end_price: 105.0, start_bar: 22, end_bar: 28, is_up: false },
            Stroke { start_price: 105.0, end_price: 108.0, start_bar: 28, end_bar: 34, is_up: true },  // 第2顶
            Stroke { start_price: 108.0, end_price: 106.0, start_bar: 34, end_bar: 40, is_up: false },
            Stroke { start_price: 106.0, end_price: 107.0, start_bar: 40, end_bar: 46, is_up: true },  // 第3顶 F'=107
            Stroke { start_price: 107.0, end_price: 104.0, start_bar: 46, end_bar: 52, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, true)]; // 向上高级段, 终点大段=1
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 110.0, low: 102.0, bar_index: 16 },
            StrokeForTracking { high: 112.0, low: 105.0, bar_index: 28 },
            StrokeForTracking { high: 108.0, low: 106.0, bar_index: 40 },
            StrokeForTracking { high: 107.0, low: 104.0, bar_index: 52 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 112.0, low: 102.0, bar_index: 22 },
            StrokeForTracking { high: 108.0, low: 105.0, bar_index: 34 },
            StrokeForTracking { high: 107.0, low: 106.0, bar_index: 46 },
        ];
        let bigseg_upper_c1 = vec![BandPoint { value: 112.0, bar_index: 22 }]; // 三顶背离(112→108→107)
        let markers = detect_bigseg_cf_sell_markers(&sups, &bigs, &segs, &strokes, &bigseg_upper_c1, &up_big, &down_big);
        assert_eq!(markers.len(), 1, "markers={:?}", markers);
        assert_eq!(markers[0], SecondPlusOneMarker { bar_index: 46, price: 107.0, is_buy: false, order: 1 });
    }

    /// 2+N买 递推: F=103 之后 X1=102(bar=58) 标 2+2买, X2=104(bar=70) 标 2+3买.
    /// 参考高点固定 min(U1,U2)=min(108,105)=105 — U3=104/U4=106 不参与,
    /// 否则累积 min=104 会导致 X2=104 不满足 104>104 而不标记 (此断言 3 个标记即验证固定性).
    #[test]
    fn test_bigseg_cf_buy_markers_2n_sequence() {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false }, // 向下高级段终点 D=100
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 10, end_bar: 16, is_up: true }, // U1 C=108
            Stroke { start_price: 108.0, end_price: 98.0, start_bar: 16, end_bar: 22, is_up: false },  // 第1底
            Stroke { start_price: 98.0, end_price: 105.0, start_bar: 22, end_bar: 28, is_up: true },  // U2=105
            Stroke { start_price: 105.0, end_price: 101.0, start_bar: 28, end_bar: 34, is_up: false }, // 第2底
            Stroke { start_price: 101.0, end_price: 104.0, start_bar: 34, end_bar: 40, is_up: true },  // U3=104 (不参与)
            Stroke { start_price: 104.0, end_price: 103.0, start_bar: 40, end_bar: 46, is_up: false }, // 第3底 F=103
            Stroke { start_price: 103.0, end_price: 106.0, start_bar: 46, end_bar: 52, is_up: true },  // U4=106 (不参与)
            Stroke { start_price: 106.0, end_price: 102.0, start_bar: 52, end_bar: 58, is_up: false }, // X1=102 → 2+2
            Stroke { start_price: 102.0, end_price: 107.0, start_bar: 58, end_bar: 64, is_up: true },
            Stroke { start_price: 107.0, end_price: 104.0, start_bar: 64, end_bar: 70, is_up: false }, // X2=104 → 2+3
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false)]; // 无新向下高级段 → 无边界
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 108.0, low: 98.0, bar_index: 22 },
            StrokeForTracking { high: 105.0, low: 101.0, bar_index: 34 },
            StrokeForTracking { high: 104.0, low: 103.0, bar_index: 46 },
            StrokeForTracking { high: 106.0, low: 102.0, bar_index: 58 },
            StrokeForTracking { high: 107.0, low: 104.0, bar_index: 70 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 108.0, low: 100.0, bar_index: 16 },
            StrokeForTracking { high: 105.0, low: 98.0, bar_index: 28 },
            StrokeForTracking { high: 104.0, low: 101.0, bar_index: 40 },
            StrokeForTracking { high: 106.0, low: 103.0, bar_index: 52 },
            StrokeForTracking { high: 107.0, low: 102.0, bar_index: 64 },
        ];
        let bigseg_lower_c1 = vec![BandPoint { value: 98.0, bar_index: 22 }]; // 三底背离(98→101→103)
        let markers = detect_bigseg_cf_buy_markers(&sups, &bigs, &segs, &strokes, &bigseg_lower_c1, &down_big, &up_big);
        assert_eq!(markers.len(), 3, "markers={:?}", markers);
        assert_eq!(markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
        assert_eq!(markers[1], SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 });
        assert_eq!(markers[2], SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 3 });
    }

    /// 2+N买 终止: 新向下高级段成立 (终点=X2大段 bar=70) → X_bar>=70 停止, 仅 2+1/2+2.
    #[test]
    fn test_bigseg_cf_buy_markers_stop_at_new_down_sup() {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false }, // 高级段终点 D=100
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 10, end_bar: 16, is_up: true }, // U1 C=108
            Stroke { start_price: 108.0, end_price: 98.0, start_bar: 16, end_bar: 22, is_up: false },  // 第1底
            Stroke { start_price: 98.0, end_price: 105.0, start_bar: 22, end_bar: 28, is_up: true },  // U2=105
            Stroke { start_price: 105.0, end_price: 101.0, start_bar: 28, end_bar: 34, is_up: false }, // 第2底
            Stroke { start_price: 101.0, end_price: 104.0, start_bar: 34, end_bar: 40, is_up: true },
            Stroke { start_price: 104.0, end_price: 103.0, start_bar: 40, end_bar: 46, is_up: false }, // 第3底 F=103
            Stroke { start_price: 103.0, end_price: 106.0, start_bar: 46, end_bar: 52, is_up: true },
            Stroke { start_price: 106.0, end_price: 102.0, start_bar: 52, end_bar: 58, is_up: false }, // X1=102 → 2+2
            Stroke { start_price: 102.0, end_price: 107.0, start_bar: 58, end_bar: 64, is_up: true },
            Stroke { start_price: 107.0, end_price: 104.0, start_bar: 64, end_bar: 70, is_up: false }, // X2: 新向下高级段终点
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (11, 11, false)]; // 新向下高级段终点=X2 (bar=70)
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 108.0, low: 98.0, bar_index: 22 },
            StrokeForTracking { high: 105.0, low: 101.0, bar_index: 34 },
            StrokeForTracking { high: 104.0, low: 103.0, bar_index: 46 },
            StrokeForTracking { high: 106.0, low: 102.0, bar_index: 58 },
            StrokeForTracking { high: 107.0, low: 104.0, bar_index: 70 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 108.0, low: 100.0, bar_index: 16 },
            StrokeForTracking { high: 105.0, low: 98.0, bar_index: 28 },
            StrokeForTracking { high: 104.0, low: 101.0, bar_index: 40 },
            StrokeForTracking { high: 106.0, low: 103.0, bar_index: 52 },
            StrokeForTracking { high: 107.0, low: 102.0, bar_index: 64 },
        ];
        let bigseg_lower_c1 = vec![BandPoint { value: 98.0, bar_index: 22 }];
        let markers = detect_bigseg_cf_buy_markers(&sups, &bigs, &segs, &strokes, &bigseg_lower_c1, &down_big, &up_big);
        assert_eq!(markers.len(), 2, "markers={:?}", markers);
        assert_eq!(markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
        assert_eq!(markers[1], SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 });
    }

    /// 2+N买 跳过不消耗编号: X1=99 破高级段终点(D=100) 不满足 → 跳过; X2=104 满足 → 标 2+2买.
    #[test]
    fn test_bigseg_cf_buy_markers_skip_keeps_order() {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false }, // 高级段终点 D=100
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 10, end_bar: 16, is_up: true }, // U1 C=108
            Stroke { start_price: 108.0, end_price: 98.0, start_bar: 16, end_bar: 22, is_up: false },  // 第1底
            Stroke { start_price: 98.0, end_price: 105.0, start_bar: 22, end_bar: 28, is_up: true },  // U2=105
            Stroke { start_price: 105.0, end_price: 101.0, start_bar: 28, end_bar: 34, is_up: false }, // 第2底
            Stroke { start_price: 101.0, end_price: 104.0, start_bar: 34, end_bar: 40, is_up: true },
            Stroke { start_price: 104.0, end_price: 103.0, start_bar: 40, end_bar: 46, is_up: false }, // 第3底 F=103
            Stroke { start_price: 103.0, end_price: 106.0, start_bar: 46, end_bar: 52, is_up: true },
            Stroke { start_price: 106.0, end_price: 99.0, start_bar: 52, end_bar: 58, is_up: false },  // X1=99 破D → 跳过
            Stroke { start_price: 99.0, end_price: 107.0, start_bar: 58, end_bar: 64, is_up: true },
            Stroke { start_price: 107.0, end_price: 104.0, start_bar: 64, end_bar: 70, is_up: false }, // X2=104 → 2+2
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false)];
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 108.0, low: 98.0, bar_index: 22 },
            StrokeForTracking { high: 105.0, low: 101.0, bar_index: 34 },
            StrokeForTracking { high: 104.0, low: 103.0, bar_index: 46 },
            StrokeForTracking { high: 106.0, low: 99.0, bar_index: 58 },
            StrokeForTracking { high: 107.0, low: 104.0, bar_index: 70 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 108.0, low: 100.0, bar_index: 16 },
            StrokeForTracking { high: 105.0, low: 98.0, bar_index: 28 },
            StrokeForTracking { high: 104.0, low: 101.0, bar_index: 40 },
            StrokeForTracking { high: 106.0, low: 103.0, bar_index: 52 },
            StrokeForTracking { high: 107.0, low: 99.0, bar_index: 64 },
        ];
        let bigseg_lower_c1 = vec![BandPoint { value: 98.0, bar_index: 22 }];
        let markers = detect_bigseg_cf_buy_markers(&sups, &bigs, &segs, &strokes, &bigseg_lower_c1, &down_big, &up_big);
        assert_eq!(markers.len(), 2, "markers={:?}", markers);
        assert_eq!(markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
        assert_eq!(markers[1], SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 2 });
    }

    /// 2+N卖 镜像递推: F'=107 之后 X1'=109(bar=58) 标 2+2卖, X2'=106(bar=70) 标 2+3卖.
    /// 参考低点固定 max(C',C'+1)=max(102,105)=105.
    #[test]
    fn test_bigseg_cf_sell_markers_2n_sequence() {
        let strokes = vec![
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 0, end_bar: 5, is_up: false },
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 5, end_bar: 10, is_up: true }, // 高级段终点 D'=110
            Stroke { start_price: 110.0, end_price: 102.0, start_bar: 10, end_bar: 16, is_up: false }, // C'=102
            Stroke { start_price: 102.0, end_price: 112.0, start_bar: 16, end_bar: 22, is_up: true },  // 第1顶
            Stroke { start_price: 112.0, end_price: 105.0, start_bar: 22, end_bar: 28, is_up: false }, // C'+1=105
            Stroke { start_price: 105.0, end_price: 108.0, start_bar: 28, end_bar: 34, is_up: true },  // 第2顶
            Stroke { start_price: 108.0, end_price: 106.0, start_bar: 34, end_bar: 40, is_up: false },
            Stroke { start_price: 106.0, end_price: 107.0, start_bar: 40, end_bar: 46, is_up: true },  // 第3顶 F'=107
            Stroke { start_price: 107.0, end_price: 104.0, start_bar: 46, end_bar: 52, is_up: false },
            Stroke { start_price: 104.0, end_price: 109.0, start_bar: 52, end_bar: 58, is_up: true },  // X1'=109 → 2+2卖
            Stroke { start_price: 109.0, end_price: 103.0, start_bar: 58, end_bar: 64, is_up: false },
            Stroke { start_price: 103.0, end_price: 106.0, start_bar: 64, end_bar: 70, is_up: true },  // X2'=106 → 2+3卖
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, true)];
        let down_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 5 },
            StrokeForTracking { high: 110.0, low: 102.0, bar_index: 16 },
            StrokeForTracking { high: 112.0, low: 105.0, bar_index: 28 },
            StrokeForTracking { high: 108.0, low: 106.0, bar_index: 40 },
            StrokeForTracking { high: 107.0, low: 104.0, bar_index: 52 },
            StrokeForTracking { high: 109.0, low: 103.0, bar_index: 64 },
        ];
        let up_big = vec![
            StrokeForTracking { high: 110.0, low: 100.0, bar_index: 10 },
            StrokeForTracking { high: 112.0, low: 102.0, bar_index: 22 },
            StrokeForTracking { high: 108.0, low: 105.0, bar_index: 34 },
            StrokeForTracking { high: 107.0, low: 106.0, bar_index: 46 },
            StrokeForTracking { high: 109.0, low: 104.0, bar_index: 58 },
            StrokeForTracking { high: 106.0, low: 103.0, bar_index: 70 },
        ];
        let bigseg_upper_c1 = vec![BandPoint { value: 112.0, bar_index: 22 }]; // 三顶背离(112→108→107)
        let markers = detect_bigseg_cf_sell_markers(&sups, &bigs, &segs, &strokes, &bigseg_upper_c1, &up_big, &down_big);
        assert_eq!(markers.len(), 3, "markers={:?}", markers);
        assert_eq!(markers[0], SecondPlusOneMarker { bar_index: 46, price: 107.0, is_buy: false, order: 1 });
        assert_eq!(markers[1], SecondPlusOneMarker { bar_index: 58, price: 109.0, is_buy: false, order: 2 });
        assert_eq!(markers[2], SecondPlusOneMarker { bar_index: 70, price: 106.0, is_buy: false, order: 3 });
    }

    /// 撤回过滤器测试公共数据: 一笔=一线段=一大段=一高级段
    /// sups = [(0,1,false) D1 区间[0,10], (2,8,true) U1 区间[10,52], (9,11,false) D_last 区间[52,70]]
    fn retreat_test_strokes() -> (Vec<Stroke>, Vec<(usize, usize, bool)>) {
        let strokes = vec![
            Stroke { start_price: 100.0, end_price: 110.0, start_bar: 0, end_bar: 5, is_up: true },
            Stroke { start_price: 110.0, end_price: 100.0, start_bar: 5, end_bar: 10, is_up: false }, // D1
            Stroke { start_price: 100.0, end_price: 108.0, start_bar: 10, end_bar: 16, is_up: true }, // U1 起点
            Stroke { start_price: 108.0, end_price: 98.0, start_bar: 16, end_bar: 22, is_up: false },
            Stroke { start_price: 98.0, end_price: 105.0, start_bar: 22, end_bar: 28, is_up: true },
            Stroke { start_price: 105.0, end_price: 101.0, start_bar: 28, end_bar: 34, is_up: false },
            Stroke { start_price: 101.0, end_price: 104.0, start_bar: 34, end_bar: 40, is_up: true },
            Stroke { start_price: 104.0, end_price: 103.0, start_bar: 40, end_bar: 46, is_up: false },
            Stroke { start_price: 103.0, end_price: 106.0, start_bar: 46, end_bar: 52, is_up: true },  // U1 终点
            Stroke { start_price: 106.0, end_price: 102.0, start_bar: 52, end_bar: 58, is_up: false }, // D_last 起点
            Stroke { start_price: 102.0, end_price: 107.0, start_bar: 58, end_bar: 64, is_up: true },
            Stroke { start_price: 107.0, end_price: 104.0, start_bar: 64, end_bar: 70, is_up: false }, // D_last 终点
        ];
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        (strokes, bigs)
    }

    /// 买点撤回: D_last 成立 → 撤回到前驱向上高级段 U1 区间[10,52]内最后买点 2+1买(46),
    /// 二买(34)/三买(40)/2+1(46) 保留, 2+2(58)/2+3(70) 删除.
    #[test]
    fn test_retreat_buy_keep_last_in_up_sup() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false)];
        let second = vec![
            SecondMarker { bar_index: 34, price: 101.0, is_buy: true },
        ];
        let third = vec![
            BigSegThirdMarker { bar_index: 40, price: 104.0, is_buy: true },
        ];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 },
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 },
            SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 3 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 1, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 34);
        assert_eq!(r.third_markers.len(), 1, "{:?}", r.third_markers);
        assert_eq!(r.third_markers[0].bar_index, 40);
        assert_eq!(r.plus_markers.len(), 1, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
    }

    /// 买点撤回: 前驱向上高级段区间内无买点 → 所有买点全部删除 (严格撤回语义).
    #[test]
    fn test_retreat_buy_all_removed_when_empty_region() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false)];
        let second = vec![SecondMarker { bar_index: 58, price: 102.0, is_buy: true }]; // U1 区间外
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 },
            SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 3 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert!(r.second_markers.is_empty(), "{:?}", r.second_markers);
        assert!(r.third_markers.is_empty());
        assert!(r.plus_markers.is_empty(), "{:?}", r.plus_markers);
    }

    /// 卖点镜像: 最后一个向上高级段 U1 成立 → 撤回到前驱向下高级段 D1 区间[0,10]内
    /// 最后卖点 二卖(5); 事件时序 (BUG 修复对称): cut=U1 终点bar=52, U1 成立时已存在的
    /// 2+1卖(46≤52) 删除, U1 成立后才产生的 2+2卖(58>52, D2 区间内) 保留.
    #[test]
    fn test_retreat_sell_mirror() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false)];
        let second = vec![SecondMarker { bar_index: 5, price: 105.0, is_buy: false }];
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 46, price: 107.0, is_buy: false, order: 1 },
            SecondPlusOneMarker { bar_index: 58, price: 109.0, is_buy: false, order: 2 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 1, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 5);
        assert_eq!(r.plus_markers.len(), 1, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0], SecondPlusOneMarker { bar_index: 58, price: 109.0, is_buy: false, order: 2 });
    }

    /// 无触发: 唯一向下高级段 (索引0, 无前驱向上高级段) → 买点全部保留不撤回.
    #[test]
    fn test_retreat_no_trigger_first_sup() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false)];
        let second: Vec<SecondMarker> = vec![];
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 },
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.plus_markers.len(), 2, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0].order, 1);
        assert_eq!(r.plus_markers[1].order, 2);
    }

    /// 多高级段叠加: sups=[D1,U1,D2,U2], 买点撤回到 U1 区间内最后买点(46);
    /// 卖点逐事件: U1 成立 → D1 区间内无卖点 → 撤回范围内 2+1卖(46, U1 终点前) 删除;
    /// U2 成立 → 撤回到 D2 区间[52,70]内最后卖点(58), 删 (58, U2终点64] 内卖点;
    /// 二卖(80>64) 属 U2 后反向向下高级段未成立阶段, 保留 — 买卖独立互不干扰.
    #[test]
    fn test_retreat_multi_sup_independent() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false), (10, 10, true)];
        let second = vec![
            SecondMarker { bar_index: 34, price: 101.0, is_buy: true },
            SecondMarker { bar_index: 58, price: 109.0, is_buy: false },
            SecondMarker { bar_index: 80, price: 103.0, is_buy: false },
        ];
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 },
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 },
            SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 3 },
            SecondPlusOneMarker { bar_index: 46, price: 107.0, is_buy: false, order: 1 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        // 买点: 区间[10,52]内 {34,46} max=46 → 删 bar>46 (2+2/2+3)
        assert_eq!(r.second_markers.len(), 3, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 34); // 二买保留
        assert_eq!(r.second_markers[1].bar_index, 58); // 二卖(58≤58)保留
        assert_eq!(r.second_markers[2].bar_index, 80); // 二卖(80>U2终点64) 未确认阶段保留
        // 卖点: U1 成立事件 D1 区间无卖点 → 2+1卖(46, 撤回范围内) 删除; U2 事件 X'=58, 删 (58,64]
        assert_eq!(r.plus_markers.len(), 1, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
    }

    /// 回归: 数据末尾是向上高级段 U_last (rposition 找到的 D2 在 U_last 之前),
    /// D2 早已成立, 其撤回不得删除 U_last 区间内买点 (二买/2+1/2+2, 事件时序) 及 U_last 终点bar后
    /// 未构成高级段的向下大段 X3 (2+3); cut = D2 终点bar, bar>cut 的买点全部保留.
    #[test]
fn test_retreat_buy_keep_markers_after_last_down_sup() {
        let (mut strokes, _) = retreat_test_strokes();
        // U_last: 大段12(向上, bar70-76) → 大段13(向下, bar76-80), 高级段区间[70,80]
        strokes.push(Stroke { start_price: 104.0, end_price: 112.0, start_bar: 70, end_bar: 76, is_up: true });
        strokes.push(Stroke { start_price: 112.0, end_price: 108.0, start_bar: 76, end_bar: 80, is_up: false });
        // X3: U_last 终点bar 80 后的向下大段 (未构成向下高级段) → 2+3 标记 bar 85
        strokes.push(Stroke { start_price: 108.0, end_price: 105.0, start_bar: 80, end_bar: 85, is_up: false });
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false), (12, 13, true)];
        // U1 区间[10,52]内买点: 二买(34)/2+1(46) → D2 撤回 X=46, 删 bar∈(46,70] 买点
        let second = vec![
            SecondMarker { bar_index: 34, price: 101.0, is_buy: true },
            SecondMarker { bar_index: 72, price: 109.0, is_buy: true }, // U_last 区间内二买 → 保留
        ];
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 }, // U1 内 → 保留
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 }, // D2 区间 → 删除
            SecondPlusOneMarker { bar_index: 74, price: 109.0, is_buy: true, order: 1 }, // U_last 内 2+1 → 保留
            SecondPlusOneMarker { bar_index: 78, price: 110.0, is_buy: true, order: 2 }, // U_last 内 2+2 → 保留
            SecondPlusOneMarker { bar_index: 85, price: 105.0, is_buy: true, order: 3 }, // X3 2+3 → 保留
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 2, "{:?}", r.second_markers); // 34 + 72
        assert_eq!(r.second_markers[0].bar_index, 34);
        assert_eq!(r.second_markers[1].bar_index, 72);
        assert_eq!(r.plus_markers.len(), 4, "{:?}", r.plus_markers); // 46/74/78/85 保留, 58 删除
        assert!(r.plus_markers.iter().all(|m| m.bar_index != 58));
    }

    /// 回归: D_last 前驱向上高级段区间内无买点 → 撤回范围内
    /// (前驱区间终点之后..cut) 全删, 更早区间内的二买/三买不受本次撤回影响 (保留).
    #[test]
    fn test_retreat_empty_region_keeps_earlier_markers() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false)];
        // U1 区间[10,52]内无买点; 更早 D1 区间[0,10]内二买(8)/三买(9) → 保留;
        // D_last 区间[52,70]内 2+2买(58)/2+3买(70) → 撤回范围内删除
        let second = vec![
            SecondMarker { bar_index: 8, price: 109.0, is_buy: true },
        ];
        let third = vec![
            BigSegThirdMarker { bar_index: 9, price: 110.0, is_buy: true },
        ];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 },
            SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 3 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 1, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 8);
        assert_eq!(r.third_markers.len(), 1, "{:?}", r.third_markers);
        assert_eq!(r.third_markers[0].bar_index, 9);
        assert!(r.plus_markers.is_empty(), "{:?}", r.plus_markers);
    }

    /// 回归: 多个向下高级段依次成立, 每次成立都触发撤回 —
    /// D2 成立撤回 U1 区间 (删 X1 之后..D2 终点), D4 成立再撤回 U3 区间 (删 X3 之后..D4 终点);
    /// 中间 D 区间内的 2+N 标记不留存.
    #[test]
    fn test_retreat_multi_down_sups_sequential() {
        let (mut strokes, _) = retreat_test_strokes();
        // U3: 大段12(up, bar70-80) → 大段13(down, bar80-90), 高级段区间[70,90]
        strokes.push(Stroke { start_price: 104.0, end_price: 112.0, start_bar: 70, end_bar: 80, is_up: true });
        strokes.push(Stroke { start_price: 112.0, end_price: 108.0, start_bar: 80, end_bar: 90, is_up: false });
        // D4: 大段14(down, bar90-100) → 大段15(up, bar100-110), 高级段区间[90,110]
        strokes.push(Stroke { start_price: 108.0, end_price: 100.0, start_bar: 90, end_bar: 100, is_up: false });
        strokes.push(Stroke { start_price: 100.0, end_price: 106.0, start_bar: 100, end_bar: 110, is_up: true });
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let bigs = segs.clone();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false), (12, 13, true), (14, 15, false)];
        let second = vec![
            SecondMarker { bar_index: 34, price: 101.0, is_buy: true }, // U1 内二买 → 保留
            SecondMarker { bar_index: 74, price: 109.0, is_buy: true }, // U3 内二买 → 保留
        ];
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 }, // U1 内 2+1 → 保留
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 }, // D2 区间 → D2 事件删除
            SecondPlusOneMarker { bar_index: 94, price: 105.0, is_buy: true, order: 2 }, // D4 区间 → D4 事件删除
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 2, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 34);
        assert_eq!(r.second_markers[1].bar_index, 74);
        assert_eq!(r.plus_markers.len(), 1, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0], SecondPlusOneMarker { bar_index: 46, price: 103.0, is_buy: true, order: 1 });
    }

    /// 最后向下高级段 D_last 成立后, 反向向上高级段未成立且未创新低 —
    /// D_last 终点bar 之后符合条件的买点必须保留.
    /// 前驱 U1 区间内无买点 → 撤回范围仅 (U1终点, D_last终点], bar>D_last 终点的买点保留;
    /// 旧实现 cut=MAX 会误删这些买点.
    #[test]
    fn test_retreat_buy_keep_unconfirmed_after_last_down_sup() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, false), (2, 8, true), (9, 11, false)];
        // D_last 区间[52,70]内 2+2买(58)/2+3买(70) → 撤回范围内删除;
        // D_last 终点bar 70 之后的买点: 二买(76)/2+1买(78) → 保留
        let second = vec![
            SecondMarker { bar_index: 8, price: 109.0, is_buy: true },  // 更早 D1 区间 → 保留
            SecondMarker { bar_index: 76, price: 104.0, is_buy: true },  // 未确认阶段 → 保留
        ];
        let third = vec![
            BigSegThirdMarker { bar_index: 9, price: 110.0, is_buy: true }, // 更早 D1 区间 → 保留
        ];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 58, price: 102.0, is_buy: true, order: 2 }, // D_last 区间 → 删除
            SecondPlusOneMarker { bar_index: 70, price: 104.0, is_buy: true, order: 3 }, // D_last 终点 → 删除
            SecondPlusOneMarker { bar_index: 78, price: 105.0, is_buy: true, order: 1 }, // 未确认阶段 → 保留
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 2, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 8);
        assert_eq!(r.second_markers[1].bar_index, 76);
        assert_eq!(r.third_markers.len(), 1, "{:?}", r.third_markers);
        assert_eq!(r.third_markers[0].bar_index, 9);
        assert_eq!(r.plus_markers.len(), 1, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0], SecondPlusOneMarker { bar_index: 78, price: 105.0, is_buy: true, order: 1 });
    }

    /// 镜像 (卖点): 最后向上高级段 U_last 成立后, 反向向下高级段未成立且未创新高 —
    /// U_last 终点bar 之后的卖点保留; 前驱 D1 区间内无卖点 → 撤回范围仅 (D1终点, U_last终点],
    /// bar>U_last 终点的卖点 (76/78) 保留.
    #[test]
    fn test_retreat_sell_keep_markers_after_last_up_sup() {
        let (strokes, bigs) = retreat_test_strokes();
        let segs: Vec<(usize, usize, bool)> = strokes.iter().enumerate().map(|(i, s)| (i, i, s.is_up)).collect();
        let sups: Vec<(usize, usize, bool)> = vec![(0, 1, true), (2, 8, false), (9, 11, true)];
        // 前驱 D1 区间[10,52]内无卖点; U_last 区间[52,70]内 2+2卖(58)/2+3卖(70) → 删除;
        // U_last 终点bar 70 之后的卖点 (76/78) → 保留
        let second = vec![SecondMarker { bar_index: 76, price: 110.0, is_buy: false }];
        let third: Vec<BigSegThirdMarker> = vec![];
        let plus = vec![
            SecondPlusOneMarker { bar_index: 58, price: 109.0, is_buy: false, order: 2 },
            SecondPlusOneMarker { bar_index: 70, price: 108.0, is_buy: false, order: 3 },
            SecondPlusOneMarker { bar_index: 78, price: 107.0, is_buy: false, order: 1 },
        ];
        let r = apply_sup_retreat_filter(&sups, &bigs, &segs, &strokes, second, third, plus);
        assert_eq!(r.second_markers.len(), 1, "{:?}", r.second_markers);
        assert_eq!(r.second_markers[0].bar_index, 76);
        assert_eq!(r.plus_markers.len(), 1, "{:?}", r.plus_markers);
        assert_eq!(r.plus_markers[0], SecondPlusOneMarker { bar_index: 78, price: 107.0, is_buy: false, order: 1 });
    }

    /// 边界: 向上笔不足3个时, 按算法设计不产出上轨 (up_count < 3 提前返回空).
    #[test]
    fn test_calc_upper_band_case1_insufficient_strokes() {
        let ff = vec![
            crate::Fractal { price: 30.0, is_top: true, bar_index: 20, merged_index: 0, time: 0 },
            crate::Fractal { price: 8.0, is_top: false, bar_index: 40, merged_index: 0, time: 0 },
            crate::Fractal { price: 30.0, is_top: true, bar_index: 60, merged_index: 0, time: 0 },
            crate::Fractal { price: 9.0, is_top: false, bar_index: 80, merged_index: 0, time: 0 },
        ];
        let (band, up) = calc_upper_band_case1(&ff);
        assert!(up.is_empty(), "1个向上笔不应产出上轨, got {}", up.len());
        assert!(band.is_empty(), "1个向上笔不应产出上轨点");
    }

    /// 核心: 3个向上笔 + 顶背离 (29→28→27 逐顶走低) → 产出1个上轨点 (value=29, bar=60).
    #[test]
    fn test_calc_upper_band_case1_triple_top_divergence() {
        let ff = vec![
            crate::Fractal { price: 30.0, is_top: true, bar_index: 20, merged_index: 0, time: 0 },
            crate::Fractal { price: 8.0, is_top: false, bar_index: 40, merged_index: 0, time: 0 },
            crate::Fractal { price: 29.0, is_top: true, bar_index: 60, merged_index: 0, time: 0 },
            crate::Fractal { price: 9.0, is_top: false, bar_index: 80, merged_index: 0, time: 0 },
            crate::Fractal { price: 28.0, is_top: true, bar_index: 100, merged_index: 0, time: 0 },
            crate::Fractal { price: 10.0, is_top: false, bar_index: 120, merged_index: 0, time: 0 },
            crate::Fractal { price: 27.0, is_top: true, bar_index: 140, merged_index: 0, time: 0 },
            crate::Fractal { price: 11.0, is_top: false, bar_index: 160, merged_index: 0, time: 0 },
        ];
        let (band, up) = calc_upper_band_case1(&ff);
        assert_eq!(up.len(), 3, "应产出3个向上笔, got {}", up.len());
        assert_eq!(band.len(), 1, "顶背离应产出1个上轨点, got {}", band.len());
        assert_eq!(band[0].value, 29.0);
        assert_eq!(band[0].bar_index, 60);
    }
}
