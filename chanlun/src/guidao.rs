//! 三级轨道系统 (笔/线段/大段 轨道与转信号) + 大段 2+N 买卖点 + 买点撤回过滤

use crate::{Fractal, MergedCandle, process_merged_candles, resolve_seg_end_point, resolve_sup_end_point, Stroke, SecondMarker};
use crate::zhongshu::BigSegThirdMarker;

#[derive(Debug, Clone)]
pub struct BandPoint {
    pub value: f64,
    pub bar_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct StrokeForTracking {
    pub high: f64,
    pub low: f64,
    pub bar_index: usize,
}

#[derive(Debug, Clone)]
pub struct TurnSignal {

    pub stroke_idx: usize,

    pub end_stroke_idx: usize,

    pub is_turn_short: bool,

    pub trigger_stroke_idx: usize,

    pub a_high: f64,

    pub a_low: f64,
}

#[inline]
fn c1_unconfirmed(third_is_last_same_dir_unit: bool, last_unit_same_dir: bool) -> bool {
    third_is_last_same_dir_unit && last_unit_same_dir
}

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


    let last_unit_up = final_fractals.last().map(|f| f.is_top).unwrap_or(false);
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..up_count {
        if c1_unconfirmed(i == up_count - 1, last_unit_up) { continue; }
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

    
    let last_unit_down = final_fractals.last().map(|f| !f.is_top).unwrap_or(false);
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..down_count {
        if c1_unconfirmed(i == down_count - 1, last_unit_down) { continue; }
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

pub fn detect_turn_signals(
    strokes: &[Stroke],
    segments: &[(usize, usize, bool)],
    highs: &[f64],
    lows: &[f64],
) -> Vec<TurnSignal> {
    detect_turn_signals_with_levels(strokes, segments, highs, lows, crate::StrokeCases::default())
}

pub fn detect_turn_signals_with_levels(
    strokes: &[Stroke],
    segments: &[(usize, usize, bool)],
    _highs: &[f64],
    _lows: &[f64],
    levels: crate::StrokeCases,
) -> Vec<TurnSignal> {
    



    
    
    let events = crate::collect_segment_case3_events_with_cases(strokes, levels.case3, levels.case4);
    let mut turn_signals: Vec<TurnSignal> = Vec::new();

    for (j, base, end_i, is_up) in events {
        if base >= strokes.len() || end_i >= strokes.len() { continue; }
        

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
            is_turn_short: !is_up, 
            trigger_stroke_idx: j,
            a_high,
            a_low,
        });
    }

    turn_signals
}

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

fn relocate_band_lift_points(
    band: Vec<BandPoint>,
    raw: &[BandPoint],
    merged: &[MergedCandle],
    fractals: &[Fractal],
    is_upper: bool,
) -> Vec<BandPoint> {
    let raw_bars: std::collections::HashSet<usize> = raw.iter().map(|p| p.bar_index).collect();

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
            out.push(p); 
        } else if let Some(&cb) = confirm.get(&p.bar_index) {
            out.push(BandPoint { value: p.value, bar_index: cb }); 
        } else {
            out.push(p); 
        }
    }
    out.sort_by_key(|p| p.bar_index);

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

pub fn compute_stroke_bands(
    final_fractals: &[Fractal],
    strokes: &[Stroke],
    segments: &[(usize, usize, bool)],
    highs: &[f64],
    lows: &[f64],
) -> (Vec<BandPoint>, Vec<BandPoint>) {
    compute_stroke_bands_with_levels(final_fractals, strokes, segments, highs, lows, crate::StrokeCases::default())
}

pub fn compute_stroke_bands_with_levels(
    final_fractals: &[Fractal],
    strokes: &[Stroke],
    segments: &[(usize, usize, bool)],
    highs: &[f64],
    lows: &[f64],
    levels: crate::StrokeCases,
) -> (Vec<BandPoint>, Vec<BandPoint>) {

    let (upper_c1, up_strokes) = calc_upper_band_case1(final_fractals);
    let upper_c2 = calc_upper_band_case2(final_fractals);
    let (lower_c1, down_strokes) = calc_lower_band_case1(final_fractals);
    let lower_c2 = calc_lower_band_case2(final_fractals);


    let turn_signals = detect_turn_signals_with_levels(strokes, segments, highs, lows, levels);
    let upper_c3 = calc_upper_band_case3(&turn_signals, strokes);
    let lower_c3 = calc_lower_band_case3(&turn_signals, strokes);


    let v_signals = detect_v_signals(&turn_signals, strokes, highs, lows);
    let upper_c4 = calc_upper_band_case4(&v_signals, strokes);
    let lower_c4 = calc_lower_band_case4(&v_signals, strokes);


    let mut upper_raw = upper_c1;
    upper_raw.extend(upper_c2);
    upper_raw.extend(upper_c3);
    upper_raw.extend(upper_c4);
    upper_raw.sort_by_key(|p| p.bar_index);
    let upper_band = apply_tracking(&upper_raw, &up_strokes, true);
    
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

#[derive(Debug, Clone)]
pub struct SegmentTurnSignal {
    pub seg_idx: usize,
    pub end_seg_idx: usize,
    pub is_turn_short: bool,
    pub a_high: f64,
    pub a_low: f64,

    pub trigger_seg_idx: usize,
}

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
    
    let last_unit_up = segments.last().map(|s| s.2).unwrap_or(false);
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..up_count {
        if c1_unconfirmed(i == up_count - 1, last_unit_up) { continue; }
        if up_segs[i].high < up_segs[i - 2].high && up_segs[i - 1].high < up_segs[i - 2].high {
            band_points.push(BandPoint { value: up_segs[i - 2].high, bar_index: up_segs[i - 2].bar_index });
        }
    }
    (band_points, up_segs)
}

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
    
    let last_unit_down = segments.last().map(|s| !s.2).unwrap_or(false);
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..down_count {
        if c1_unconfirmed(i == down_count - 1, last_unit_down) { continue; }
        if down_segs[i].low > down_segs[i - 2].low && down_segs[i - 1].low > down_segs[i - 2].low {
            band_points.push(BandPoint { value: down_segs[i - 2].low, bar_index: down_segs[i - 2].bar_index });
        }
    }
    (band_points, down_segs)
}

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

pub fn detect_segment_turn_signals(
    segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    _highs: &[f64],
    _lows: &[f64],
) -> Vec<SegmentTurnSignal> {
    detect_segment_turn_signals_with_levels(segments, big_segments, strokes, _highs, _lows, crate::StrokeCases::default())
}

pub fn detect_segment_turn_signals_with_levels(
    segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    _highs: &[f64],
    _lows: &[f64],
    levels: crate::StrokeCases,
) -> Vec<SegmentTurnSignal> {
    let n_segments = segments.len();
    if n_segments < 2 { return vec![]; }

    let projected: Vec<Stroke> = segments.iter().map(|(si, ei, is_up)| Stroke {
        is_up: *is_up, start_price: strokes[*si].start_price,
        end_price: strokes[*ei].end_price, start_bar: 0, end_bar: 0,
    }).collect();
    let events = crate::collect_segment_case3_events_with_cases(&projected, levels.case3, levels.case4);
    let mut turn_signals: Vec<SegmentTurnSignal> = Vec::new();

    for (j, base, end_i, is_up) in events {
        if base >= n_segments || end_i >= n_segments { continue; }
        

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
            is_turn_short: !is_up, 
            a_high,
            a_low,
            trigger_seg_idx: j,
        });
    }

    turn_signals
}

