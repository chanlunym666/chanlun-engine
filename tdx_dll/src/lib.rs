//! slzs_chanlun — 缠论通达信64位DLL插件 (⚠️ TDX仅支持64位)
//! ⚠️ 通达信是 64 位 (x86_64) / MT4 是 32 位 (i686) — 永不混淆
//! 基于 chanlun_lean_lib 纯Rust算法核心
//!
//! TDX调用约定:
//!   TDXDLL3(编号, HIGH, LOW, mode)
//!   编号1=分型, 2=笔, 3=线段, 4=大段, 35=高级段
//!   轨道: 5-6=笔轨, 7-8=线段轨, 9-10=大段轨
//!   37=二买/二卖/三买/三卖文字 (mode 1=二买 2=二卖 3=三买 4=三卖)
//!   39=中枢ZG, 40=中枢ZD, 41=中枢开始标记, 42=中枢结束标记
//!   46=设置笔 case 参数, 49=设置线段~高级段 case 参数 (mode: 0全开/3关case3/4关case4/5关case34)

use std::os::raw::{c_int, c_float, c_ushort};

use chanlun_lean_lib::{ChanlunPipeline, StrokeCases, guidao, zhongshu};

// ── TDX 接口结构体 ──
#[repr(C, packed(1))]
pub struct PluginTCalcFuncInfo {
    n_func_mark: c_ushort,
    p_call_func: Option<PlugInFunc>,
}

type PlugInFunc = unsafe extern "C" fn(c_int, *mut c_float, *mut c_float, *mut c_float, *mut c_float);

// ── Pipeline 缓存 ──
use std::sync::Mutex;
use std::sync::Arc;
// Arc 共享 (2026-08-20 对齐 flowsurface P3/MT4: 命中/存缓存均免 from_parts 9 Vec 克隆; 全局 Mutex 跨线程故用 Arc 非 Rc)
// 2026-09-05 参数化: 缓存键含 (笔, 层) case 参数; case 状态存线程局部 TL_CASES —
//   TDX GUI 各图表重算同线程且公式设置行先于数据行执行; 其他线程不调 46/49 → 恒全开.
static PIPELINE_CACHE: Mutex<Option<((Vec<f64>, Vec<f64>, u8, u8), Arc<ChanlunPipeline>)>> = Mutex::new(None);

thread_local! {
    /// 用户 case 参数 (0/3/4/5, 默认 0=全开): (笔, 线段~高级段)
    static TL_CASES: std::cell::RefCell<(u8, u8)> = const { std::cell::RefCell::new((0u8, 0u8)) };
}

/// 计算/命中管线 — 返回 Arc 共享引用 (对齐 flowsurface P3/MT4, 免 from_parts 9 Vec 克隆)
fn get_pipeline(highs: Vec<f64>, lows: Vec<f64>) -> Arc<ChanlunPipeline> {
    let (sc, lc) = TL_CASES.with(|c| *c.borrow());
    let mut cache = PIPELINE_CACHE.lock().unwrap();
    if let Some((ref key, ref pipeline)) = *cache {
        if key.0.len() == highs.len() && key.1.len() == lows.len()
            && key.0.first() == highs.first() && key.1.first() == lows.first()
            && key.0.last() == highs.last() && key.1.last() == lows.last()
            && key.2 == sc && key.3 == lc
        {
            return Arc::clone(pipeline);
        }
    }
    let rc = Arc::new(ChanlunPipeline::new_with_cases(
        highs.clone(), lows.clone(),
        StrokeCases::from_param(sc), StrokeCases::from_param(lc),
    ));
    *cache = Some(((highs, lows, sc, lc), Arc::clone(&rc)));
    rc
}

// ── 缠论 case 参数设置 (2026-09-05; 默认 0=全开=历史行为零回归) ──
// mark 46 = 笔参数, mark 49 = 线段~高级段参数 (两函数分开注册以区分槽位)
// mode 值: 0全开 / 3关case3 / 4关case4 / 5关case34; 参数不变不动作, 变化才清管线缓存
unsafe fn apply_case_param(slot: usize, m: u8) {
    let changed = TL_CASES.with(|c| {
        let mut t = c.borrow_mut();
        let cur = if slot == 0 { t.0 } else { t.1 };
        if cur != m {
            if slot == 0 { t.0 = m; } else { t.1 = m; }
            true
        } else {
            false
        }
    });
    if changed {
        *PIPELINE_CACHE.lock().unwrap() = None; // 参数变化 → 强制重建管线
    }
}

unsafe extern "C" fn set_stroke_cases_fn(
    data_len: c_int,
    out: *mut c_float,
    _highs_in: *mut c_float,
    _lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let m = if mode.is_null() { 0 } else { *mode as u8 };
    apply_case_param(0, m);
    write_output(out, data_len as usize, &[]);
}

unsafe extern "C" fn set_level_cases_fn(
    data_len: c_int,
    out: *mut c_float,
    _highs_in: *mut c_float,
    _lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let m = if mode.is_null() { 0 } else { *mode as u8 };
    apply_case_param(1, m);
    write_output(out, data_len as usize, &[]);
}

// ── 辅助函数 ──
unsafe fn read_floats(ptr: *mut c_float, len: usize) -> Vec<f64> {
    let slice = std::slice::from_raw_parts(ptr, len);
    slice.iter().map(|&v| v as f64).collect()
}

unsafe fn write_output(out: *mut c_float, data_len: usize, values: &[(usize, f32)]) {
    let slice = std::slice::from_raw_parts_mut(out, data_len);
    for v in slice.iter_mut() { *v = 0.0; }
    for &(idx, val) in values {
        if idx < data_len { *slice.get_unchecked_mut(idx) = val; }
    }
}

// ── 分型 (valid_fractals: 全部过滤后的分型，非仅笔端点) ──
// 底分型: -1, 顶分型: +1
unsafe extern "C" fn fractals_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32;

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let vf = &pipeline.valid_fractals;

    let mut values: Vec<(usize, f32)> = Vec::new();
    for f in vf {
        if m == 1 {
            let dir: f32 = if f.is_top { 1.0 } else { -1.0 };
            values.push((f.bar_index, dir));
        } else {
            values.push((f.bar_index, f.price as f32));
        }
    }
    write_output(out, dl, &values);
}

// ── 笔 (Strokes) ──
// 对齐 Flowsurface chart/kline.rs L922-947 与 build_strokes L222-233:
//   上升笔: start=底分型(-1) → end=顶分型(+1)
//   下降笔: start=顶分型(+1) → end=底分型(-1)
//   线型: 实线(Solid), 颜色: #006400 DarkGreen, 宽度2
unsafe extern "C" fn strokes_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32;

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let strokes = &pipeline.strokes;

    let mut values: Vec<(usize, f32)> = Vec::new();
    for s in strokes {
        if m == 1 {
            // 上升笔: s.is_up = true
            //   start处是底分型 → dir=-1
            //   end处是顶分型   → dir=+1
            let start_dir: f32 = if s.is_up { -1.0 } else { 1.0 };
            let end_dir: f32   = if s.is_up { 1.0 } else { -1.0 };
            values.push((s.start_bar, start_dir));
            values.push((s.end_bar, end_dir));
        } else {
            values.push((s.start_bar, s.start_price as f32));
            values.push((s.end_bar, s.end_price as f32));
        }
    }
    write_output(out, dl, &values);
}

// ── 线段 (Segments) ──
// 对齐 Flowsurface chart/kline.rs L897-920, 色#BA55D3, 宽3px
// segment = (start_stroke_idx, end_stroke_idx, is_up)
// 方向: ±100 区分于笔
unsafe extern "C" fn segments_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32;
    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    let mut values: Vec<(usize, f32)> = Vec::new();
    for &(start_si, end_si, is_up) in segs {
        if start_si < strokes.len() && end_si < strokes.len() {
            let ss = &strokes[start_si];
            let se = &strokes[end_si];
            if m == 1 {
                let sd: f32 = if is_up { -100.0 } else { 100.0 };
                let ed: f32 = if is_up { 100.0 } else { -100.0 };
                values.push((ss.start_bar, sd));
                values.push((se.end_bar, ed));
            } else {
                values.push((ss.start_bar, ss.start_price as f32));
                values.push((se.end_bar, se.end_price as f32));
            }
        }
    }
    write_output(out, dl, &values);
}

// ── 大段 (Big Segments) ──
// 对齐 Flowsurface chart/kline.rs L860-894, 色#00BFFF, 宽4px
// big_segment索引→segments→strokes (两层解析)
// 方向: ±1000 区分于笔和段
unsafe extern "C" fn big_segments_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32;
    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    let bigs = &pipeline.big_segments;
    let mut values: Vec<(usize, f32)> = Vec::new();
    for &(bs_start, bs_end, is_up) in bigs {
        if let (Some(seg_s), Some(seg_e)) = (segs.get(bs_start), segs.get(bs_end)) {
            let rs = seg_s.0;
            let re = seg_e.1;
            if rs < strokes.len() && re < strokes.len() {
                let ss = &strokes[rs];
                let se = &strokes[re];
                if m == 1 {
                    let sd: f32 = if is_up { -1000.0 } else { 1000.0 };
                    let ed: f32 = if is_up { 1000.0 } else { -1000.0 };
                    values.push((ss.start_bar, sd));
                    values.push((se.end_bar, ed));
                } else {
                    values.push((ss.start_bar, ss.start_price as f32));
                    values.push((se.end_bar, se.end_price as f32));
                }
            }
        }
    }
    write_output(out, dl, &values);
}

// ── 高级段 (Superior Segments, 大段→高级段投影, Case2+Case3+延伸, Case4已移除) ──
// mark 35, mode=1: 方向(±10000), mode=2: 价格
// 色#D3D3D3(LightGray), LINETHICK2, 实线
// 三级解析: 高级段→大段→线段→笔→价格坐标
unsafe extern "C" fn superior_segments_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32;
    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    let bigs = &pipeline.big_segments;
    let sups = &pipeline.superior_segments;
    let mut values: Vec<(usize, f32)> = Vec::new();
    for &(sup_start, sup_end, is_up) in sups {
        // 三级解析: 高级段→大段→线段→笔
        if let (Some(big_s), Some(big_e)) = (bigs.get(sup_start), bigs.get(sup_end)) {
            if let (Some(seg_s), Some(seg_e)) = (segs.get(big_s.0), segs.get(big_e.1)) {
                let rs = seg_s.0; // stroke_start_idx
                let re = seg_e.1; // stroke_end_idx
                if rs < strokes.len() && re < strokes.len() {
                    let ss = &strokes[rs];
                    let se = &strokes[re];
                    if m == 1 {
                        let sd: f32 = if is_up { -10000.0 } else { 10000.0 };
                        let ed: f32 = if is_up { 10000.0 } else { -10000.0 };
                        values.push((ss.start_bar, sd));
                        values.push((se.end_bar, ed));
                    } else {
                        values.push((ss.start_bar, ss.start_price as f32));
                        values.push((se.end_bar, se.end_price as f32));
                    }
                }
            }
        }
    }
    write_output(out, dl, &values);
}

// ── 笔轨道 (Stroke Bands) ──
// mark 5=上轨(DimGray), mark 6=下轨(DimGray)
/// 对齐 Flowsurface kline.rs L982-1030: COLOR696969, 1px, step-style
/// Case1 (三重顶/底) + Case2 (1.618扩张) + Case3 (转多/转空) + Case4 (V信号) + 追踪
unsafe extern "C" fn stroke_band_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32; // 5=上轨, 6=下轨

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let ff = &pipeline.final_fractals;
    let is_upper = m == 5;

    // 提取同向笔
    let mut strokes: Vec<(f64, f64, usize)> = Vec::new();
    for i in 1..ff.len() {
        let prev = &ff[i-1]; let curr = &ff[i];
        if is_upper {
            if !prev.is_top && curr.is_top { strokes.push((prev.price, curr.price, curr.bar_index)); }
        } else {
            if prev.is_top && !curr.is_top { strokes.push((prev.price, curr.price, curr.bar_index)); }
        }
    }
    // Case1: triple divergence
    let mut bp: Vec<(usize, f64)> = Vec::new();
    if strokes.len() >= 3 {
        for i in 2..strokes.len() {
            let e0 = strokes[i-2].1; let b0 = strokes[i-2].2;
            let e1 = strokes[i-1].1; let e2 = strokes[i].1;
            if is_upper { if e2 < e0 && e1 < e0 { bp.push((b0, e0)); } }
            else { if e2 > e0 && e1 > e0 { bp.push((b0, e0)); } }
        }
    }
    // Case2: 1.618 expansion — 提取反向笔检测扩张 (内联实现, 与库guidao.rs L86-223完全一致)
    let mut case2_strokes: Vec<(f64, f64, usize)> = Vec::new(); // (high, low, bar)
    for i in 1..ff.len() {
        let prev = &ff[i-1]; let curr = &ff[i];
        if is_upper {
            // 上轨Case2: 下降笔 (top→bottom), bar=顶分型bar
            if prev.is_top && !curr.is_top {
                case2_strokes.push((prev.price, curr.price, prev.bar_index));
            }
        } else {
            // 下轨Case2: 上升笔 (bottom→top), bar=顶分型bar
            if !prev.is_top && curr.is_top {
                case2_strokes.push((curr.price, prev.price, curr.bar_index));
            }
        }
    }
    if case2_strokes.len() >= 2 {
        for i in 1..case2_strokes.len() {
            let (fh, fl, fb) = case2_strokes[i-1];
            let (sh, sl, _) = case2_strokes[i];
            let first_space = fh - fl;
            let second_space = sh - sl;
            if second_space >= first_space * 1.618 {
                if is_upper && fh > sh {
                    bp.push((fb, fh));
                } else if !is_upper && fl < sl {
                    bp.push((fb, fl));
                }
            }
        }
    }
    // Case3: 转多/转空 — 内联实现, 与库guidao.rs L320-397 + L501-536完全一致
    let pl_strokes = &pipeline.strokes;
    let n_strokes = pl_strokes.len();
    if n_strokes >= 2 {
        // detect_turn_signals 内联
        let mut turn_signals: Vec<(usize, usize, bool, f64, f64)> = Vec::new();
        // (stroke_idx, end_stroke_idx, is_turn_short, a_high, a_low)
        for i in 0..n_strokes {
            // check_up/down_segment_case2 内联 (与库lib.rs L247-281完全一致)
            let (ok, end_idx) = if pl_strokes[i].is_up {
                let mut result = (false, i);
                if i + 1 < n_strokes {
                    let first_low = pl_strokes[i].start_price;
                    let mut prev_high = pl_strokes[i].end_price;
                    for j in (i + 1)..n_strokes {
                        if pl_strokes[j].is_up {
                            let cur_low = pl_strokes[j].start_price;
                            let cur_high = pl_strokes[j].end_price;
                            if cur_low <= first_low { break; }
                            if cur_high > prev_high { result = (true, j); break; }
                            prev_high = cur_high;
                        }
                    }
                }
                result
            } else {
                let mut result = (false, i);
                if i + 1 < n_strokes {
                    let first_high = pl_strokes[i].start_price;
                    let mut prev_low = pl_strokes[i].end_price;
                    for j in (i + 1)..n_strokes {
                        if !pl_strokes[j].is_up {
                            let cur_high = pl_strokes[j].start_price;
                            let cur_low = pl_strokes[j].end_price;
                            if cur_high >= first_high { break; }
                            if cur_low < prev_low { result = (true, j); break; }
                            prev_low = cur_low;
                        }
                    }
                }
                result
            };
            if !ok || end_idx >= n_strokes { continue; }
            // Compute a_high/a_low over strokes[i..=end_idx]
            let mut a_high = f64::NEG_INFINITY;
            let mut a_low = f64::INFINITY;
            for k in i..=end_idx {
                let s = &pl_strokes[k];
                a_high = a_high.max(s.start_price).max(s.end_price);
                a_low = a_low.min(s.start_price).min(s.end_price);
            }
            let start_price = pl_strokes[i].start_price;
            let end_bar = pl_strokes[end_idx].end_bar;
            let search_stop = match pl_strokes.get(end_idx + 1) {
                Some(s) if s.is_up != pl_strokes[i].is_up => s.end_bar,
                _ => continue,
            };
            let search_start = end_bar + 1;
            if pl_strokes[i].is_up {
                // 转空: forward search for low ≤ start_price
                for bar in search_start..=search_stop {
                    if bar < lows.len() && lows[bar] <= start_price {
                        turn_signals.push((i, end_idx, true, a_high, a_low));
                        break;
                    }
                }
            } else {
                // 转多: forward search for high ≥ start_price
                for bar in search_start..=search_stop {
                    if bar < highs.len() && highs[bar] >= start_price {
                        turn_signals.push((i, end_idx, false, a_high, a_low));
                        break;
                    }
                }
            }
        }
        // calc_upper/lower_band_case3 内联
        for (_, end_stroke_idx, is_turn_short, ts_a_high, ts_a_low) in &turn_signals {
            if let Some(s) = pl_strokes.get(*end_stroke_idx) {
                if is_upper && *is_turn_short {
                    bp.push((s.end_bar, *ts_a_high));
                } else if !is_upper && !*is_turn_short {
                    bp.push((s.end_bar, *ts_a_low));
                }
            }
        }
        // Case4: V信号反弹 — 内联实现, 与库guidao.rs L400-496 + L538-586完全一致
        // detect_v_signals 内联
        let ts_count = turn_signals.len();
        if ts_count > 0 && n_strokes >= 2 {
            let mut v_signals: Vec<(usize, usize, bool, f64, f64)> = Vec::new();
            // (stroke_idx, end_stroke_idx, is_turn_short, a_high, a_low)
            let mut seen_keys: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
            for &(ts_si, ts_end_si, ts_is_short, ts_ah, ts_al) in &turn_signals {
                let first_idx = ts_end_si + 1;
                // Fallback: not enough strokes for 45-56 pattern
                if first_idx >= n_strokes.saturating_sub(1) {
                    let cmp_price = pl_strokes[ts_si].start_price;
                    if !ts_is_short {
                        // 转多 fallback: scan up-strokes, then bars for low ≤ cmp_price
                        for fi in first_idx..n_strokes {
                            if pl_strokes[fi].is_up {
                                let up_end_bar = pl_strokes[fi].end_bar;
                                let search_start = up_end_bar.saturating_add(1).min(lows.len());
                                if search_start < lows.len() {
                                    if lows[search_start..].iter().any(|&v| v <= cmp_price) {
                                        let key = (ts_si, ts_end_si);
                                        if seen_keys.insert(key) {
                                            v_signals.push((ts_si, ts_end_si, ts_is_short, ts_ah, ts_al));
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    } else {
                        // 转空 fallback: scan down-strokes, then bars for high ≥ cmp_price
                        for fi in first_idx..n_strokes {
                            if !pl_strokes[fi].is_up {
                                let down_end_bar = pl_strokes[fi].end_bar;
                                let search_start = down_end_bar.saturating_add(1).min(highs.len());
                                if search_start < highs.len() {
                                    if highs[search_start..].iter().any(|&v| v >= cmp_price) {
                                        let key = (ts_si, ts_end_si);
                                        if seen_keys.insert(key) {
                                            v_signals.push((ts_si, ts_end_si, ts_is_short, ts_ah, ts_al));
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                    continue;
                }
                // Main path: 45-56 pattern
                if ts_is_short {
                    // 转空 → V多: first stroke=down, second stroke=up, 6>4
                    if pl_strokes[first_idx].is_up { continue; }
                    let second_idx = first_idx + 1;
                    if second_idx >= n_strokes { continue; }
                    if !pl_strokes[second_idx].is_up { continue; }
                    if pl_strokes[second_idx].end_price > pl_strokes[ts_end_si].end_price {
                        let key = (ts_si, ts_end_si);
                        if seen_keys.insert(key) {
                            v_signals.push((ts_si, ts_end_si, ts_is_short, ts_ah, ts_al));
                        }
                    }
                } else {
                    // 转多 → V空: first stroke=up, second stroke=down, 6<4
                    if !pl_strokes[first_idx].is_up { continue; }
                    let second_idx = first_idx + 1;
                    if second_idx >= n_strokes { continue; }
                    if pl_strokes[second_idx].is_up { continue; }
                    if pl_strokes[second_idx].end_price < pl_strokes[ts_end_si].end_price {
                        let key = (ts_si, ts_end_si);
                        if seen_keys.insert(key) {
                            v_signals.push((ts_si, ts_end_si, ts_is_short, ts_ah, ts_al));
                        }
                    }
                }
            }
            // calc_upper/lower_band_case4 内联
            for (_, vs_end_si, vs_is_short, _, _) in &v_signals {
                let first_idx = vs_end_si + 1;
                if let Some(s) = pl_strokes.get(first_idx) {
                    if is_upper && !*vs_is_short && s.is_up {
                        // 上轨Case4: 转多→V空, 取上升笔终点
                        bp.push((s.end_bar, s.end_price));
                    } else if !is_upper && *vs_is_short && !s.is_up {
                        // 下轨Case4: 转空→V多, 取下降笔终点
                        bp.push((s.end_bar, s.end_price));
                    }
                }
            }
        }
    }
    // Tracking: extend band as K线 breaks beyond, 延续到最新K线
    if !bp.is_empty() {
        bp.sort_by_key(|(b, _)| *b);
        let mut tracked: Vec<(usize, f64)> = Vec::new();
        let price_arr = if is_upper { &highs } else { &lows };
        for i in 0..bp.len() {
            let (b, v) = bp[i];
            tracked.push((b, v));
            let next_bar = if i + 1 < bp.len() { bp[i+1].0.min(dl) } else { dl };
            let mut cur_val = v;
            for bar in (b + 1)..next_bar {
                if is_upper && price_arr[bar] > cur_val {
                    cur_val = price_arr[bar]; tracked.push((bar, cur_val));
                } else if !is_upper && price_arr[bar] < cur_val {
                    cur_val = price_arr[bar]; tracked.push((bar, cur_val));
                }
            }
        }
        bp = tracked;
        bp.sort_by_key(|(b, _)| *b);
    }
    // Step-style output
    let mut values: Vec<(usize, f32)> = Vec::new();
    if !bp.is_empty() {
        let mut cur_val = bp[0].1 as f32;
        let mut pt_idx = 1usize;
        for bar in 0..dl {
            while pt_idx < bp.len() && bp[pt_idx].0 <= bar { cur_val = bp[pt_idx].1 as f32; pt_idx += 1; }
            if bar >= bp[0].0 { values.push((bar, cur_val)); }
        }
    }
    write_output(out, dl, &values);
}

// ── 线段轨道 (Segment Bands) ──
// mark 7=上轨(COLORFF00FF), mark 8=下轨(COLORFF00FF)
/// 对齐 Flowsurface: Case1 (三重顶/底) + Case2 (1.618扩张) + Case3 (转多/转空) + 追踪
/// 与库guidao.rs L647-959 calc_seg_upper/lower_band_case1+case2+case3完全一致
unsafe extern "C" fn segment_band_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32; // 7=上轨, 8=下轨

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let segs = &pipeline.segments;
    let pl_strokes = &pipeline.strokes;
    let is_upper = m == 7;

    // 2026-08-24: 全量下沉库 (对齐 flowsurface app chanlun_guidao.rs compute_segment_tracked_bands 直接委派库)
    // 旧手写 Case3/4 与库"一个线段当大段"状态机不一致, 是伪转多/伪转空 BUG 根源 (提交 9386187)
    let (seg_upper, seg_lower) = guidao::compute_segment_bands(
        segs, &pipeline.big_segments, pl_strokes, &highs, &lows, &pipeline.final_fractals,
    );
    let band = if is_upper { &seg_upper } else { &seg_lower };
    let bp: Vec<(usize, f64)> = band.iter().map(|pt| (pt.bar_index, pt.value)).collect();

    // Step style output
    let mut values: Vec<(usize, f32)> = Vec::new();
    if !bp.is_empty() {
        let mut cur_val = bp[0].1 as f32;
        let mut pt_idx = 1usize;
        for bar in 0..dl {
            while pt_idx < bp.len() && bp[pt_idx].0 <= bar { cur_val = bp[pt_idx].1 as f32; pt_idx += 1; }
            if bar >= bp[0].0 { values.push((bar, cur_val)); }
        }
    }
    write_output(out, dl, &values);
}

// ── 大段轨道共用: 库 Case1-4+追踪 (2026-08-24: 轨道取值三级统一, 下沉库) → 逐bar轨道线 ──
fn compute_big_seg_band_line(
    big_segs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    pl_strokes: &[chanlun_lean_lib::Stroke],
    superior_segs: &[(usize, usize, bool)],
    final_fractals: &[chanlun_lean_lib::Fractal], // 2026-08-25: 分型确认bar落点 (三级轨道同构)
    highs: &[f64],
    lows: &[f64],
    is_upper: bool,
    dl: usize,
) -> Vec<f32> {
    // 全量下沉库 (对齐 flowsurface app compute_bigseg_tracked_bands 直接委派库)
    let (upper_band, lower_band, _, _) = guidao::compute_bigseg_bands(
        big_segs, segs, pl_strokes, superior_segs, final_fractals, highs, lows,
    );
    let band = if is_upper { &upper_band } else { &lower_band };
    let bp: Vec<(usize, f64)> = band.iter().map(|pt| (pt.bar_index, pt.value)).collect();

    // 逐bar轨道线
    let mut band_line = vec![0.0f32; dl];
    if !bp.is_empty() {
        let mut cur_val = bp[0].1 as f32;
        let mut pt_idx = 1usize;
        for bar in 0..dl {
            while pt_idx < bp.len() && bp[pt_idx].0 <= bar { cur_val = bp[pt_idx].1 as f32; pt_idx += 1; }
            if bar >= bp[0].0 { band_line[bar] = cur_val; }
        }
    }
    band_line
}

// ── 大段轨道 (Big Segment Bands) ──
// mark 9=上轨(COLOR9F9F00), mark 10=下轨(COLOR9F9F00)
unsafe extern "C" fn big_segment_band_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32; // 9=上轨, 10=下轨

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let big_segs = &pipeline.big_segments;
    let segs = &pipeline.segments;
    let pl_strokes = &pipeline.strokes;
    let is_upper = m == 9;

    let band_line = compute_big_seg_band_line(
        big_segs, segs, pl_strokes, &pipeline.superior_segments, &pipeline.final_fractals, &highs, &lows, is_upper, dl,
    );

    let mut values: Vec<(usize, f32)> = Vec::new();
    for (bar, &val) in band_line.iter().enumerate() {
        if val != 0.0 { values.push((bar, val)); }
    }
    write_output(out, dl, &values);
}

// ── 买卖点标记输入链 (社区版: 中枢 → 二买二卖 → 三买三卖 → 2+N买/卖 → 中阴撤回过滤) ──
// 返回 (second_markers, third_markers), 供 mark 37 输出 (mode 1-4)
fn build_markers(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[chanlun_lean_lib::Stroke],
    highs: &[f64],
    lows: &[f64],
) -> (
    Vec<chanlun_lean_lib::SecondMarker>,
    Vec<chanlun_lean_lib::zhongshu::BigSegThirdMarker>,
) {
    let zs = chanlun_lean_lib::zhongshu::detect_bigseg_zhongshus(sups, bigs, segs, strokes);
    // 二买+二卖合并 (detect_second_buy_markers 只产二买 is_buy=true, 二卖在独立 sell 函数, GUI 两函数合并)
    let mut sm = chanlun_lean_lib::detect_second_buy_markers(strokes, segs, bigs, sups);
    sm.extend(chanlun_lean_lib::detect_second_sell_markers(strokes, segs, bigs, sups));
    let tm = chanlun_lean_lib::zhongshu::detect_bigseg_third_marks(sups, bigs, segs, strokes, &zs, highs, lows);
    // 2+N买/卖
    let (blc1, db) = guidao::calc_bigseg_lower_band_case1(bigs, segs, strokes);
    let (buc1, ub) = guidao::calc_bigseg_upper_band_case1(bigs, segs, strokes);
    let mut pm = guidao::detect_bigseg_cf_buy_markers(sups, bigs, segs, strokes, &blc1, &db, &ub);
    pm.extend(guidao::detect_bigseg_cf_sell_markers(sups, bigs, segs, strokes, &buc1, &ub, &db));
    // 中阴撤回过滤: 标记输出=过滤后标记
    let rf = guidao::apply_sup_retreat_filter(sups, bigs, segs, strokes, sm, tm, pm);
    (rf.second_markers, rf.third_markers)
}

// ── 二买/二卖/三买/三卖 文字标记 (mark 37, 2026-08-11 对齐 MT4 chanlun_markers) ──
// mode 1=二买 2=二卖 3=三买 4=三卖; 输出=标记价格 (文字定位点)
unsafe extern "C" fn buy_sell_markers_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32;

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let big_segs = &pipeline.big_segments;
    let segs = &pipeline.segments;
    let pl_strokes = &pipeline.strokes;
    let (sm, tm) = build_markers(
        &pipeline.superior_segments, big_segs, segs, pl_strokes, &highs, &lows,
    );

    let mut values: Vec<(usize, f32)> = Vec::new();
    match m {
        1 => { for x in &sm { if x.is_buy { values.push((x.bar_index, x.price as f32)); } } }
        2 => { for x in &sm { if !x.is_buy { values.push((x.bar_index, x.price as f32)); } } }
        3 => { for x in &tm { if x.is_buy { values.push((x.bar_index, x.price as f32)); } } }
        4 => { for x in &tm { if !x.is_buy { values.push((x.bar_index, x.price as f32)); } } }
        _ => {}
    }
    write_output(out, dl, &values);
}

// ── 大段中枢 ZG/ZD (marks 39/40, 2026-08-11 对齐 MT4 chanlun_zhongshus) ──
// mode 39=ZG, 40=ZD; 输出=中枢区间 [start_bar, end_bar] 内逐bar填值 (画矩形上下沿)
unsafe extern "C" fn zhongshu_fn(
    data_len: c_int,
    out: *mut c_float,
    highs_in: *mut c_float,
    lows_in: *mut c_float,
    mode: *mut c_float,
) {
    let dl = data_len as usize;
    let highs = read_floats(highs_in, dl);
    let lows = read_floats(lows_in, dl);
    let m = *mode as i32; // 39=ZG, 40=ZD, 41=中枢开始标记, 42=中枢结束标记
    let use_zg = m == 39;
    let use_marker = m == 41 || m == 42;

    let pipeline = get_pipeline(highs.clone(), lows.clone());
    let zs = zhongshu::detect_bigseg_zhongshus(
        &pipeline.superior_segments, &pipeline.big_segments,
        &pipeline.segments, &pipeline.strokes,
    );

    let mut values: Vec<(usize, f32)> = Vec::new();
    for z in zs {
        if use_marker {
            // 最佳实践(对齐成熟TDX缠论公式): DLL输出中枢开始/结束标记,
            // 公式侧 STICKLINE 画矩形竖边, 无未来函数
            let b = if m == 41 { z.start_bar } else { z.end_bar };
            if b < dl { values.push((b, 1.0)); }
        } else {
            let v = if use_zg { z.zg } else { z.zd };
            for b in z.start_bar..=z.end_bar {
                if b < dl { values.push((b, v as f32)); }
            }
        }
    }
    write_output(out, dl, &values);
}

// ── 函数注册表 ──
static mut G_CALC_FUNC_SETS: [PluginTCalcFuncInfo; 20] = [
    // ── 开源: 分型/笔/线段/大段/高级段/三级轨道/买卖点/中枢 ──
    PluginTCalcFuncInfo { n_func_mark: 10, p_call_func: Some(big_segment_band_fn) },
    PluginTCalcFuncInfo { n_func_mark: 9, p_call_func: Some(big_segment_band_fn) },
    PluginTCalcFuncInfo { n_func_mark: 8, p_call_func: Some(segment_band_fn) },
    PluginTCalcFuncInfo { n_func_mark: 7, p_call_func: Some(segment_band_fn) },
    PluginTCalcFuncInfo { n_func_mark: 6, p_call_func: Some(stroke_band_fn) },
    PluginTCalcFuncInfo { n_func_mark: 5, p_call_func: Some(stroke_band_fn) },
    PluginTCalcFuncInfo { n_func_mark: 4, p_call_func: Some(big_segments_fn) },
    PluginTCalcFuncInfo { n_func_mark: 3, p_call_func: Some(segments_fn) },
    PluginTCalcFuncInfo { n_func_mark: 2, p_call_func: Some(strokes_fn) },
    PluginTCalcFuncInfo { n_func_mark: 1, p_call_func: Some(fractals_fn) },
    PluginTCalcFuncInfo { n_func_mark: 35, p_call_func: Some(superior_segments_fn) },
    PluginTCalcFuncInfo { n_func_mark: 37, p_call_func: Some(buy_sell_markers_fn) },
    PluginTCalcFuncInfo { n_func_mark: 39, p_call_func: Some(zhongshu_fn) },
    PluginTCalcFuncInfo { n_func_mark: 40, p_call_func: Some(zhongshu_fn) },
    PluginTCalcFuncInfo { n_func_mark: 41, p_call_func: Some(zhongshu_fn) },
    PluginTCalcFuncInfo { n_func_mark: 42, p_call_func: Some(zhongshu_fn) },
    // ── 参数设置 (2026-09-05): 46=设笔 case 参数, 49=设线段~高级段 case 参数 ──
    PluginTCalcFuncInfo { n_func_mark: 46, p_call_func: Some(set_stroke_cases_fn) },
    PluginTCalcFuncInfo { n_func_mark: 49, p_call_func: Some(set_level_cases_fn) },
    PluginTCalcFuncInfo { n_func_mark: 0, p_call_func: None },
    PluginTCalcFuncInfo { n_func_mark: 0, p_call_func: None },
];


#[no_mangle]
#[allow(static_mut_refs)]
pub unsafe extern "C" fn RegisterTdxFunc(p_fun: *mut *mut PluginTCalcFuncInfo) -> c_int {
    if (*p_fun).is_null() {
        *p_fun = G_CALC_FUNC_SETS.as_mut_ptr();
        1
    } else {
        0
    }
}

// ── 自验证测试 ──
#[cfg(test)]
mod tests {
    use super::*;

    /// 用简单的高低交替K线验证管线: 分型→笔 的链条完整性
    #[test]
    fn test_fractal_to_stroke_chain() {
        // 构造有明显波峰波谷的数据: 涨→跌→涨→跌
        let mut highs: Vec<f64> = Vec::new();
        let mut lows: Vec<f64> = Vec::new();
        let base = 100.0;
        // 波段1: 上涨 (0-20)
        for i in 0..20 {
            highs.push(base + i as f64 * 0.5 + 2.0);
            lows.push(base + i as f64 * 0.5 - 2.0);
        }
        // 波段2: 下跌 (20-45)
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0);
            lows.push(p - 2.0);
        }
        // 波段3: 上涨 (45-70)
        for i in 0..25 {
            let p = base + i as f64 * 0.4;
            highs.push(p + 2.0);
            lows.push(p - 2.0);
        }
        // 波段4: 下跌 (70-95)
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0);
            lows.push(p - 2.0);
        }

        let pipeline = get_pipeline(highs.clone(), lows.clone());
        let ff = &pipeline.final_fractals;
        let strokes = &pipeline.strokes;

        eprintln!("\n=== 分型 final_fractals ({} 个) ===", ff.len());
        for (i, f) in ff.iter().enumerate() {
            let dir = if f.is_top { "顶T" } else { "底B" };
            eprintln!("  F[{}] bar={} price={:.2} {}", i, f.bar_index, f.price, dir);
        }

        eprintln!("\n=== 笔 strokes ({} 笔) ===", strokes.len());
        for (i, s) in strokes.iter().enumerate() {
            let dir = if s.is_up { "↑" } else { "↓" };
            eprintln!("  S[{}] {}: bar {} ({:.2}) → bar {} ({:.2})",
                i, dir, s.start_bar, s.start_price, s.end_bar, s.end_price);
        }

        // 验证: 笔端点必须来自final_fractals
        assert!(!ff.is_empty(), "should have fractals");
        assert!(!strokes.is_empty(), "should have strokes");
        assert_eq!(strokes.len() + 1, ff.len(),
            "N strokes should come from N+1 fractals");

        // 验证: 每笔的start和end分别来自相邻分型
        for (i, s) in strokes.iter().enumerate() {
            let f_start = &ff[i];
            let f_end = &ff[i + 1];
            assert_eq!(s.start_bar, f_start.bar_index,
                "stroke[{}] start_bar mismatch", i);
            assert_eq!(s.end_bar, f_end.bar_index,
                "stroke[{}] end_bar mismatch", i);
            assert_eq!(s.start_price, f_start.price,
                "stroke[{}] start_price mismatch", i);
            assert_eq!(s.end_price, f_end.price,
                "stroke[{}] end_price mismatch", i);
            // 方向验证
            let expected_is_up = !f_start.is_top && f_end.is_top;
            assert_eq!(s.is_up, expected_is_up,
                "stroke[{}] direction mismatch", i);
        }

        // 验证: start_bar < end_bar (笔必须向前推进)
        for s in strokes {
            assert!(s.start_bar < s.end_bar,
                "stroke must go forward: {} → {}", s.start_bar, s.end_bar);
        }

        eprintln!("\n✓ fractal→stroke chain verified");
    }

    /// 验证 FFI 输出的方向值正确
    #[test]
    fn test_stroke_direction_values() {
        let mut highs: Vec<f64> = Vec::new();
        let mut lows: Vec<f64> = Vec::new();
        let base = 100.0;
        for i in 0..20 {
            highs.push(base + i as f64 * 0.5 + 2.0);
            lows.push(base + i as f64 * 0.5 - 2.0);
        }
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0); lows.push(p - 2.0);
        }
        for i in 0..25 {
            let p = base + i as f64 * 0.4;
            highs.push(p + 2.0); lows.push(p - 2.0);
        }
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0); lows.push(p - 2.0);
        }

        let pipeline = get_pipeline(highs.clone(), lows.clone());
        let strokes = &pipeline.strokes;

        for s in strokes {
            // 上升笔: start是底分型(F.is_top=false) → start_dir=-1
            //         end是顶分型(F.is_top=true)   → end_dir=+1
            let start_dir: f32 = if s.is_up { -1.0 } else { 1.0 };
            let end_dir: f32   = if s.is_up { 1.0 } else { -1.0 };

            // 底分型和顶分型方向必须不同
            assert_ne!(start_dir, end_dir,
                "stroke bar {}→{}: dirs must differ", s.start_bar, s.end_bar);

            // start_dir和end_dir必须与stroke的is_up一致
            if s.is_up {
                assert_eq!(start_dir, -1.0, "up stroke start must be -1 (bottom)");
                assert_eq!(end_dir, 1.0, "up stroke end must be +1 (top)");
            } else {
                assert_eq!(start_dir, 1.0, "down stroke start must be +1 (top)");
                assert_eq!(end_dir, -1.0, "down stroke end must be -1 (bottom)");
            }
        }
        eprintln!("✓ stroke direction values verified for {} strokes", strokes.len());
    }

    /// 验证分型最终输出与final_fractals一致
    #[test]
    fn test_fractal_output() {
        let mut highs: Vec<f64> = Vec::new();
        let mut lows: Vec<f64> = Vec::new();
        let base = 100.0;
        for i in 0..20 {
            highs.push(base + i as f64 * 0.5 + 2.0);
            lows.push(base + i as f64 * 0.5 - 2.0);
        }
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0); lows.push(p - 2.0);
        }
        for i in 0..25 {
            let p = base + i as f64 * 0.4;
            highs.push(p + 2.0); lows.push(p - 2.0);
        }
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0); lows.push(p - 2.0);
        }

        let pipeline = get_pipeline(highs.clone(), lows.clone());
        let ff = &pipeline.final_fractals;

        assert!(!ff.is_empty());
        eprintln!("first fractal: bar={} {} {}", ff[0].bar_index, ff[0].price,
            if ff[0].is_top { "顶" } else { "底" });

        // 验证方向交替
        for i in 1..ff.len() {
            assert_ne!(ff[i-1].is_top, ff[i].is_top,
                "fractals must alternate: F[{}] and F[{}] both {}", i-1, i,
                if ff[i].is_top { "top" } else { "bottom" });
        }
        eprintln!("✓ fractal output verified: {} fractals, all alternating", ff.len());
    }

    /// 验证 g1==g2 时笔管线正常输出 (对齐 flowsurface, 无后置过滤)
    /// 场景: d1(3.84)→g1(3.94)→d2(3.85)→g2(3.94)
    #[test]
    fn test_g1_equals_g2_pipeline_ok() {
        let mut highs: Vec<f64> = Vec::new();
        let mut lows: Vec<f64> = Vec::new();
        for i in 0..5 { highs.push(3.80 + i as f64 * 0.035); lows.push(3.78 + i as f64 * 0.030); }
        for i in 0..5 { highs.push(3.94 - i as f64 * 0.018); lows.push(3.92 - i as f64 * 0.020); }
        for i in 0..3 { highs.push(3.85 + i as f64 * 0.045); lows.push(3.83 + i as f64 * 0.040); }

        let pipeline = get_pipeline(highs.clone(), lows.clone());
        let strokes = &pipeline.strokes;

        eprintln!("\n=== g1==g2 pipeline: {} strokes ===", strokes.len());
        for s in strokes {
            eprintln!("  {} bar{} ({:.2}) → bar{} ({:.2})",
                if s.is_up { "↑" } else { "↓" }, s.start_bar, s.start_price, s.end_bar, s.end_price);
        }
        // 管线至少输出一笔 (g1==g2 在 check_original_valid 中由 >=  + != 等价 <, 不触发阻挡)
        assert!(!strokes.is_empty(), "should produce at least one stroke");
        eprintln!("✓ g1==g2 pipeline verified");
    }

    /// 诊断: 验证 guidao 库函数在 DLL 中是否能正常工作
    #[test]
    fn test_stroke_band_output() {
        use chanlun_lean_lib::Fractal;
        let test_ff = vec![
            Fractal { price: 30.0, is_top: true, bar_index: 20, merged_index: 0, time: 0 },
            Fractal { price: 8.0, is_top: false, bar_index: 40, merged_index: 0, time: 0 },
            Fractal { price: 30.0, is_top: true, bar_index: 60, merged_index: 0, time: 0 },
            Fractal { price: 9.0, is_top: false, bar_index: 80, merged_index: 0, time: 0 },
        ];
        // Inline verification: should find 1 up stroke
        let mut count = 0;
        for i in 1..test_ff.len() {
            if !test_ff[i-1].is_top && test_ff[i].is_top { count += 1; }
        }
        assert_eq!(count, 1, "inline: expected 1 up stroke, got {}", count);
        eprintln!("✓ stroke band inline logic verified");
    }

    /// 验证 Case2 (1.618扩张) 笔轨道: 上轨+下轨+负面验证
    #[test]
    fn test_stroke_band_case2() {
        use chanlun_lean_lib::Fractal;
        use chanlun_lean_lib::guidao;

        // 上轨Case2: 2个下降笔, 第二笔空间≥1.618×第一笔, first_high>second_high
        // 下降笔1: high=100 low=80 space=20, 下降笔2: high=95 low=60 space=35
        let ff_upper = vec![
            Fractal { price: 100.0, is_top: true,  bar_index: 10, merged_index: 0, time: 0 },
            Fractal { price: 80.0,  is_top: false, bar_index: 20, merged_index: 0, time: 0 },
            Fractal { price: 95.0,  is_top: true,  bar_index: 30, merged_index: 0, time: 0 },
            Fractal { price: 60.0,  is_top: false, bar_index: 40, merged_index: 0, time: 0 },
        ];
        let upper_c2 = guidao::calc_upper_band_case2(&ff_upper);
        assert_eq!(upper_c2.len(), 1, "upper Case2: expected 1 point");
        assert_eq!(upper_c2[0].bar_index, 10);
        assert!((upper_c2[0].value - 100.0).abs() < 0.001);

        // 下轨Case2: 2个上升笔, 第二笔空间≥1.618×第一笔, first_low<second_low
        // 上升笔1: low=60 high=80 space=20, 上升笔2: low=65 high=100 space=35
        let ff_lower = vec![
            Fractal { price: 60.0,  is_top: false, bar_index: 10, merged_index: 0, time: 0 },
            Fractal { price: 80.0,  is_top: true,  bar_index: 20, merged_index: 0, time: 0 },
            Fractal { price: 65.0,  is_top: false, bar_index: 30, merged_index: 0, time: 0 },
            Fractal { price: 100.0, is_top: true,  bar_index: 40, merged_index: 0, time: 0 },
        ];
        let lower_c2 = guidao::calc_lower_band_case2(&ff_lower);
        assert_eq!(lower_c2.len(), 1, "lower Case2: expected 1 point");
        assert_eq!(lower_c2[0].bar_index, 20);
        assert!((lower_c2[0].value - 60.0).abs() < 0.001);

        // 负面验证: 空间不足1.618 → 不触发
        let ff_neg = vec![
            Fractal { price: 100.0, is_top: true,  bar_index: 10, merged_index: 0, time: 0 },
            Fractal { price: 80.0,  is_top: false, bar_index: 20, merged_index: 0, time: 0 },
            Fractal { price: 95.0,  is_top: true,  bar_index: 30, merged_index: 0, time: 0 },
            Fractal { price: 75.0,  is_top: false, bar_index: 40, merged_index: 0, time: 0 },
        ];
        // 下降笔1: space=20, 下降笔2: space=20 → 20 < 32.36 → 不触发
        let upper_neg = guidao::calc_upper_band_case2(&ff_neg);
        assert_eq!(upper_neg.len(), 0, "negative Case2: expected 0 points");

        eprintln!("✓ Case2 (1.618 expansion) verified: upper=1pt, lower=1pt, negative=0pt");
    }

    /// 验证 Case3 (转多/转空) 笔轨道: 转空→上轨 + 转多→下轨 + 负面验证
    #[test]
    fn test_stroke_band_case3() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case3: 转空 ===
        // Stroke 0: 上升 10→30 bar0-10
        // Stroke 1: 下降 30→15 bar10-20
        // Stroke 2: 上升 15→35 bar20-30 (扩张: low 15>10, high 35>30 → check_up_segment_case2=true)
        // Stroke 3: 下降 35→5  bar30-40 (转空触发: low≤10)
        let strokes_upper = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 15.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 15.0, end_price: 35.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 35.0, end_price: 5.0,  start_bar: 30, end_bar: 40, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![];
        let highs = vec![20.0; 41];
        let mut lows  = vec![18.0; 41];
        // bar 35: low=8 ≤ start_price=10 → 触发转空
        lows[35] = 8.0;

        let ts_upper = guidao::detect_turn_signals(&strokes_upper, &segs, &highs, &lows);
        assert_eq!(ts_upper.len(), 1, "should have 1 turn signal (转空)");
        assert!(ts_upper[0].is_turn_short, "should be 转空");
        assert_eq!(ts_upper[0].stroke_idx, 0);
        assert_eq!(ts_upper[0].end_stroke_idx, 2);
        assert!((ts_upper[0].a_high - 35.0).abs() < 0.001, "a_high should be 35");

        let upper_c3 = guidao::calc_upper_band_case3(&ts_upper, &strokes_upper);
        assert_eq!(upper_c3.len(), 1, "upper Case3: expected 1 point");
        assert_eq!(upper_c3[0].bar_index, 30, "bar should be strokes[2].end_bar=30");
        assert!((upper_c3[0].value - 35.0).abs() < 0.001, "value should be a_high=35");

        // 转空不产生下轨点
        let lower_from_ts = guidao::calc_lower_band_case3(&ts_upper, &strokes_upper);
        assert_eq!(lower_from_ts.len(), 0, "lower from 转空 signals: expected 0");

        // === 下轨Case3: 转多 ===
        // Stroke 0: 下降 40→20 bar0-10
        // Stroke 1: 上升 20→35 bar10-20
        // Stroke 2: 下降 35→10 bar20-30 (扩张: high 35<40, low 10<20 → check_down_segment_case2=true)
        // Stroke 3: 上升 10→50 bar30-40 (转多触发: high≥40)
        let strokes_lower = vec![
            Stroke { start_price: 40.0, end_price: 20.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 20.0, end_price: 35.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 35.0, end_price: 10.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 30, end_bar: 40, is_up: true },
        ];
        let mut highs2 = vec![20.0; 41];
        let lows2  = vec![18.0; 41];
        // bar 35: high=45 ≥ start_price=40 → 触发转多
        highs2[35] = 45.0;

        let ts_lower = guidao::detect_turn_signals(&strokes_lower, &segs, &highs2, &lows2);
        assert_eq!(ts_lower.len(), 1, "should have 1 turn signal (转多)");
        assert!(!ts_lower[0].is_turn_short, "should be 转多");
        assert_eq!(ts_lower[0].stroke_idx, 0);
        assert_eq!(ts_lower[0].end_stroke_idx, 2);
        assert!((ts_lower[0].a_low - 10.0).abs() < 0.001, "a_low should be 10");

        let lower_c3 = guidao::calc_lower_band_case3(&ts_lower, &strokes_lower);
        assert_eq!(lower_c3.len(), 1, "lower Case3: expected 1 point");
        assert_eq!(lower_c3[0].bar_index, 30, "bar should be strokes[2].end_bar=30");
        assert!((lower_c3[0].value - 10.0).abs() < 0.001, "value should be a_low=10");

        // === 负面验证: 无扩张 → 无转信号 ===
        // 2个上升笔但第二个low≤第一个low → check_up_segment_case2返回false
        let strokes_neg = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 15.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 8.0,  end_price: 25.0, start_bar: 20, end_bar: 30, is_up: true }, // low=8 ≤ 10 → 不扩张
            Stroke { start_price: 25.0, end_price: 5.0,  start_bar: 30, end_bar: 40, is_up: false },
        ];
        let ts_neg = guidao::detect_turn_signals(&strokes_neg, &segs, &highs, &lows);
        assert_eq!(ts_neg.len(), 0, "negative: no expansion → no turn signals");

        eprintln!("✓ Case3 (转多/转空) verified: 转空→upper=1pt, 转多→lower=1pt, negative=0pt");
    }

    /// 验证 Case4 (V信号反弹) 笔轨道: V空→上轨 + V多→下轨 + 负面验证
    #[test]
    fn test_stroke_band_case4() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case4: 转多→V空 ===
        // 2026-08-24 新API状态机方案 (collect_segment_case3_events):
        // Stroke 3: 上升 10→50 经 C3 升格 (50 > dn0 起点 40) → 转多事件 (3,0,2)
        // Stroke 4: 下降 50→5  经 C3 升格 (5 < up3 起点 10) → 转空事件 (4,3,3) → V空
        let strokes_upper = vec![
            Stroke { start_price: 40.0, end_price: 20.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 20.0, end_price: 35.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 35.0, end_price: 10.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 50.0, end_price: 5.0,  start_bar: 40, end_bar: 50, is_up: false },
        ];
        let segs: Vec<(usize, usize, bool)> = vec![];
        let mut highs_u = vec![20.0; 51];
        let lows_u  = vec![18.0; 51];
        // bar 35: high=45 ≥ start_price=40 → 触发转多
        highs_u[35] = 45.0;

        let ts_upper = guidao::detect_turn_signals(&strokes_upper, &segs, &highs_u, &lows_u);
        assert_eq!(ts_upper.len(), 2, "should have 2 turn signals (转多+转空)");
        assert!(!ts_upper[0].is_turn_short, "should be 转多");
        assert_eq!(ts_upper[0].end_stroke_idx, 2);

        let vs_upper = guidao::detect_v_signals(&ts_upper, &strokes_upper, &highs_u, &lows_u);
        assert_eq!(vs_upper.len(), 1, "should have 1 V-signal (V空)");
        assert!(!vs_upper[0].is_turn_short, "V空 comes from 转多");

        let upper_c4 = guidao::calc_upper_band_case4(&vs_upper, &strokes_upper);
        assert_eq!(upper_c4.len(), 1, "upper Case4: expected 1 point");
        assert_eq!(upper_c4[0].bar_index, 40, "bar should be strokes[3].end_bar=40");
        assert!((upper_c4[0].value - 50.0).abs() < 0.001, "value should be strokes[3].end_price=50");

        // V空不产生下轨点
        let lower_from_vs = guidao::calc_lower_band_case4(&vs_upper, &strokes_upper);
        assert_eq!(lower_from_vs.len(), 0, "lower from V空 signals: expected 0");

        // === 下轨Case4: 转空→V多 ===
        // 2026-08-24 新API状态机方案 (collect_segment_case3_events):
        // Stroke 3: 下降 35→5  经 C3 升格 (5 < up0 起点 10) → 转空事件 (3,0,2)
        // Stroke 4: 上升 5→40 经 C3 升格 (40 > dn3 起点 35) → 转多事件 (4,3,3) → V多
        let strokes_lower = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 15.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 15.0, end_price: 35.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 35.0, end_price: 5.0,  start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 5.0,  end_price: 40.0, start_bar: 40, end_bar: 50, is_up: true },
        ];
        let highs_l = vec![20.0; 51];
        let mut lows_l  = vec![18.0; 51];
        // bar 35: low=8 ≤ start_price=10 → 触发转空
        lows_l[35] = 8.0;

        let ts_lower = guidao::detect_turn_signals(&strokes_lower, &segs, &highs_l, &lows_l);
        assert_eq!(ts_lower.len(), 2, "should have 2 turn signals (转空+转多)");
        assert!(ts_lower[0].is_turn_short, "should be 转空");
        assert_eq!(ts_lower[0].end_stroke_idx, 2);

        let vs_lower = guidao::detect_v_signals(&ts_lower, &strokes_lower, &highs_l, &lows_l);
        assert_eq!(vs_lower.len(), 1, "should have 1 V-signal (V多)");
        assert!(vs_lower[0].is_turn_short, "V多 comes from 转空");

        let lower_c4 = guidao::calc_lower_band_case4(&vs_lower, &strokes_lower);
        assert_eq!(lower_c4.len(), 1, "lower Case4: expected 1 point");
        assert_eq!(lower_c4[0].bar_index, 40, "bar should be strokes[3].end_bar=40");
        assert!((lower_c4[0].value - 5.0).abs() < 0.001, "value should be strokes[3].end_price=5");

        // === 负面验证: V信号不满足条件 ===
        // dn4 终点 15 ≥ up3 起点 10 → 无 C3 升格 → 无转空事件 → 无 V空
        let strokes_neg = vec![
            Stroke { start_price: 40.0, end_price: 20.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 20.0, end_price: 35.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 35.0, end_price: 10.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 30, end_bar: 40, is_up: true },
            // dn4 终点 15 ≥ up3 起点 10 → 无转空事件 → 无 V空
            Stroke { start_price: 50.0, end_price: 15.0, start_bar: 40, end_bar: 50, is_up: false },
        ];
        let ts_neg = guidao::detect_turn_signals(&strokes_neg, &segs, &highs_u, &lows_u);
        let vs_neg = guidao::detect_v_signals(&ts_neg, &strokes_neg, &highs_u, &lows_u);
        assert_eq!(vs_neg.len(), 0, "negative: V空 condition not met → no V-signals");

        eprintln!("✓ Case4 (V信号) verified: V空→upper=1pt, V多→lower=1pt, negative=0pt");
    }

    /// 验证线段轨道Case1 (三重顶/底): 上轨三重顶 + 下轨三重底 + 负面验证
    #[test]
    fn test_segment_band_case1() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case1: 上升线段三重顶 ===
        // 3个上升线段, 高点依次降低: 50 > 40 > 30
        let strokes_upper = vec![
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 50.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 20.0, end_price: 40.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 40.0, end_price: 15.0, start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 15.0, end_price: 30.0, start_bar: 40, end_bar: 50, is_up: true },
            Stroke { start_price: 30.0, end_price: 5.0,  start_bar: 50, end_bar: 60, is_up: false },
        ];
        // segments: (start_stroke_idx, end_stroke_idx, is_up)
        let segs_upper = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
            (4, 4, true),  (5, 5, false),
        ];
        let (upper_bp, up_segs) = guidao::calc_seg_upper_band_case1(&segs_upper, &strokes_upper);
        assert_eq!(upper_bp.len(), 1, "upper Case1: expected 1 point");
        assert_eq!(upper_bp[0].bar_index, 10, "bar should be strokes[0].end_bar=10");
        assert!((upper_bp[0].value - 50.0).abs() < 0.001, "value should be 50");
        assert_eq!(up_segs.len(), 3, "should have 3 up-segments");

        // === 下轨Case1: 下降线段三重底 ===
        // 3个下降线段, 低点依次升高: 10 < 15 < 20
        let strokes_lower = vec![
            Stroke { start_price: 50.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 40.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 40.0, end_price: 15.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 15.0, end_price: 35.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 35.0, end_price: 20.0, start_bar: 40, end_bar: 50, is_up: false },
            Stroke { start_price: 20.0, end_price: 45.0, start_bar: 50, end_bar: 60, is_up: true },
        ];
        let segs_lower = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false), (5, 5, true),
        ];
        let (lower_bp, down_segs) = guidao::calc_seg_lower_band_case1(&segs_lower, &strokes_lower);
        assert_eq!(lower_bp.len(), 1, "lower Case1: expected 1 point");
        assert_eq!(lower_bp[0].bar_index, 10, "bar should be strokes[0].end_bar=10");
        assert!((lower_bp[0].value - 10.0).abs() < 0.001, "value should be 10");
        assert_eq!(down_segs.len(), 3, "should have 3 down-segments");

        // === 负面验证: 高点依次升高 → 不满足三重顶 ===
        let strokes_neg = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 20.0, end_price: 40.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 40.0, end_price: 15.0, start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 15.0, end_price: 50.0, start_bar: 40, end_bar: 50, is_up: true },
            Stroke { start_price: 50.0, end_price: 5.0,  start_bar: 50, end_bar: 60, is_up: false },
        ];
        let segs_neg = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
            (4, 4, true),  (5, 5, false),
        ];
        let (neg_bp, _) = guidao::calc_seg_upper_band_case1(&segs_neg, &strokes_neg);
        assert_eq!(neg_bp.len(), 0, "negative: ascending highs → no triple-top");

        eprintln!("✓ 线段轨道Case1 verified: 上轨三重顶=1pt, 下轨三重底=1pt, negative=0pt");
    }

    /// 验证线段轨道Case2 (1.618扩张): 上轨下降线段扩张 + 下轨上升线段扩张 + 负面验证
    #[test]
    fn test_segment_band_case2() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case2: 下降线段1.618扩张 ===
        // 下降线段1: 50→20 (空间30), 下降线段2: 45→5 (空间40 >= 30*1.618=48.54? 不够)
        // 改: 下降线段1: 50→30 (空间20), 下降线段2: 45→10 (空间35 >= 20*1.618=32.36) → 扩张且 50>45
        let strokes_upper = vec![
            Stroke { start_price: 30.0, end_price: 50.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 50.0, end_price: 30.0, start_bar: 10, end_bar: 20, is_up: false }, // down_seg1: 50→30, 空间20
            Stroke { start_price: 30.0, end_price: 45.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 45.0, end_price: 10.0, start_bar: 30, end_bar: 40, is_up: false }, // down_seg2: 45→10, 空间35
        ];
        let segs_upper = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
        ];
        let upper_c2 = guidao::calc_seg_upper_band_case2(&segs_upper, &strokes_upper);
        assert_eq!(upper_c2.len(), 1, "upper Case2: expected 1 point");
        assert_eq!(upper_c2[0].bar_index, 10, "bar should be down_seg1 start_bar=10");
        assert!((upper_c2[0].value - 50.0).abs() < 0.001, "value should be down_seg1 high=50");

        // === 下轨Case2: 上升线段1.618扩张 ===
        // 上升线段1: 10→30 (空间20), 上升线段2: 15→50 (空间35 >= 20*1.618=32.36) → 扩张且 10<15
        let strokes_lower = vec![
            Stroke { start_price: 30.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 10, end_bar: 20, is_up: true },  // up_seg1: 10→30, 空间20
            Stroke { start_price: 30.0, end_price: 15.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 15.0, end_price: 50.0, start_bar: 30, end_bar: 40, is_up: true },  // up_seg2: 15→50, 空间35
        ];
        let segs_lower = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
        ];
        let lower_c2 = guidao::calc_seg_lower_band_case2(&segs_lower, &strokes_lower);
        assert_eq!(lower_c2.len(), 1, "lower Case2: expected 1 point");
        assert_eq!(lower_c2[0].bar_index, 20, "bar should be up_seg1 end_bar=20");
        assert!((lower_c2[0].value - 10.0).abs() < 0.001, "value should be up_seg1 low=10");

        // === 负面验证: 空间不足1.618 → 不扩张 ===
        // 下降线段1: 50→30 (空间20), 下降线段2: 45→25 (空间20 < 20*1.618=32.36)
        let strokes_neg = vec![
            Stroke { start_price: 30.0, end_price: 50.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 50.0, end_price: 30.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 30.0, end_price: 45.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 45.0, end_price: 25.0, start_bar: 30, end_bar: 40, is_up: false },
        ];
        let segs_neg = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
        ];
        let neg_c2 = guidao::calc_seg_upper_band_case2(&segs_neg, &strokes_neg);
        assert_eq!(neg_c2.len(), 0, "negative: space < 1.618 → no expansion");

        eprintln!("✓ 线段轨道Case2 verified: 上轨扩张=1pt, 下轨扩张=1pt, negative=0pt");
    }

    /// 验证线段轨道Case3 (转多/转空): 上轨转空 + 下轨转多 + 负面验证
    #[test]
    fn test_segment_band_case3() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case3: 转空 ===
        // 2026-08-24 新API状态机方案: up_seg2 经 C2 ok (end_idx=2), dn_seg3(60→5)
        // 经 C3 升格 (5 < up0 起点 10) → 转空事件 (3,0,2) → 点=up_seg2 终点 60@bar30
        let strokes_upper = vec![
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 50.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 20.0, end_price: 60.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 60.0, end_price: 5.0,  start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 25.0, end_price: 55.0, start_bar: 40, end_bar: 50, is_up: true },
        ];
        let segs_upper = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
            (4, 4, true),
        ];
        let highs_u = vec![60.0f64; 51];
        let mut lows_u = vec![11.0f64; 51];
        lows_u[31] = 5.0; // ≤ 10 → 转空
        let big_segs: Vec<(usize, usize, bool)> = vec![];
        let ts_upper = guidao::detect_segment_turn_signals(&segs_upper, &big_segs, &strokes_upper, &highs_u, &lows_u);
        let upper_bp = guidao::calc_seg_upper_band_case3(&ts_upper, &segs_upper, &strokes_upper);
        assert_eq!(upper_bp.len(), 1, "upper Case3: expected 1 point");
        assert_eq!(upper_bp[0].bar_index, 30, "bar should be strokes[2].end_bar=30");
        assert!((upper_bp[0].value - 60.0).abs() < 0.001, "value should be a_high=60");

        // === 下轨Case3: 转多 ===
        // 2026-08-24 新API状态机方案: dn_seg2 经 C2 ok (end_idx=2), up_seg3(5→55)
        // 经 C3 升格 (55 > dn0 起点 50) → 转多事件 (3,0,2) → 点=dn_seg2 终点 5@bar30
        let strokes_lower = vec![
            Stroke { start_price: 50.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 40.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 40.0, end_price: 5.0,  start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 5.0,  end_price: 55.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 45.0, end_price: 15.0, start_bar: 40, end_bar: 50, is_up: false },
        ];
        let segs_lower = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false),
        ];
        let mut highs_l = vec![49.0f64; 51];
        let lows_l = vec![10.0f64; 51];
        highs_l[31] = 55.0; // ≥ 50 → 转多
        let ts_lower = guidao::detect_segment_turn_signals(&segs_lower, &big_segs, &strokes_lower, &highs_l, &lows_l);
        let lower_bp = guidao::calc_seg_lower_band_case3(&ts_lower, &segs_lower, &strokes_lower);
        assert_eq!(lower_bp.len(), 1, "lower Case3: expected 1 point");
        assert_eq!(lower_bp[0].bar_index, 30, "bar should be strokes[2].end_bar=30");
        assert!((lower_bp[0].value - 5.0).abs() < 0.001, "value should be a_low=5");

        // === 负面验证: 无 C3 价格突破 → 无转信号 ===
        // dn_seg3(60→25): 25 ≥ up0 起点 10 → 无 C3 升格 → 无转空事件
        let strokes_neg = vec![
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 50.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 20.0, end_price: 60.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 60.0, end_price: 25.0, start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 25.0, end_price: 55.0, start_bar: 40, end_bar: 50, is_up: true },
        ];
        let ts_neg = guidao::detect_segment_turn_signals(&segs_upper, &big_segs, &strokes_neg, &highs_u, &lows_u);
        let neg_bp = guidao::calc_seg_upper_band_case3(&ts_neg, &segs_upper, &strokes_neg);
        assert_eq!(neg_bp.len(), 0, "negative: no C3 break → no turn signal");

        eprintln!("✓ 线段轨道Case3 verified: 上轨转空=1pt, 下轨转多=1pt, negative=0pt");
    }

    /// 验证线段轨道Case4 (V信号反弹): V空→上轨 + V多→下轨 + 负面验证
    #[test]
    fn test_segment_band_case4() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case4: 转多→V空 ===
        // 2026-08-24 新API状态机方案:
        // seg3=up(5→55) 经 C3 升格 (55 > dn0 起点 50) → 转多事件 (3,0,2)
        // seg4=down(45→3) 经 C3 升格 (3 < up3 起点 5) → 转空事件 (4,3,3) → V空
        let strokes_upper = vec![
            Stroke { start_price: 50.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 40.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 40.0, end_price: 5.0,  start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 5.0,  end_price: 55.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 45.0, end_price: 3.0,  start_bar: 40, end_bar: 50, is_up: false },
        ];
        let segs_upper = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false),
        ];
        let mut highs_u = vec![40.0f64; 51];
        let lows_u = vec![6.0f64; 51];
        highs_u[35] = 55.0; // ≥ 50 → 转多
        let big_segs: Vec<(usize, usize, bool)> = vec![];
        let ts_upper = guidao::detect_segment_turn_signals(&segs_upper, &big_segs, &strokes_upper, &highs_u, &lows_u);
        assert_eq!(ts_upper.len(), 2, "should have 2 turn signals (转多+转空)");
        assert!(!ts_upper[0].is_turn_short, "should be 转多");
        assert_eq!(ts_upper[0].end_seg_idx, 2);

        let vs_upper = guidao::detect_segment_v_signals(&ts_upper, &segs_upper, &strokes_upper, &highs_u, &lows_u);
        assert_eq!(vs_upper.len(), 1, "should have 1 V-signal (V空)");
        assert!(!vs_upper[0].is_turn_short, "V空 comes from 转多");

        let upper_c4 = guidao::calc_seg_upper_band_case4(&vs_upper, &segs_upper, &strokes_upper);
        assert_eq!(upper_c4.len(), 1, "upper Case4: expected 1 point");
        assert_eq!(upper_c4[0].bar_index, 40, "bar should be strokes[3].end_bar=40");
        assert!((upper_c4[0].value - 55.0).abs() < 0.001, "value should be strokes[3].end_price=55");

        // V空不产生下轨点
        let lower_from_vs = guidao::calc_seg_lower_band_case4(&vs_upper, &segs_upper, &strokes_upper);
        assert_eq!(lower_from_vs.len(), 0, "lower from V空 signals: expected 0");

        // === 下轨Case4: 转空→V多 ===
        // 2026-08-24 新API状态机方案:
        // seg3=down(35→5) 经 C3 升格 (5 < up0 起点 10) → 转空事件 (3,0,2)
        // seg4=up(5→40) 经 C3 升格 (40 > dn3 起点 35) → 转多事件 (4,3,3) → V多
        let strokes_lower = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 15.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 15.0, end_price: 35.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 35.0, end_price: 5.0,  start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 5.0,  end_price: 40.0, start_bar: 40, end_bar: 50, is_up: true },
        ];
        let segs_lower = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
            (4, 4, true),
        ];
        let highs_l = vec![40.0f64; 51];
        let mut lows_l = vec![8.0f64; 51];
        lows_l[35] = 7.0; // ≤ 10 → 转空

        let ts_lower = guidao::detect_segment_turn_signals(&segs_lower, &big_segs, &strokes_lower, &highs_l, &lows_l);
        assert_eq!(ts_lower.len(), 2, "should have 2 turn signals (转空+转多)");
        assert!(ts_lower[0].is_turn_short, "should be 转空");
        assert_eq!(ts_lower[0].end_seg_idx, 2);

        let vs_lower = guidao::detect_segment_v_signals(&ts_lower, &segs_lower, &strokes_lower, &highs_l, &lows_l);
        assert_eq!(vs_lower.len(), 1, "should have 1 V-signal (V多)");
        assert!(vs_lower[0].is_turn_short, "V多 comes from 转空");

        let lower_c4 = guidao::calc_seg_lower_band_case4(&vs_lower, &segs_lower, &strokes_lower);
        assert_eq!(lower_c4.len(), 1, "lower Case4: expected 1 point");
        assert_eq!(lower_c4[0].bar_index, 40, "bar should be strokes[3].end_bar=40");
        assert!((lower_c4[0].value - 5.0).abs() < 0.001, "value should be strokes[3].end_price=5");

        // === 负面验证: V信号不满足条件 ===
        // up3(5→45) 未突破 dn0 起点 50 → 无 C3 转多事件 → 无 V空
        let strokes_neg = vec![
            Stroke { start_price: 50.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 40.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 40.0, end_price: 5.0,  start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 5.0,  end_price: 45.0, start_bar: 30, end_bar: 40, is_up: true },
            // up3(5→45) 未突破 dn0 起点 50 → 无 C3 事件 → 无 V空
            Stroke { start_price: 45.0, end_price: 8.0,  start_bar: 40, end_bar: 50, is_up: false },
        ];
        let segs_neg = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false),
        ];
        let ts_neg = guidao::detect_segment_turn_signals(&segs_neg, &big_segs, &strokes_neg, &highs_u, &lows_u);
        let vs_neg = guidao::detect_segment_v_signals(&ts_neg, &segs_neg, &strokes_neg, &highs_u, &lows_u);
        assert_eq!(vs_neg.len(), 0, "negative: V空 condition not met → no V-signals");

        eprintln!("✓ 线段轨道Case4 verified: V空→upper=1pt, V多→lower=1pt, negative=0pt");
    }

    /// 验证大段轨道Case1 (三重顶/底): 上轨三重顶 + 下轨三重底 + 负面验证
    #[test]
    fn test_bigseg_band_case1() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case1: 上升大段三重顶 ===
        // 3个上升大段, 高点依次降低: 50 > 40 > 30
        // 每个大段=1个上升线段, end_seg的最后一笔就是该大段的高点
        let strokes_upper = vec![
            Stroke { start_price: 10.0, end_price: 50.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 50.0, end_price: 30.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 30.0, end_price: 40.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 40.0, end_price: 20.0, start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 20.0, end_price: 30.0, start_bar: 40, end_bar: 50, is_up: true },
            Stroke { start_price: 30.0, end_price: 10.0, start_bar: 50, end_bar: 60, is_up: false },
        ];
        let segs_upper = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
            (4, 4, true),  (5, 5, false),
        ];
        // 每个大段=单独的上升线段, end_seg的last stroke就是上升笔, end_price=高点
        let big_segs_upper = vec![
            (0, 0, true),  // high=strokes[0].end_price=50, bar=strokes[0].end_bar=10
            (2, 2, true),  // high=strokes[2].end_price=40, bar=strokes[2].end_bar=30
            (4, 4, true),  // high=strokes[4].end_price=30, bar=strokes[4].end_bar=50
        ];
        let (upper_bp, up_big) = guidao::calc_bigseg_upper_band_case1(&big_segs_upper, &segs_upper, &strokes_upper);
        assert_eq!(upper_bp.len(), 1, "upper Case1: expected 1 point");
        assert_eq!(upper_bp[0].bar_index, 10, "bar should be strokes[0].end_bar=10");
        assert!((upper_bp[0].value - 50.0).abs() < 0.001, "value should be 50");
        assert_eq!(up_big.len(), 3, "should have 3 up big-segments");

        // === 下轨Case1: 下降大段三重底 ===
        // 3个下降大段, 低点依次升高: 10 < 15 < 20
        let strokes_lower = vec![
            Stroke { start_price: 50.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 30.0, end_price: 15.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 15.0, end_price: 35.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 35.0, end_price: 20.0, start_bar: 40, end_bar: 50, is_up: false },
            Stroke { start_price: 20.0, end_price: 40.0, start_bar: 50, end_bar: 60, is_up: true },
        ];
        let segs_lower = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false), (5, 5, true),
        ];
        let big_segs_lower = vec![
            (0, 0, false), // low=strokes[0].end_price=10, bar=strokes[0].end_bar=10
            (2, 2, false), // low=strokes[2].end_price=15, bar=strokes[2].end_bar=30
            (4, 4, false), // low=strokes[4].end_price=20, bar=strokes[4].end_bar=50
        ];
        let (lower_bp, down_big) = guidao::calc_bigseg_lower_band_case1(&big_segs_lower, &segs_lower, &strokes_lower);
        assert_eq!(lower_bp.len(), 1, "lower Case1: expected 1 point");
        assert_eq!(lower_bp[0].bar_index, 10, "bar should be strokes[0].end_bar=10");
        assert!((lower_bp[0].value - 10.0).abs() < 0.001, "value should be 10");
        assert_eq!(down_big.len(), 3, "should have 3 down big-segments");

        // === 负面验证: 高点依次升高 → 不满足三重顶 ===
        let strokes_neg = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 20.0, end_price: 40.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 40.0, end_price: 15.0, start_bar: 30, end_bar: 40, is_up: false },
            Stroke { start_price: 15.0, end_price: 50.0, start_bar: 40, end_bar: 50, is_up: true },
            Stroke { start_price: 50.0, end_price: 5.0,  start_bar: 50, end_bar: 60, is_up: false },
        ];
        let segs_neg = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
            (4, 4, true),  (5, 5, false),
        ];
        let big_segs_neg = vec![
            (0, 0, true),  // high=30
            (2, 2, true),  // high=40
            (4, 4, true),  // high=50  → ascending → no triple-top
        ];
        let (neg_bp, _) = guidao::calc_bigseg_upper_band_case1(&big_segs_neg, &segs_neg, &strokes_neg);
        assert_eq!(neg_bp.len(), 0, "negative: ascending highs → no triple-top");

        eprintln!("✓ 大段轨道Case1 verified: 上轨三重顶=1pt, 下轨三重底=1pt, negative=0pt");
    }

    /// 验证大段轨道Case2 (1.618扩张): 上轨下降大段扩张 + 下轨上升大段扩张 + 负面验证
    #[test]
    fn test_bigseg_band_case2() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // === 上轨Case2: 下降大段1.618扩张 ===
        // 下降大段1: 50→30 (空间20), 下降大段2: 45→10 (空间35 >= 20*1.618=32.36) → 扩张且 50>45
        let strokes_upper = vec![
            Stroke { start_price: 50.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 30.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 45.0, end_price: 10.0, start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 10.0, end_price: 25.0, start_bar: 30, end_bar: 40, is_up: true },
        ];
        let segs_upper = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
        ];
        let big_segs_upper = vec![
            (0, 0, false), // high=strokes[0].start_price=50, low=strokes[0].end_price=30, bar=strokes[0].start_bar=0
            (2, 2, false), // high=strokes[2].start_price=45, low=strokes[2].end_price=10, bar=strokes[2].start_bar=20
        ];
        let upper_c2 = guidao::calc_bigseg_upper_band_case2(&big_segs_upper, &segs_upper, &strokes_upper);
        assert_eq!(upper_c2.len(), 1, "upper Case2: expected 1 point");
        assert_eq!(upper_c2[0].bar_index, 0, "bar should be strokes[0].start_bar=0");
        assert!((upper_c2[0].value - 50.0).abs() < 0.001, "value should be first_high=50");

        // === 下轨Case2: 上升大段1.618扩张 ===
        // 上升大段1: 10→30 (空间20), 上升大段2: 15→50 (空间35 >= 20*1.618=32.36) → 扩张且 10<15
        let strokes_lower = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 15.0, end_price: 50.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 50.0, end_price: 35.0, start_bar: 30, end_bar: 40, is_up: false },
        ];
        let segs_lower = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
        ];
        let big_segs_lower = vec![
            (0, 0, true), // high=strokes[0].end_price=30, low=strokes[0].start_price=10, bar=strokes[0].end_bar=10
            (2, 2, true), // high=strokes[2].end_price=50, low=strokes[2].start_price=15, bar=strokes[2].end_bar=30
        ];
        let lower_c2 = guidao::calc_bigseg_lower_band_case2(&big_segs_lower, &segs_lower, &strokes_lower);
        assert_eq!(lower_c2.len(), 1, "lower Case2: expected 1 point");
        assert_eq!(lower_c2[0].bar_index, 10, "bar should be strokes[0].end_bar=10");
        assert!((lower_c2[0].value - 10.0).abs() < 0.001, "value should be first_low=10");

        // === 负面验证: 空间不足1.618 → 不扩张 ===
        // 上升大段1: 10→30 (空间20), 上升大段2: 15→25 (空间10 < 20*1.618=32.36)
        let strokes_neg = vec![
            Stroke { start_price: 10.0, end_price: 30.0, start_bar: 0,  end_bar: 10, is_up: true },
            Stroke { start_price: 30.0, end_price: 20.0, start_bar: 10, end_bar: 20, is_up: false },
            Stroke { start_price: 15.0, end_price: 25.0, start_bar: 20, end_bar: 30, is_up: true },
            Stroke { start_price: 25.0, end_price: 18.0, start_bar: 30, end_bar: 40, is_up: false },
        ];
        let segs_neg = vec![
            (0, 0, true),  (1, 1, false),
            (2, 2, true),  (3, 3, false),
        ];
        let big_segs_neg = vec![
            (0, 0, true),  // space=20
            (2, 2, true),  // space=10
        ];
        let neg_c2 = guidao::calc_bigseg_lower_band_case2(&big_segs_neg, &segs_neg, &strokes_neg);
        assert_eq!(neg_c2.len(), 0, "negative: space < 1.618 → no expansion");

        eprintln!("✓ 大段轨道Case2 verified: 上轨扩张=1pt, 下轨扩张=1pt, negative=0pt");
    }

    /// 验证大段轨道Case3 (新API, 2026-08-24): 高级段状态机 Case3 升格 → 转多/转空
    /// 旧 inner_big 方案 (build_inner_big_segments) 已随库移除 (提交 9386187: 伪转多/伪转空 BUG 根源)
    #[test]
    fn test_bigseg_band_case3() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // big_segs: dn→up→dn→up→dn→up; dn0 C2 成立(end=2) → up3 终点 45 > dn0 起点 40 → 转多升格 (一个大段当高级段)
        let strokes = vec![
            Stroke { start_price: 40.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 25.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 25.0, end_price: 8.0,  start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 8.0,  end_price: 45.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 45.0, end_price: 15.0, start_bar: 40, end_bar: 50, is_up: false },
            Stroke { start_price: 15.0, end_price: 30.0, start_bar: 50, end_bar: 60, is_up: true },
        ];
        let segs = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false), (5, 5, true),
        ];
        let big_segs = segs.clone();
        let superior_segments: Vec<(usize, usize, bool)> = vec![]; // 终态兑底 = Case2 段终点 end_i=2

        let ts = guidao::detect_big_seg_turn_signals(&big_segs, &segs, &strokes, &superior_segments);
        assert_eq!(ts.len(), 1, "should have 1 turn signal (转多)");
        assert!(!ts[0].is_turn_short, "should be 转多");
        assert_eq!(ts[0].end_seg_idx, 2, "终态大段 = 兑底 end_i=2");
        assert_eq!(ts[0].trigger_seg_idx, 3, "触发大段 j=3");

        let lower_c3 = guidao::calc_bigseg_lower_band_case3(&ts, &big_segs, &segs, &strokes);
        assert_eq!(lower_c3.len(), 1, "lower Case3: expected 1 point");
        assert_eq!(lower_c3[0].bar_index, 30, "bar should be strokes[2].end_bar=30");
        assert!((lower_c3[0].value - 8.0).abs() < 0.001, "value should be strokes[2].end_price=8");

        // 转多不产生上轨点
        let upper_from_ts = guidao::calc_bigseg_upper_band_case3(&ts, &big_segs, &segs, &strokes);
        assert_eq!(upper_from_ts.len(), 0, "upper from 转多 signals: expected 0");

        // 负面验证: j+1 无紧邻反向升格 → 无 V 信号
        let vs = guidao::detect_big_v_signals(&ts);
        assert_eq!(vs.len(), 0, "negative: no adjacent reverse upgrade → no V-signal");

        eprintln!("✓ 大段轨道Case3 (新API) verified: 转多→下轨1pt, 上轨0pt, V=0");
    }

    /// 验证大段轨道Case4 (新API, 2026-08-24): 转多 j 之后 j+1 紧邻反向升格 → V空
    #[test]
    fn test_bigseg_band_case4() {
        use chanlun_lean_lib::Stroke;
        use chanlun_lean_lib::guidao;

        // big_segs: dn→up→dn→up→dn→up; up3 C3 升格 (转多, j=3) 后 dn4 终点 3 < up3 起点 8 → 紧邻转空升格 (j=4) → V空
        let strokes = vec![
            Stroke { start_price: 40.0, end_price: 10.0, start_bar: 0,  end_bar: 10, is_up: false },
            Stroke { start_price: 10.0, end_price: 25.0, start_bar: 10, end_bar: 20, is_up: true },
            Stroke { start_price: 25.0, end_price: 8.0,  start_bar: 20, end_bar: 30, is_up: false },
            Stroke { start_price: 8.0,  end_price: 45.0, start_bar: 30, end_bar: 40, is_up: true },
            Stroke { start_price: 45.0, end_price: 3.0,  start_bar: 40, end_bar: 50, is_up: false }, // 终点 3 < up3 起点 8 → 转空升格
            Stroke { start_price: 3.0,  end_price: 15.0, start_bar: 50, end_bar: 60, is_up: true },
        ];
        let segs = vec![
            (0, 0, false), (1, 1, true),
            (2, 2, false), (3, 3, true),
            (4, 4, false), (5, 5, true),
        ];
        let big_segs = segs.clone();
        let superior_segments: Vec<(usize, usize, bool)> = vec![];

        let ts = guidao::detect_big_seg_turn_signals(&big_segs, &segs, &strokes, &superior_segments);
        assert_eq!(ts.len(), 2, "should have 2 turn signals (转多 j=3 + 转空 j=4)");
        assert!(!ts[0].is_turn_short, "ts[0] should be 转多");
        assert_eq!(ts[0].trigger_seg_idx, 3);
        assert!(ts[1].is_turn_short, "ts[1] should be 转空");
        assert_eq!(ts[1].trigger_seg_idx, 4);

        let vs = guidao::detect_big_v_signals(&ts);
        assert_eq!(vs.len(), 1, "should have 1 V-signal (V空)");
        assert!(!vs[0].is_turn_short, "V空 comes from 转多");

        let upper_c4 = guidao::calc_bigseg_upper_band_case4(&vs, &big_segs, &segs, &strokes);
        assert_eq!(upper_c4.len(), 1, "upper Case4: expected 1 point");
        assert_eq!(upper_c4[0].bar_index, 40, "bar should be strokes[3].end_bar=40 (触发大段 j=3 终点)");
        assert!((upper_c4[0].value - 45.0).abs() < 0.001, "value should be strokes[3].end_price=45");

        // V空不产生下轨点
        let lower_from_vs = guidao::calc_bigseg_lower_band_case4(&vs, &big_segs, &segs, &strokes);
        assert_eq!(lower_from_vs.len(), 0, "lower from V空 signals: expected 0");

        eprintln!("✓ 大段轨道Case4 (新API) verified: 转多+紧邻转空→V空→上轨1pt, 下轨0pt");
    }
    /// 参数化: mark 46/49 设置 case 参数并失效重建管线 (2026-09-05)
    #[test]
    fn test_set_cases_invalidates_pipeline_and_table_registered() {
        // 1) 函数表须注册 mark 46/49 (TDX lookup 按 n_func_mark; 走真实注册路径)
        let mut marks: Vec<u16> = Vec::new();
        unsafe {
            let mut p: *mut PluginTCalcFuncInfo = std::ptr::null_mut();
            RegisterTdxFunc(&mut p);
            assert!(!p.is_null(), "RegisterTdxFunc 必须返回函数表");
            let mut i = 0usize;
            loop {
                let e = std::ptr::read(p.add(i));
                if e.n_func_mark == 0 { break; }   // sentinel 结束
                marks.push(e.n_func_mark);
                i += 1;
            }
        }
        assert!(marks.contains(&46) && marks.contains(&49), "函数表须注册 mark 46/49, marks={marks:?}");

        // 2) 默认全开: 设置 0 后管线可建
        let highs: Vec<f64> = (0..40).map(|i| 100.0 + ((i % 10) as f64) * 0.5).collect();
        let lows: Vec<f64> = (0..40).map(|i| 99.0 + ((i % 10) as f64) * 0.5).collect();
        unsafe {
            let mode0: f32 = 0.0;
            let out: Vec<f32> = vec![0.0; 40];
            set_stroke_cases_fn(40, out.as_ptr() as *mut f32, highs.as_ptr() as *const f64 as *mut f32, lows.as_ptr() as *const f64 as *mut f32, &mode0 as *const f32 as *mut f32);
            set_level_cases_fn(40, out.as_ptr() as *mut f32, highs.as_ptr() as *const f64 as *mut f32, lows.as_ptr() as *const f64 as *mut f32, &mode0 as *const f32 as *mut f32);
        }
        let (sc, lc) = TL_CASES.with(|c| *c.borrow());
        assert_eq!((sc, lc), (0, 0), "默认全开");
        let _p = get_pipeline(highs.clone(), lows.clone());
        assert!(PIPELINE_CACHE.lock().unwrap().is_some(), "管线应已构建");

        // 3) 改参 (5,5) → 清缓存 → 重建后 TL_CASES 生效
        unsafe {
            let mode5: f32 = 5.0;
            let out: Vec<f32> = vec![0.0; 40];
            set_stroke_cases_fn(40, out.as_ptr() as *mut f32, highs.as_ptr() as *const f64 as *mut f32, lows.as_ptr() as *const f64 as *mut f32, &mode5 as *const f32 as *mut f32);
            set_level_cases_fn(40, out.as_ptr() as *mut f32, highs.as_ptr() as *const f64 as *mut f32, lows.as_ptr() as *const f64 as *mut f32, &mode5 as *const f32 as *mut f32);
        }
        let (sc2, lc2) = TL_CASES.with(|c| *c.borrow());
        assert_eq!((sc2, lc2), (5, 5), "参数应写入 (5,5)");
        assert!(PIPELINE_CACHE.lock().unwrap().is_none(), "参数变化应清管线缓存");

        // 复位避免污染其他测试 (同线程共享 TL_CASES)
        unsafe {
            let mode0: f32 = 0.0;
            let out: Vec<f32> = vec![0.0; 40];
            set_stroke_cases_fn(40, out.as_ptr() as *mut f32, highs.as_ptr() as *const f64 as *mut f32, lows.as_ptr() as *const f64 as *mut f32, &mode0 as *const f32 as *mut f32);
            set_level_cases_fn(40, out.as_ptr() as *mut f32, highs.as_ptr() as *const f64 as *mut f32, lows.as_ptr() as *const f64 as *mut f32, &mode0 as *const f32 as *mut f32);
        }
    }
}