pub fn detect_segment_v_signals(
    seg_turn_signals: &[SegmentTurnSignal],
    _segments: &[(usize, usize, bool)],
    _strokes: &[Stroke],
    _highs: &[f64],
    _lows: &[f64],
) -> Vec<SegmentTurnSignal> {
    if seg_turn_signals.is_empty() { return vec![]; }


    let trigger_map: std::collections::HashMap<usize, bool> = seg_turn_signals
        .iter()
        .map(|ts| (ts.trigger_seg_idx, ts.is_turn_short))
        .collect();

    let mut v_signals: Vec<SegmentTurnSignal> = Vec::new();
    for ts in seg_turn_signals {
        let j = ts.trigger_seg_idx;
        match trigger_map.get(&(j + 1)) {
            Some(&next_short) if next_short != ts.is_turn_short => {

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

pub fn compute_segment_bands(
    segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
    final_fractals: &[Fractal],
) -> (Vec<BandPoint>, Vec<BandPoint>) {
    compute_segment_bands_with_levels(segments, big_segments, strokes, highs, lows, final_fractals, crate::StrokeCases::default())
}

pub fn compute_segment_bands_with_levels(
    segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
    final_fractals: &[Fractal],
    levels: crate::StrokeCases,
) -> (Vec<BandPoint>, Vec<BandPoint>) {
    let (seg_upper_c1, seg_up) = calc_seg_upper_band_case1(segments, strokes);
    let seg_upper_c2 = calc_seg_upper_band_case2(segments, strokes);
    let (seg_lower_c1, seg_down) = calc_seg_lower_band_case1(segments, strokes);
    let seg_lower_c2 = calc_seg_lower_band_case2(segments, strokes);
    let seg_turn_signals = detect_segment_turn_signals_with_levels(segments, big_segments, strokes, highs, lows, levels);
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
    
    let merged = process_merged_candles(highs, lows);
    let upper_band = relocate_band_lift_points(upper_band, &upper_raw, &merged, final_fractals, true);
    let lower_band = relocate_band_lift_points(lower_band, &lower_raw, &merged, final_fractals, false);
    (upper_band, lower_band)
}

#[derive(Debug, Clone)]
pub struct BigSegmentTurnSignal {
    pub seg_idx: usize,
    pub end_seg_idx: usize,
    pub is_turn_short: bool,
    pub a_high: f64,
    pub a_low: f64,

    pub trigger_seg_idx: usize,
}

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
    
    let last_unit_up = big_segments.last().map(|b| b.2).unwrap_or(false);
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..up_count {
        if c1_unconfirmed(i == up_count - 1, last_unit_up) { continue; }
        if up_big[i].high < up_big[i - 2].high && up_big[i - 1].high < up_big[i - 2].high {
            band_points.push(BandPoint { value: up_big[i - 2].high, bar_index: up_big[i - 2].bar_index });
        }
    }
    (band_points, up_big)
}

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
    
    let last_unit_down = big_segments.last().map(|b| !b.2).unwrap_or(false);
    let mut band_points: Vec<BandPoint> = Vec::new();
    for i in 2..down_count {
        if c1_unconfirmed(i == down_count - 1, last_unit_down) { continue; }
        if down_big[i].low > down_big[i - 2].low && down_big[i - 1].low > down_big[i - 2].low {
            band_points.push(BandPoint { value: down_big[i - 2].low, bar_index: down_big[i - 2].bar_index });
        }
    }
    (band_points, down_big)
}

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

pub fn detect_big_seg_turn_signals(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_segments: &[(usize, usize, bool)],
) -> Vec<BigSegmentTurnSignal> {
    detect_big_seg_turn_signals_with_levels(big_segments, segments, strokes, superior_segments, crate::StrokeCases::default())
}

pub fn detect_big_seg_turn_signals_with_levels(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_segments: &[(usize, usize, bool)],
    levels: crate::StrokeCases,
) -> Vec<BigSegmentTurnSignal> {
    let n_bigs = big_segments.len();
    if n_bigs < 2 { return vec![]; }

    let events = crate::collect_superior_case3_events_with_cases(strokes, segments, big_segments, levels.case3, levels.case4);
    let mut turn_signals: Vec<BigSegmentTurnSignal> = Vec::new();

    for (j, base, end_i, is_up) in events {
        if base >= n_bigs || end_i >= n_bigs { continue; }
        

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
            is_turn_short: !is_up, 
            a_high,
            a_low,
            trigger_seg_idx: j,
        });
    }

    turn_signals
}

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

pub fn detect_big_v_signals(
    big_turn_signals: &[BigSegmentTurnSignal],
) -> Vec<BigSegmentTurnSignal> {
    if big_turn_signals.is_empty() { return vec![]; }


    let trigger_map: std::collections::HashMap<usize, bool> = big_turn_signals
        .iter()
        .map(|ts| (ts.trigger_seg_idx, ts.is_turn_short))
        .collect();

    let mut v_signals: Vec<BigSegmentTurnSignal> = Vec::new();
    for ts in big_turn_signals {
        let j = ts.trigger_seg_idx;
        match trigger_map.get(&(j + 1)) {
            Some(&next_short) if next_short != ts.is_turn_short => {

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

pub fn compute_bigseg_bands(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_segments: &[(usize, usize, bool)],
    final_fractals: &[Fractal],
    highs: &[f64],
    lows: &[f64],
) -> (Vec<BandPoint>, Vec<BandPoint>, Vec<BandPoint>, Vec<BandPoint>) {
    compute_bigseg_bands_with_levels(big_segments, segments, strokes, superior_segments, final_fractals, highs, lows, crate::StrokeCases::default())
}

pub fn compute_bigseg_bands_with_levels(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_segments: &[(usize, usize, bool)],
    final_fractals: &[Fractal],
    highs: &[f64],
    lows: &[f64],
    levels: crate::StrokeCases,
) -> (Vec<BandPoint>, Vec<BandPoint>, Vec<BandPoint>, Vec<BandPoint>) {
    let (bigseg_upper_c1, bigseg_up) = calc_bigseg_upper_band_case1(big_segments, segments, strokes);
    let bigseg_upper_c2 = calc_bigseg_upper_band_case2(big_segments, segments, strokes);
    let (bigseg_lower_c1, bigseg_down) = calc_bigseg_lower_band_case1(big_segments, segments, strokes);
    let bigseg_lower_c2 = calc_bigseg_lower_band_case2(big_segments, segments, strokes);
    let bigseg_turn_signals = detect_big_seg_turn_signals_with_levels(big_segments, segments, strokes, superior_segments, levels);
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
    
    let merged = process_merged_candles(highs, lows);
    let upper_band = relocate_band_lift_points(upper_band, &upper_raw, &merged, final_fractals, true);
    let lower_band = relocate_band_lift_points(lower_band, &lower_raw, &merged, final_fractals, false);
    (upper_band, lower_band, upper_raw, lower_raw)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondPlusOneMarker {
    pub bar_index: usize,
    pub price: f64,

    pub is_buy: bool,

    pub order: u32,
}

pub fn detect_bigseg_cf_buy_markers(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    bigseg_lower_c1: &[BandPoint],
    down_big: &[StrokeForTracking],
    up_big: &[StrokeForTracking],
) -> Vec<SecondPlusOneMarker> {
    // 旧入口（既有调用点零改动）: 不传大三买阻断集 ⇒ 逐条等于改动前
    detect_bigseg_cf_buy_markers_stop(
        sups, bigs, segs, strokes, bigseg_lower_c1, down_big, up_big, &[],
    )
}

/// ［阻断终止约束］大2+N买 的「大三买阻断」终止约束**（2+N 序列终止判据，与主体判定同构，只多一道闸）
///
///
/// 语义：2+N 是「三底背离 F（=大二买）之后同向段终点的递推」，一旦其后第一个向下大段
/// 的终点本身已成为**大三买**（= 中枢A 的延伸已终结、结构进入新中枢领地），
/// 该 2+N 序列**即刻终止**，其后（含同向段未成立的过渡阶段）不再标记。
///
/// 判据实现：对序列中每个已产出标记 `prev`（F 点 order=1 与各递推项），
///   取 `nxt` = `prev` 之后第一个向下大段终点 bar；
///   若 `nxt ∈ third_buy_bars`（大三买基准 bar 集）⇒ 该序列在 `nxt` 处终止。
///   递推循环内一旦 `xb >= 该终止 bar` 即 break。
///
/// ⚠️ 买侧；卖侧见对应镜像函数。`third_buy_bars` 传空切片 ⇒ **完全等于旧行为**
///    （既有调用点与全部历史测试逐条不变）。
/// ⚠️ `third_buy_bars` 应传**大三买基准集**（`detect_bigseg_third_marks` 的买侧输出，
///    即 `bigseg_third_marks_all_zhongshu` 中 `order == 0` 的项），**不含** 3+N 递推项——
///    基准集已 100% 覆盖全部 2+N∩3+N 冲突（10 数据集 / 90 万 bar 实测）。
pub fn detect_bigseg_cf_buy_markers_stop(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    _bigseg_lower_c1: &[BandPoint],
    _down_big: &[StrokeForTracking],
    _up_big: &[StrokeForTracking],
    third_buy_bars: &[usize],
) -> Vec<SecondPlusOneMarker> {
    if sups.is_empty() { return vec![]; }

    // 部件(大段)终点列表 (bar 升序; 与高2+N 的高级段终点列表语义位置一致)
    let down_bigs: Vec<(usize, f64)> = bigs.iter().enumerate().filter(|(_, s)| !s.2)
        .filter_map(|(i, _)| resolve_sup_end_point(i, bigs, segs, strokes)).collect();
    let up_bigs: Vec<(usize, f64)> = bigs.iter().enumerate().filter(|(_, s)| s.2)
        .filter_map(|(i, _)| resolve_sup_end_point(i, bigs, segs, strokes)).collect();

    // 大三买阻断: `prev` 之后第一个向下大段终点若为大三买 ⇒ 序列终止 bar
    let stop_bar_of = |prev_bar: usize| -> Option<usize> {
        if third_buy_bars.is_empty() { return None; }
        down_bigs.iter().map(|(b, _)| *b).find(|b| *b > prev_bar)
            .filter(|b| third_buy_bars.contains(b))
    };

    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_big, is_up)) in sups.iter().enumerate() {
        if is_up { continue; } // 只处理向下高级段
        // 检测点 = 向下高级段终点 (低点)
        let Some((dbe, d_price)) = resolve_sup_end_point(end_big, bigs, segs, strokes) else { continue; };

        // F = 检测点后首个三底背离组 (连续 3 个向下大段, 第3底>第1底 且 第2底>第1底) 的第三底
        let mut f: Option<(usize, f64)> = None;
        for i in 2..down_bigs.len() {
            let (b0, l0) = down_bigs[i - 2];
            let (_b1, l1) = down_bigs[i - 1];
            let (b2, l2) = down_bigs[i];
            if b0 >= dbe && l2 > l0 && l1 > l0 {
                f = Some((b2, l2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        // [窗口守卫] 三底第一底须落在 [检测点终点, 下一个同向(向下)高级段终点) 窗口内;
        // 跨段共享三底组 → 相同标记重复产出, 跳过本检测点 (与高2+N 逐条对齐)
        let next_down_sup = sups[k + 1..].iter().find(|s| !s.2)
            .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
        if let Some((nb, _)) = next_down_sup {
            if third_bar >= nb { continue; }
        }

        // C = 检测点后首个向上大段终点 (high)
        let Some((_, c_price)) = up_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
        if c_price > f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
            // 2+N 递推 (对齐高2+N): 参考高点 = min(U1, U2) 固定;
            // 新向下高级段终点 bar 为界; 满足 min(U1,U2) > X > 检测点价 → 依次标记 2+2, 2+3, ...
            let second_up = up_bigs.iter().filter(|(b, _)| *b > dbe).nth(1);
            let ref_high = second_up.map(|(_, h)| c_price.min(*h));
            if let Some(rh) = ref_high {
                let mut order = 2;
                // 大三买阻断: F 点(大二买)之后第一个向下大段若为大三买 ⇒ 序列即刻终止
                let mut stop = stop_bar_of(third_bar);
                for (xb, xl) in down_bigs.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_down_sup {
                        if *xb >= b { break; }
                    }
                    if let Some(sb) = stop {
                        if *xb >= sb { break; }
                    }
                    if rh > *xl && *xl > d_price {
                        markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                        order += 1;
                        // 本项自身成为新的「前一个买点」: 其后的第一个向下大段若为大三买同样终止
                        if stop.is_none() {
                            stop = stop_bar_of(*xb);
                        }
                    }
                }
            }
        }
    }

    // ── 新增: 初始数据特定情况·变体1（初始数据补充检测点） ──
    // 初始数据没有向下高级段、却有向上高级段 (sups 首段为 UP 且其起点 > 0, 开头存在游离向下大段)
    // → 游离向下大段作为补充检测点, 判定与现有完全一致 (三底背离 F / C > F / 2+N 递推);
    // 守卫: 第三底须在第一个向下高级段终点之前 (与现有 next_down_sup 守卫同语义)
    if let Some(&(first_start, _, true)) = sups.first() {
        if first_start > 0 {
            let first_down_sup_end = sups.iter().find(|s| !s.2)
                .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
            for k in 0..first_start {
                if bigs[k].2 { continue; } // 只处理向下大段 (补充检测点, 与现有 buy 循环同语义)
                let Some((dbe, d_price)) = resolve_sup_end_point(k, bigs, segs, strokes) else { continue; };
                // F = 检测点后首个三底背离组 (第三底)
                let mut f: Option<(usize, f64)> = None;
                for i in 2..down_bigs.len() {
                    let (b0, l0) = down_bigs[i - 2];
                    let (_b1, l1) = down_bigs[i - 1];
                    let (b2, l2) = down_bigs[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };
                // [窗口守卫] 第三底须在第一个向下高级段终点之前
                if let Some((nb, _)) = first_down_sup_end {
                    if third_bar >= nb { continue; }
                }
                // C = 检测点后首个向上大段终点 (high)
                let Some((_, c_price)) = up_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                if c_price > f_price {
                    markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
                    // 2+N 递推 (与现有一致): 参考高点 = min(U1, U2) 固定; 边界 = 第一个向下高级段终点
                    let second_up = up_bigs.iter().filter(|(b, _)| *b > dbe).nth(1);
                    let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                    if let Some(rh) = ref_high {
                        let mut order = 2;
                        // 大三买阻断 (初始数据分支同步, 与主体同规)
                        let mut stop = stop_bar_of(third_bar);
                        for (xb, xl) in down_bigs.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_down_sup_end {
                                if *xb >= b { break; }
                            }
                            if let Some(sb) = stop {
                                if *xb >= sb { break; }
                            }
                            if rh > *xl && *xl > d_price {
                                markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                order += 1;
                                if stop.is_none() {
                                    stop = stop_bar_of(*xb);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 新增: 初始数据特定情况·变体2（初始数据补充检测点） ──
    // 初始数据没有向下高级段、却有向上高级段 case2 (首段 UP 且从第一个大段开始, 无游离段):
    // 数据入口 = 向上高级段 case2 的起点 (低点);
    // 起点注入为三底背离组第一底 (检测点自身=第一底, 与现有 2+1 语义一致),
    // 其后向下大段终点与起点价比较 (第3底>起点 且 第2底>起点) → F; C>F → 2+1买;
    // 比较对象 = 组内第一底 (起点), 非相邻前底
    if let Some(&(first_start, _, true)) = sups.first() {
        if first_start == 0 {
            // 数据入口 = 高级段[0] 起点 (即其起始大段起点低点)
            if let Some((dbe, d_price)) = crate::resolve_sup_start_point(sups[0].0, bigs, segs, strokes) {
                // 底部候选 = 虚拟第一底 (检测点自身) + 其后向下大段终点
                let mut bottoms: Vec<(usize, f64)> = Vec::with_capacity(down_bigs.len() + 1);
                bottoms.push((dbe, d_price));
                bottoms.extend(down_bigs.iter().filter(|(b, _)| *b > dbe).copied());
                // F = 首个三底背离组第三底 (第3底>第1底 且 第2底>第1底, 与现有判定逐条一致)
                let mut f: Option<(usize, f64)> = None;
                for i in 2..bottoms.len() {
                    let (b0, l0) = bottoms[i - 2];
                    let (_b1, l1) = bottoms[i - 1];
                    let (b2, l2) = bottoms[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {
                    // [窗口守卫] 第三底须在第一个向下高级段终点之前 (与现有初始分支同语义)
                    let first_down_sup_end = sups.iter().find(|s| !s.2)
                        .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
                    let window_ok = match first_down_sup_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        // C = 检测点后首个向上大段终点 (high)
                        let c = up_bigs.iter().find(|(b, _)| *b > dbe).copied();
                        if let Some((_, c_price)) = c {
                            if c_price > f_price {
                                markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
                                // 2+N 递推 (与现有一致): 参考高点 = min(U1, U2) 固定; 边界 = 第一个向下高级段终点
                                let second_up = up_bigs.iter().filter(|(b, _)| *b > dbe).nth(1);
                                let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                                if let Some(rh) = ref_high {
                                    let mut order = 2;
                                    // 大三买阻断 (初始数据特定情况·变体2 同步, 与主体同规)
                                    let mut stop = stop_bar_of(third_bar);
                                    for (xb, xl) in down_bigs.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_down_sup_end {
                                            if *xb >= b { break; }
                                        }
                                        if let Some(sb) = stop {
                                            if *xb >= sb { break; }
                                        }
                                        if rh > *xl && *xl > d_price {
                                            markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                            order += 1;
                                            if stop.is_none() {
                                                stop = stop_bar_of(*xb);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    markers
}

pub fn detect_bigseg_cf_sell_markers(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    bigseg_upper_c1: &[BandPoint],
    up_big: &[StrokeForTracking],
    down_big: &[StrokeForTracking],
) -> Vec<SecondPlusOneMarker> {
    // 旧入口（既有调用点零改动）: 不传大三卖阻断集 ⇒ 逐条等于改动前
    detect_bigseg_cf_sell_markers_stop(
        sups, bigs, segs, strokes, bigseg_upper_c1, up_big, down_big, &[],
    )
}

/// ［阻断终止约束］大2+N卖 的「大三卖阻断」终止约束**（买侧镜像）
///
/// 定版规则：「『大二卖』或『大2+N卖』之后的第一个向上大段，若是大三卖，
///   则该 2+N 序列即刻终止（含同向段未成立的过渡阶段）」—— 与买侧对称。
///
/// 语义：2+N卖 是「三顶背离 F（=大二卖）之后同向段终点的递推」，一旦其后第一个**向上大段**
/// 的终点本身已成为**大三卖**（= 中枢A 延伸终结），该序列**即刻终止**，其后不再标记。
///
/// ⚠️ `third_sell_bars` 传空切片 ⇒ **完全等于旧行为**；应传**大三卖基准集**
///    （`detect_bigseg_third_marks` 的卖侧输出，不含 3+N 递推项）。
pub fn detect_bigseg_cf_sell_markers_stop(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    _bigseg_upper_c1: &[BandPoint],
    _up_big: &[StrokeForTracking],
    _down_big: &[StrokeForTracking],
    third_sell_bars: &[usize],
) -> Vec<SecondPlusOneMarker> {
    if sups.is_empty() { return vec![]; }

    // 部件(大段)终点列表 (bar 升序; 与高2+N卖 的高级段终点列表语义位置一致)
    let down_bigs: Vec<(usize, f64)> = bigs.iter().enumerate().filter(|(_, s)| !s.2)
        .filter_map(|(i, _)| resolve_sup_end_point(i, bigs, segs, strokes)).collect();
    let up_bigs: Vec<(usize, f64)> = bigs.iter().enumerate().filter(|(_, s)| s.2)
        .filter_map(|(i, _)| resolve_sup_end_point(i, bigs, segs, strokes)).collect();

    // 大三卖阻断 (买侧镜像): `prev` 之后第一个**向上大段**终点若为大三卖 ⇒ 序列终止
    let stop_bar_of = |prev_bar: usize| -> Option<usize> {
        if third_sell_bars.is_empty() { return None; }
        up_bigs.iter().map(|(b, _)| *b).find(|b| *b > prev_bar)
            .filter(|b| third_sell_bars.contains(b))
    };

    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_big, is_up)) in sups.iter().enumerate() {
        if !is_up { continue; } // 只处理向上高级段
        // 检测点 = 向上高级段终点 (高点)
        let Some((ube, d_price)) = resolve_sup_end_point(end_big, bigs, segs, strokes) else { continue; };

        // F = 检测点后首个三顶背离组 (连续 3 个向上大段, 第3顶<第1顶 且 第2顶<第1顶) 的第三顶
        let mut f: Option<(usize, f64)> = None;
        for i in 2..up_bigs.len() {
            let (b0, h0) = up_bigs[i - 2];
            let (_b1, h1) = up_bigs[i - 1];
            let (b2, h2) = up_bigs[i];
            if b0 >= ube && h2 < h0 && h1 < h0 {
                f = Some((b2, h2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        // [窗口守卫] 三顶第一顶须落在 [检测点终点, 下一个同向(向上)高级段终点) 窗口内
        let next_up_sup = sups[k + 1..].iter().find(|s| s.2)
            .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
        if let Some((nb, _)) = next_up_sup {
            if third_bar >= nb { continue; }
        }

        // C = 检测点后首个向下大段终点 (low)
        let Some((_, c_price)) = down_bigs.iter().find(|(b, _)| *b > ube).copied() else { continue; };
        if c_price < f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
            // 2+N 递推 (镜像, 对齐高2+N卖): 参考低点 = max(C', C'+1) 固定;
            // 新向上高级段终点 bar 为界; 满足 max(C',C'+1) < X < 检测点价 → 依次标记 2+2, 2+3, ...
            let second_down = down_bigs.iter().filter(|(b, _)| *b > ube).nth(1);
            let ref_low = second_down.map(|(_, l)| c_price.max(*l));
            if let Some(rl) = ref_low {
                let mut order = 2;
                // 大三卖阻断 (买侧镜像): F(大二卖)之后第一个向上大段若为大三卖 ⇒ 序列即刻终止
                let mut stop = stop_bar_of(third_bar);
                for (xb, xh) in up_bigs.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_up_sup {
                        if *xb >= b { break; }
                    }
                    if let Some(sb) = stop {
                        if *xb >= sb { break; }
                    }
                    if rl < *xh && *xh < d_price {
                        markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                        order += 1;
                        if stop.is_none() {
                            stop = stop_bar_of(*xb);
                        }
                    }
                }
            }
        }
    }

    // ── 新增: 初始数据特定情况·变体1（初始数据补充检测点） ──
    // 初始数据没有向上高级段、却有向下高级段 (sups 首段为 DOWN 且其起点 > 0, 开头存在游离向上大段)
    // → 游离向上大段作为补充检测点, 判定与现有完全一致 (三顶背离 F / C'<F' / 2+N 递推);
    // 守卫: 第三顶须在第一个向上高级段终点之前 (与现有 next_up_sup 守卫同语义)
    if let Some(&(first_start, _, false)) = sups.first() {
        if first_start > 0 {
            let first_up_sup_end = sups.iter().find(|s| s.2)
                .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
            for k in 0..first_start {
                if !bigs[k].2 { continue; } // 只处理向上大段 (补充检测点)
                let Some((ube, d_price)) = resolve_sup_end_point(k, bigs, segs, strokes) else { continue; };
                // F = 检测点后首个三顶背离组 (第三顶)
                let mut f: Option<(usize, f64)> = None;
                for i in 2..up_bigs.len() {
                    let (b0, h0) = up_bigs[i - 2];
                    let (_b1, h1) = up_bigs[i - 1];
                    let (b2, h2) = up_bigs[i];
                    if b0 >= ube && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };
                // [窗口守卫] 第三顶须在第一个向上高级段终点之前
                if let Some((nb, _)) = first_up_sup_end {
                    if third_bar >= nb { continue; }
                }
                // C = 检测点后首个向下大段终点 (low)
                let Some((_, c_price)) = down_bigs.iter().find(|(b, _)| *b > ube).copied() else { continue; };
                if c_price < f_price {
                    markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                    // 2+N 递推 (镜像, 与现有一致): 参考低点 = max(C', C'+1) 固定; 边界 = 第一个向上高级段终点
                    let second_down = down_bigs.iter().filter(|(b, _)| *b > ube).nth(1);
                    let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                    if let Some(rl) = ref_low {
                        let mut order = 2;
                        // 大三卖阻断 (初始数据分支同步, 与主体同规)
                        let mut stop = stop_bar_of(third_bar);
                        for (xb, xh) in up_bigs.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_up_sup_end {
                                if *xb >= b { break; }
                            }
                            if let Some(sb) = stop {
                                if *xb >= sb { break; }
                            }
                            if rl < *xh && *xh < d_price {
                                markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                order += 1;
                                if stop.is_none() {
                                    stop = stop_bar_of(*xb);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 新增: 初始数据特定情况·变体2（初始数据补充检测点） ──
    // 初始数据没有向上高级段、却有向下高级段 case2 (首段 DOWN 且从第一个大段开始, 无游离段):
    // 数据入口 = 向下高级段 case2 的起点 (高点); 起点注入为三顶背离组第一顶 (检测点自身=第一顶),
    // 其后向上大段终点与起点价比较 (第3顶<起点 且 第2顶<起点) → F; C<F → 2+1卖;
    // 比较对象 = 组内第一顶 (起点), 非相邻前顶 (卖镜像, 与买侧同规则)
    if let Some(&(first_start, _, false)) = sups.first() {
        if first_start == 0 {
            // 数据入口 = 高级段[0] 起点 (即其起始大段起点高点)
            if let Some((ube, d_price)) = crate::resolve_sup_start_point(sups[0].0, bigs, segs, strokes) {
                // 顶部候选 = 虚拟第一顶 (检测点自身) + 其后向上大段终点
                let mut tops: Vec<(usize, f64)> = Vec::with_capacity(up_bigs.len() + 1);
                tops.push((ube, d_price));
                tops.extend(up_bigs.iter().filter(|(b, _)| *b > ube).copied());
                // F = 首个三顶背离组第三顶 (第3顶<第1顶 且 第2顶<第1顶, 与现有判定逐条一致)
                let mut f: Option<(usize, f64)> = None;
                for i in 2..tops.len() {
                    let (b0, h0) = tops[i - 2];
                    let (_b1, h1) = tops[i - 1];
                    let (b2, h2) = tops[i];
                    if b0 >= ube && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {
                    // [窗口守卫] 第三顶须在第一个向上高级段终点之前 (与现有初始分支同语义)
                    let first_up_sup_end = sups.iter().find(|s| s.2)
                        .and_then(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes));
                    let window_ok = match first_up_sup_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        // C = 检测点后首个向下大段终点 (low)
                        let c = down_bigs.iter().find(|(b, _)| *b > ube).copied();
                        if let Some((_, c_price)) = c {
                            if c_price < f_price {
                                markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                                // 2+N 递推 (镜像, 与现有一致): 参考低点 = max(C', C'+1) 固定; 边界 = 第一个向上高级段终点
                                let second_down = down_bigs.iter().filter(|(b, _)| *b > ube).nth(1);
                                let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                                if let Some(rl) = ref_low {
                                    let mut order = 2;
                                    // 大三卖阻断 (初始数据特定情况·变体2 同步, 与主体同规)
                                    let mut stop = stop_bar_of(third_bar);
                                    for (xb, xh) in up_bigs.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_up_sup_end {
                                            if *xb >= b { break; }
                                        }
                                        if let Some(sb) = stop {
                                            if *xb >= sb { break; }
                                        }
                                        if rl < *xh && *xh < d_price {
                                            markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                            order += 1;
                                            if stop.is_none() {
                                                stop = stop_bar_of(*xb);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    markers
}


pub struct RetreatFilterResult {
    pub second_markers: Vec<SecondMarker>,
    pub third_markers: Vec<BigSegThirdMarker>,
    pub plus_markers: Vec<SecondPlusOneMarker>,
    
    pub superior_second_markers: Vec<SecondMarker>,
}

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

pub fn apply_sup_retreat_filter(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    mut second_markers: Vec<SecondMarker>,
    mut third_markers: Vec<BigSegThirdMarker>,
    mut plus_markers: Vec<SecondPlusOneMarker>,
    mut superior_second_markers: Vec<SecondMarker>,
) -> RetreatFilterResult {

    for d_idx in (1..sups.len()).filter(|&i| !sups[i].2) {
        if let Some((sb, eb)) = sup_range(sups[d_idx - 1], bigs, segs, strokes) {
            let mut buy_bars: Vec<usize> = Vec::new();
            for m in second_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in third_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in plus_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in superior_second_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }

            
            let cut = sup_range(sups[d_idx], bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match buy_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => {
                    second_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    third_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    plus_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    superior_second_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                }
                None => {

                    second_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    third_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    plus_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    superior_second_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                }
            }
        }
    }
    
    for u_idx in (1..sups.len()).filter(|&i| sups[i].2) {
        if let Some((sb, eb)) = sup_range(sups[u_idx - 1], bigs, segs, strokes) {
            let mut sell_bars: Vec<usize> = Vec::new();
            for m in second_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in third_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in plus_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in superior_second_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            let cut = sup_range(sups[u_idx], bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match sell_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => {
                    second_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    third_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    plus_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    superior_second_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                }
                None => {
                    
                    second_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    third_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    plus_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    superior_second_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                }
            }
        }
    }
    RetreatFilterResult { second_markers, third_markers, plus_markers, superior_second_markers }
}


// ═══ 线段级/笔级 2+N 买/卖 (宿主段 = 大段/线段, 部件 = 线段/笔) ═══
// ===== 线段 2+N买/卖 + 线段买点撤回 () =====
/// 线2+N买 (宿主 = 大段, 部件 = 线段; 与 `detect_bigseg_cf_buy_markers` **逐行同构**, 层级降一级):
/// 检测点 = 向下大段终点; 其后首个三底背离组 (连续 3 个向下线段, 第3底>第1底 且 第2底>第1底) 的第三底 F,
/// C = 检测点后首个向上线段终点 (需 > F) → 在 F 标记 线2+1买; 之后向下线段终点 X 满足
/// min(U1,U2) > X > 检测点价且新向下大段未成立 → 线2+2, 线2+3, ...; 另含两处「初始数据特定情况」分支。
///
/// ⚠️ 与 `detect_bigseg_cf_buy_markers` 的差别: 不接收轨道参数 —— 该级判定本就是**序列直判**
/// (与上级  严格对齐后的形态一致)。
pub fn detect_seg_cf_buy_markers(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<SecondPlusOneMarker> {
    if big_segments.is_empty() { return vec![]; }

    // 部件(线段)终点列表 (bar 升序)
    let down_segs: Vec<(usize, f64)> = segments.iter().enumerate().filter(|(_, s)| !s.2)
        .filter_map(|(i, _)| resolve_seg_end_point(i, segments, strokes)).collect();
    let up_segs: Vec<(usize, f64)> = segments.iter().enumerate().filter(|(_, s)| s.2)
        .filter_map(|(i, _)| resolve_seg_end_point(i, segments, strokes)).collect();

    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_seg, is_up)) in big_segments.iter().enumerate() {
        if is_up { continue; } // 只处理向下大段
        // 检测点 = 向下大段终点 (低点)
        let Some((dbe, d_price)) = resolve_seg_end_point(end_seg, segments, strokes) else { continue; };

        // F = 检测点后首个三底背离组 (连续 3 个向下线段) 的第三底
        let mut f: Option<(usize, f64)> = None;
        for i in 2..down_segs.len() {
            let (b0, l0) = down_segs[i - 2];
            let (_b1, l1) = down_segs[i - 1];
            let (b2, l2) = down_segs[i];
            if b0 >= dbe && l2 > l0 && l1 > l0 {
                f = Some((b2, l2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        // [窗口守卫] 三底第一底须落在 [检测点终点, 下一个同向(向下)大段终点) 窗口内
        let next_down_big = big_segments[k + 1..].iter().find(|s| !s.2)
            .and_then(|&(_, eb, _)| resolve_seg_end_point(eb, segments, strokes));
        if let Some((nb, _)) = next_down_big {
            if third_bar >= nb { continue; }
        }

        // C = 检测点后首个向上线段终点 (high)
        let Some((_, c_price)) = up_segs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
        if c_price > f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
            // 2+N 递推: 参考高点 = min(U1, U2) 固定; 新向下大段终点 bar 为界
            let second_up = up_segs.iter().filter(|(b, _)| *b > dbe).nth(1);
            let ref_high = second_up.map(|(_, h)| c_price.min(*h));
            if let Some(rh) = ref_high {
                let mut order = 2;
                for (xb, xl) in down_segs.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_down_big {
                        if *xb >= b { break; }
                    }
                    if rh > *xl && *xl > d_price {
                        markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                        order += 1;
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体1: host 首段 UP 且起点 > 0 → 两类游离线段都作补充检测点 ──
    if let Some(&(first_start, _, true)) = big_segments.first() {
        if first_start > 0 {
            let first_down_big_end = big_segments.iter().find(|s| !s.2)
                .and_then(|&(_, eb, _)| resolve_seg_end_point(eb, segments, strokes));
            for k in 0..first_start {
                let Some((dbe, d_price)) = resolve_seg_end_point(k, segments, strokes) else { continue; };
                let mut f: Option<(usize, f64)> = None;
                for i in 2..down_segs.len() {
                    let (b0, l0) = down_segs[i - 2];
                    let (_b1, l1) = down_segs[i - 1];
                    let (b2, l2) = down_segs[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };
                if let Some((nb, _)) = first_down_big_end {
                    if third_bar >= nb { continue; }
                }
                let Some((_, c_price)) = up_segs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                if c_price > f_price {
                    markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
                    let second_up = up_segs.iter().filter(|(b, _)| *b > dbe).nth(1);
                    let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                    if let Some(rh) = ref_high {
                        let mut order = 2;
                        for (xb, xl) in down_segs.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_down_big_end {
                                if *xb >= b { break; }
                            }
                            if rh > *xl && *xl > d_price {
                                markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                order += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体2: host 首段 UP 且起点 == 0 (无游离段) → 数据入口 = 向上大段 case2 起点低点 ──
    if let Some(&(first_start, _, true)) = big_segments.first() {
        if first_start == 0 {
            if let Some((dbe, d_price)) = crate::resolve_seg_start_point(big_segments[0].0, segments, strokes) {
                let mut bottoms: Vec<(usize, f64)> = Vec::with_capacity(down_segs.len() + 1);
                bottoms.push((dbe, d_price));
                bottoms.extend(down_segs.iter().filter(|(b, _)| *b > dbe).copied());
                let mut f: Option<(usize, f64)> = None;
                for i in 2..bottoms.len() {
                    let (b0, l0) = bottoms[i - 2];
                    let (_b1, l1) = bottoms[i - 1];
                    let (b2, l2) = bottoms[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {
                    let first_down_big_end = big_segments.iter().find(|s| !s.2)
                        .and_then(|&(_, eb, _)| resolve_seg_end_point(eb, segments, strokes));
                    let window_ok = match first_down_big_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        let c = up_segs.iter().find(|(b, _)| *b > dbe).copied();
                        if let Some((_, c_price)) = c {
                            if c_price > f_price {
                                markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
                                let second_up = up_segs.iter().filter(|(b, _)| *b > dbe).nth(1);
                                let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                                if let Some(rh) = ref_high {
                                    let mut order = 2;
                                    for (xb, xl) in down_segs.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_down_big_end {
                                            if *xb >= b { break; }
                                        }
                                        if rh > *xl && *xl > d_price {
                                            markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                            order += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    markers
}

/// 线2+N卖 (**完全镜像**: 检测点 = 向上大段终点; F = 三顶背离第三顶; C'<F' → 线2+1卖;
/// 之后向上线段终点 X 满足 max(C',C'+1) < X < 检测点价且新向上大段未成立 → 线2+2, 线2+3, ...)
pub fn detect_seg_cf_sell_markers(
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<SecondPlusOneMarker> {
    if big_segments.is_empty() { return vec![]; }

    let down_segs: Vec<(usize, f64)> = segments.iter().enumerate().filter(|(_, s)| !s.2)
        .filter_map(|(i, _)| resolve_seg_end_point(i, segments, strokes)).collect();
    let up_segs: Vec<(usize, f64)> = segments.iter().enumerate().filter(|(_, s)| s.2)
        .filter_map(|(i, _)| resolve_seg_end_point(i, segments, strokes)).collect();

    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_seg, is_up)) in big_segments.iter().enumerate() {
        if !is_up { continue; } // 只处理向上大段
        let Some((ube, d_price)) = resolve_seg_end_point(end_seg, segments, strokes) else { continue; };

        // F = 检测点后首个三顶背离组 (连续 3 个向上线段) 的第三顶
        let mut f: Option<(usize, f64)> = None;
        for i in 2..up_segs.len() {
            let (b0, h0) = up_segs[i - 2];
            let (_b1, h1) = up_segs[i - 1];
            let (b2, h2) = up_segs[i];
            if b0 >= ube && h2 < h0 && h1 < h0 {
                f = Some((b2, h2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        let next_up_big = big_segments[k + 1..].iter().find(|s| s.2)
            .and_then(|&(_, eb, _)| resolve_seg_end_point(eb, segments, strokes));
        if let Some((nb, _)) = next_up_big {
            if third_bar >= nb { continue; }
        }

        let Some((_, c_price)) = down_segs.iter().find(|(b, _)| *b > ube).copied() else { continue; };
        if c_price < f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
            let second_down = down_segs.iter().filter(|(b, _)| *b > ube).nth(1);
            let ref_low = second_down.map(|(_, l)| c_price.max(*l));
            if let Some(rl) = ref_low {
                let mut order = 2;
                for (xb, xh) in up_segs.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_up_big {
                        if *xb >= b { break; }
                    }
                    if rl < *xh && *xh < d_price {
                        markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                        order += 1;
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体1 卖镜像: host 首段 DOWN 且起点 > 0 ──
    if let Some(&(first_start, _, false)) = big_segments.first() {
        if first_start > 0 {
            let first_up_big_end = big_segments.iter().find(|s| s.2)
                .and_then(|&(_, eb, _)| resolve_seg_end_point(eb, segments, strokes));
            for k in 0..first_start {
                let Some((ube, d_price)) = resolve_seg_end_point(k, segments, strokes) else { continue; };
                let mut f: Option<(usize, f64)> = None;
                for i in 2..up_segs.len() {
                    let (b0, h0) = up_segs[i - 2];
                    let (_b1, h1) = up_segs[i - 1];
                    let (b2, h2) = up_segs[i];
                    if b0 >= ube && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };
                if let Some((nb, _)) = first_up_big_end {
                    if third_bar >= nb { continue; }
                }
                let Some((_, c_price)) = down_segs.iter().find(|(b, _)| *b > ube).copied() else { continue; };
                if c_price < f_price {
                    markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                    let second_down = down_segs.iter().filter(|(b, _)| *b > ube).nth(1);
                    let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                    if let Some(rl) = ref_low {
                        let mut order = 2;
                        for (xb, xh) in up_segs.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_up_big_end {
                                if *xb >= b { break; }
                            }
                            if rl < *xh && *xh < d_price {
                                markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                order += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体2 卖镜像: host 首段 DOWN 且起点 == 0 ──
    if let Some(&(first_start, _, false)) = big_segments.first() {
        if first_start == 0 {
            if let Some((ube, d_price)) = crate::resolve_seg_start_point(big_segments[0].0, segments, strokes) {
                let mut tops: Vec<(usize, f64)> = Vec::with_capacity(up_segs.len() + 1);
                tops.push((ube, d_price));
                tops.extend(up_segs.iter().filter(|(b, _)| *b > ube).copied());
                let mut f: Option<(usize, f64)> = None;
                for i in 2..tops.len() {
                    let (b0, h0) = tops[i - 2];
                    let (_b1, h1) = tops[i - 1];
                    let (b2, h2) = tops[i];
                    if b0 >= ube && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {
                    let first_up_big_end = big_segments.iter().find(|s| s.2)
                        .and_then(|&(_, eb, _)| resolve_seg_end_point(eb, segments, strokes));
                    let window_ok = match first_up_big_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        let c = down_segs.iter().find(|(b, _)| *b > ube).copied();
                        if let Some((_, c_price)) = c {
                            if c_price < f_price {
                                markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                                let second_down = down_segs.iter().filter(|(b, _)| *b > ube).nth(1);
                                let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                                if let Some(rl) = ref_low {
                                    let mut order = 2;
                                    for (xb, xh) in up_segs.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_up_big_end {
                                            if *xb >= b { break; }
                                        }
                                        if rl < *xh && *xh < d_price {
                                            markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                            order += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    markers
}

// ===== 笔2+N买/卖 + 笔级撤回过滤 () =====
/// 笔2+N买 (宿主 = 线段, 部件 = 笔; 与 `detect_seg_cf_buy_markers` **逐行同构**, 层级再降一级):
/// 检测点 = 向下线段终点; 其后首个三底背离组 (连续 3 个向下笔, 第3底>第1底 且 第2底>第1底) 的第三底 F,
/// C = 检测点后首个向上笔终点 (需 > F) → 在 F 标记 笔2+1买; 之后向下笔终点 X 满足
/// min(U1,U2) > X > 检测点价且新向下线段未成立 → 笔2+2, 笔2+3, ...; 另含两处「初始数据特定情况」分支。
///
/// ⚠️ 与 `detect_seg_cf_buy_markers` 同款: 不接收轨道参数 —— 该级判定本就是**序列直判**。
pub fn detect_stroke_cf_buy_markers(
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<SecondPlusOneMarker> {
    if segments.is_empty() { return vec![]; }

    // 部件(笔)终点列表 (bar 升序)
    let down_parts: Vec<(usize, f64)> = strokes.iter().filter(|s| !s.is_up).map(|s| (s.end_bar, s.end_price)).collect();
    let up_parts: Vec<(usize, f64)> = strokes.iter().filter(|s| s.is_up).map(|s| (s.end_bar, s.end_price)).collect();

    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, part_end, is_up)) in segments.iter().enumerate() {
        if is_up { continue; } // 只处理向下线段
        // 检测点 = 向下线段终点 (低点)
        let Some((dbe, d_price)) = crate::resolve_stroke_end_point(part_end, strokes) else { continue; };

        // F = 检测点后首个三底背离组 (连续 3 个向下笔) 的第三底
        let mut f: Option<(usize, f64)> = None;
        for i in 2..down_parts.len() {
            let (b0, l0) = down_parts[i - 2];
            let (_b1, l1) = down_parts[i - 1];
            let (b2, l2) = down_parts[i];
            if b0 >= dbe && l2 > l0 && l1 > l0 {
                f = Some((b2, l2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        // [窗口守卫] 三底第一底须落在 [检测点终点, 下一个同向(向下)线段终点) 窗口内
        let next_down_seg = segments[k + 1..].iter().find(|s| !s.2)
            .and_then(|&(_, eb, _)| crate::resolve_stroke_end_point(eb, strokes));
        if let Some((nb, _)) = next_down_seg {
            if third_bar >= nb { continue; }
        }

        // C = 检测点后首个向上笔终点 (high)
        let Some((_, c_price)) = up_parts.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
        if c_price > f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
            // 2+N 递推: 参考高点 = min(U1, U2) 固定; 新向下线段终点 bar 为界
            let second_up = up_parts.iter().filter(|(b, _)| *b > dbe).nth(1);
            let ref_high = second_up.map(|(_, h)| c_price.min(*h));
            if let Some(rh) = ref_high {
                let mut order = 2;
                for (xb, xl) in down_parts.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_down_seg {
                        if *xb >= b { break; }
                    }
                    if rh > *xl && *xl > d_price {
                        markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                        order += 1;
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体1: host 首段 UP 且起点 > 0 → 开头游离笔作补充检测点 ──
    if let Some(&(first_start, _, true)) = segments.first() {
        if first_start > 0 {
            let first_down_seg_end = segments.iter().find(|s| !s.2)
                .and_then(|&(_, eb, _)| crate::resolve_stroke_end_point(eb, strokes));
            for k in 0..first_start {
                let Some((dbe, d_price)) = crate::resolve_stroke_end_point(k, strokes) else { continue; };
                let mut f: Option<(usize, f64)> = None;
                for i in 2..down_parts.len() {
                    let (b0, l0) = down_parts[i - 2];
                    let (_b1, l1) = down_parts[i - 1];
                    let (b2, l2) = down_parts[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };
                if let Some((nb, _)) = first_down_seg_end {
                    if third_bar >= nb { continue; }
                }
                let Some((_, c_price)) = up_parts.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                if c_price > f_price {
                    markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
                    let second_up = up_parts.iter().filter(|(b, _)| *b > dbe).nth(1);
                    let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                    if let Some(rh) = ref_high {
                        let mut order = 2;
                        for (xb, xl) in down_parts.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_down_seg_end {
                                if *xb >= b { break; }
                            }
                            if rh > *xl && *xl > d_price {
                                markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                order += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体2: host 首段 UP 且起点 == 0 (无游离笔) → 数据入口 = 向上线段 case2 起点低点 ──
    if let Some(&(first_start, _, true)) = segments.first() {
        if first_start == 0 {
            if let Some((dbe, d_price)) = crate::resolve_stroke_start_point(segments[0].0, strokes) {
                let mut bottoms: Vec<(usize, f64)> = Vec::with_capacity(down_parts.len() + 1);
                bottoms.push((dbe, d_price));
                bottoms.extend(down_parts.iter().filter(|(b, _)| *b > dbe).copied());
                let mut f: Option<(usize, f64)> = None;
                for i in 2..bottoms.len() {
                    let (b0, l0) = bottoms[i - 2];
                    let (_b1, l1) = bottoms[i - 1];
                    let (b2, l2) = bottoms[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {
                    let first_down_seg_end = segments.iter().find(|s| !s.2)
                        .and_then(|&(_, eb, _)| crate::resolve_stroke_end_point(eb, strokes));
                    let window_ok = match first_down_seg_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        if let Some((_, c_price)) = up_parts.iter().find(|(b, _)| *b > dbe).copied() {
                            if c_price > f_price {
                                markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
                                let second_up = up_parts.iter().filter(|(b, _)| *b > dbe).nth(1);
                                let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                                if let Some(rh) = ref_high {
                                    let mut order = 2;
                                    for (xb, xl) in down_parts.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_down_seg_end {
                                            if *xb >= b { break; }
                                        }
                                        if rh > *xl && *xl > d_price {
                                            markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                            order += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    markers
}

/// 笔2+N卖 (**完全镜像**: 检测点 = 向上线段终点; F = 三顶背离第三顶; C'<F' → 笔2+1卖;
/// 之后向上笔终点 X 满足 max(C',C'+1) < X < 检测点价且新向上线段未成立 → 笔2+2, 笔2+3, ...)
pub fn detect_stroke_cf_sell_markers(
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<SecondPlusOneMarker> {
    if segments.is_empty() { return vec![]; }

    let down_parts: Vec<(usize, f64)> = strokes.iter().filter(|s| !s.is_up).map(|s| (s.end_bar, s.end_price)).collect();
    let up_parts: Vec<(usize, f64)> = strokes.iter().filter(|s| s.is_up).map(|s| (s.end_bar, s.end_price)).collect();

    let mut markers: Vec<SecondPlusOneMarker> = Vec::new();
    for (k, &(_, part_end, is_up)) in segments.iter().enumerate() {
        if !is_up { continue; } // 只处理向上线段
        let Some((ube, d_price)) = crate::resolve_stroke_end_point(part_end, strokes) else { continue; };

        // F = 检测点后首个三顶背离组 (连续 3 个向上笔) 的第三顶
        let mut f: Option<(usize, f64)> = None;
        for i in 2..up_parts.len() {
            let (b0, h0) = up_parts[i - 2];
            let (_b1, h1) = up_parts[i - 1];
            let (b2, h2) = up_parts[i];
            if b0 >= ube && h2 < h0 && h1 < h0 {
                f = Some((b2, h2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        let next_up_seg = segments[k + 1..].iter().find(|s| s.2)
            .and_then(|&(_, eb, _)| crate::resolve_stroke_end_point(eb, strokes));
        if let Some((nb, _)) = next_up_seg {
            if third_bar >= nb { continue; }
        }

        let Some((_, c_price)) = down_parts.iter().find(|(b, _)| *b > ube).copied() else { continue; };
        if c_price < f_price {
            markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
            let second_down = down_parts.iter().filter(|(b, _)| *b > ube).nth(1);
            let ref_low = second_down.map(|(_, l)| c_price.max(*l));
            if let Some(rl) = ref_low {
                let mut order = 2;
                for (xb, xh) in up_parts.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_up_seg {
                        if *xb >= b { break; }
                    }
                    if rl < *xh && *xh < d_price {
                        markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                        order += 1;
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体1 卖镜像: host 首段 DOWN 且起点 > 0 ──
    if let Some(&(first_start, _, false)) = segments.first() {
        if first_start > 0 {
            let first_up_seg_end = segments.iter().find(|s| s.2)
                .and_then(|&(_, eb, _)| crate::resolve_stroke_end_point(eb, strokes));
            for k in 0..first_start {
                let Some((ube, d_price)) = crate::resolve_stroke_end_point(k, strokes) else { continue; };
                let mut f: Option<(usize, f64)> = None;
                for i in 2..up_parts.len() {
                    let (b0, h0) = up_parts[i - 2];
                    let (_b1, h1) = up_parts[i - 1];
                    let (b2, h2) = up_parts[i];
                    if b0 >= ube && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };
                if let Some((nb, _)) = first_up_seg_end {
                    if third_bar >= nb { continue; }
                }
                let Some((_, c_price)) = down_parts.iter().find(|(b, _)| *b > ube).copied() else { continue; };
                if c_price < f_price {
                    markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                    let second_down = down_parts.iter().filter(|(b, _)| *b > ube).nth(1);
                    let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                    if let Some(rl) = ref_low {
                        let mut order = 2;
                        for (xb, xh) in up_parts.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_up_seg_end {
                                if *xb >= b { break; }
                            }
                            if rl < *xh && *xh < d_price {
                                markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                order += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 初始数据特定情况·变体2 卖镜像: host 首段 DOWN 且起点 == 0 ──
    if let Some(&(first_start, _, false)) = segments.first() {
        if first_start == 0 {
            if let Some((ube, d_price)) = crate::resolve_stroke_start_point(segments[0].0, strokes) {
                let mut tops: Vec<(usize, f64)> = Vec::with_capacity(up_parts.len() + 1);
                tops.push((ube, d_price));
                tops.extend(up_parts.iter().filter(|(b, _)| *b > ube).copied());
                let mut f: Option<(usize, f64)> = None;
                for i in 2..tops.len() {
                    let (b0, h0) = tops[i - 2];
                    let (_b1, h1) = tops[i - 1];
                    let (b2, h2) = tops[i];
                    if b0 >= ube && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {
                    let first_up_seg_end = segments.iter().find(|s| s.2)
                        .and_then(|&(_, eb, _)| crate::resolve_stroke_end_point(eb, strokes));
                    let window_ok = match first_up_seg_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        if let Some((_, c_price)) = down_parts.iter().find(|(b, _)| *b > ube).copied() {
                            if c_price < f_price {
                                markers.push(SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                                let second_down = down_parts.iter().filter(|(b, _)| *b > ube).nth(1);
                                let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                                if let Some(rl) = ref_low {
                                    let mut order = 2;
                                    for (xb, xh) in up_parts.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_up_seg_end {
                                            if *xb >= b { break; }
                                        }
                                        if rl < *xh && *xh < d_price {
                                            markers.push(SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                            order += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    markers
}

// ═══ 大段级 3+N 专用撤回过滤 (自身 bar 计入候选池) ═══
// ===== 大段 3+N买/卖 撤回过滤 () =====
/// 大段 **3+N买/3+N卖** 的撤回过滤器 —— 与 [`apply_sup_retreat_filter`] 的**大段三买那一支逐行同构**
/// （同一份窗口、同一个 `x` / `cut` / `eb`、同一套 retain 谓词），只是作用对象换成 3+N 家族。
///
/// 🔴 ** 修正**：`x` = 前驱区间内**最后一个买点**，
///    必须把**3+N 自己的 bar 也算进候选池** —— 与「2+N买」同规（2+N 位于 `plus_markers`，
///    本来就在池内）。此前漏算 ⇒ 当 3+N 恰好是区间内最晚的买点时，`x` 偏小、3+N 反而被自己的
///    撤回规则删掉（欧美30m 2025-05-13 02:30 / 2025-06-19 14:30 两例即如此）。
///    ⚠️ 本函数只对 3+N 列表做 retain ⇒ **不会改变 二买/三买/2+N 既有信号**。
///
/// ⚠️ 为什么单独一支而不改 `apply_sup_retreat_filter` 签名：后者有 ~110 个调用点，加参数会波及全仓。
pub fn apply_sup_retreat_filter_3n(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    second_markers: &[SecondMarker],
    third_markers: &[BigSegThirdMarker],
    plus_markers: &[SecondPlusOneMarker],
    mut markers: Vec<crate::zhongshu::BigSeg3NMarker>,
) -> Vec<crate::zhongshu::BigSeg3NMarker> {
    // 买点撤回: 每个向下高级段成立 (索引>0 才有前驱向上高级段)
    for d_idx in (1..sups.len()).filter(|&i| !sups[i].2) {
        if let Some((sb, eb)) = sup_range(sups[d_idx - 1], bigs, segs, strokes) {
            let mut buy_bars: Vec<usize> = Vec::new();
            for m in second_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in third_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in plus_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            // ★ 3+N 自己的 bar 也计入候选池 (与 2+N 同规; 见上方修正说明)
            for m in markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            let cut = sup_range(sups[d_idx], bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match buy_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut),
                None => markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut),
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
            // ★ 3+N 自己的 bar 也计入候选池 (卖点镜像)
            for m in markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            let cut = sup_range(sups[u_idx], bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match sell_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut),
                None => markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut),
            }
        }
    }
    markers
}

