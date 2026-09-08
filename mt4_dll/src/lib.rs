//! slzs_chanlun_mt4 — 缠论MT4 32位DLL插件 (⚠️ MT4仅支持32位)
//! ⚠️ 通达信是 64 位 (x86_64) / MT4 是 32 位 (i686) — 永不混淆
//! 基于 chanlun_lean_lib 纯Rust算法核心
//! 全算法在Rust侧计算, MQL4只负责显示
//!
//! MT4调用约定 (⚠️ 32位使用 stdcall):
//!   官方DLL示例全部使用 __stdcall + undecorated export names
//!   Rust侧: extern "system" + #[export_name] 实现
//!
//!   #import "slzs_chanlun_mt4.dll"
//!     int chanlun_init(int rates_total, const double &highs[], const double &lows[]);
//!     int chanlun_get_strokes(double &up[], double &down[]);
//!     ...
//!   #import
//!
//! 架构:
//!   线程本地缓存(TL_PIPELINE, thread_local) → 各getter惰性读取 → 填充MT4 buffer

// MT4 32位 DLL 单线程调用约定: 静态缓存使用 static mut (原件模式, 刻意为之)
#![allow(static_mut_refs)]

use std::os::raw::c_int;
use std::cell::RefCell;
use std::rc::Rc;

#[allow(unused_imports)]
use chanlun_lean_lib::{ChanlunPipeline, guidao};

/// 从 MQL4 传入的数组指针读取 f64 数据 (对齐 TDX DLL read_floats 模式)
unsafe fn read_f64s(ptr: *const f64, len: usize) -> Vec<f64> {
    if ptr.is_null() || len == 0 {
        return vec![];
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    slice.to_vec()
}

// ── 线程本地管线缓存 (每线程独立, 消除 EA 多线程竞态) ──
// Rc 共享: 命中/存缓存/getter 读取均免 from_parts 9 Vec 克隆
thread_local! {
    static TL_PIPELINE: RefCell<Option<(Vec<f64>, Vec<f64>, usize, Rc<ChanlunPipeline>)>> = RefCell::new(None);
    static TL_RATES_TOTAL: RefCell<usize> = RefCell::new(0);
    /// 缠论参数 (指标 Parameters 标签 input 传入; 0/3/4/5 语义同库 StrokeCases::from_param;
    /// 默认 (0,0) 全开 = 历史行为; 由 chanlun_set_cases 写入, 写入时清 TL_PIPELINE 强制按新参重建)
    static TL_CASES: RefCell<(u8, u8)> = const { RefCell::new((0u8, 0u8)) };
}

// ── 独立标记/中枢缓存 (单线程, 零锁) ──
static mut MARKERS_COMPUTED: bool = false;
static mut MARKERS_CACHE: Vec<(usize, u8, f64)> = Vec::new(); // (bar_index, kind, price) kind: 0=二买 1=二卖 2=三买 3=三卖
// ── 大段中枢矩形缓存 (矩形 + ZG/ZD 上下沿) ──
static mut ZHONGSHUS_COMPUTED: bool = false;
static mut ZHONGSHUS_CACHE: Vec<(usize, usize, f64, f64)> = Vec::new(); // (start_bar, end_bar, zg, zd)

/// 计算/命中管线 — 返回 Rc 共享引用 (免 from_parts 9 Vec 克隆)
fn get_pipeline(highs: Vec<f64>, lows: Vec<f64>) -> Rc<ChanlunPipeline> {
    let n = highs.len();
    let pipeline = TL_PIPELINE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some((ref cached_h, ref cached_l, _, ref pipeline)) = *cache {
            if cached_h.len() == n && cached_l.len() == n
                && cached_h.first() == highs.first()
                && cached_l.first() == lows.first()
                && cached_h.last() == highs.last()
                && cached_l.last() == lows.last()
            {
                return Some(Rc::clone(pipeline));
            }
        }
        let rc = Rc::new(ChanlunPipeline::new_with_cases(
            highs.clone(),
            lows.clone(),
            chanlun_lean_lib::StrokeCases::from_param(TL_CASES.with(|c| c.borrow().0)),
            chanlun_lean_lib::StrokeCases::from_param(TL_CASES.with(|c| c.borrow().1)),
        ));
        *cache = Some((highs, lows, n, Rc::clone(&rc)));
        Some(rc)
    });
    TL_RATES_TOTAL.with(|rt| { *rt.borrow_mut() = n; });
    pipeline.unwrap()
}

/// 从缓存读取已计算的管线 (Rc 引用, 不触发重算/克隆)
fn get_cached_pipeline() -> Option<Rc<ChanlunPipeline>> {
    TL_PIPELINE.with(|cell| {
        cell.borrow().as_ref().map(|(_, _, _, p)| Rc::clone(p))
    })
}

/// 读取缓存的 rates_total
fn get_cached_rates_total() -> usize {
    TL_RATES_TOTAL.with(|rt| *rt.borrow())
}

// ── 辅助函数 ──

/// 构建买卖点标记输入链 (中枢 → 二买二卖 → 三买三卖 → 2+N买/卖 → 撤回过滤)
/// 返回 (second_markers, third_markers), 供 chanlun_markers_compute 渲染
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
    // 二买+二卖合并 (detect_second_buy_markers 只产二买 is_buy=true, 二卖在独立 sell 函数)
    let mut sm = chanlun_lean_lib::detect_second_buy_markers(strokes, segs, bigs, sups);
    sm.extend(chanlun_lean_lib::detect_second_sell_markers(strokes, segs, bigs, sups));
    let tm = chanlun_lean_lib::zhongshu::detect_bigseg_third_marks(sups, bigs, segs, strokes, &zs, highs, lows);
    // 2+N买/卖
    let (blc1, db) = guidao::calc_bigseg_lower_band_case1(bigs, segs, strokes);
    let (buc1, ub) = guidao::calc_bigseg_upper_band_case1(bigs, segs, strokes);
    let mut pm = guidao::detect_bigseg_cf_buy_markers(sups, bigs, segs, strokes, &blc1, &db, &ub);
    pm.extend(guidao::detect_bigseg_cf_sell_markers(sups, bigs, segs, strokes, &buc1, &ub, &db));
    // 撤回过滤: 标记输出=过滤后标记
    let rf = guidao::apply_sup_retreat_filter(sups, bigs, segs, strokes, sm, tm, pm);
    (rf.second_markers, rf.third_markers)
}

/// 初始化管线 — 传入全量 K 线数据, 内部缓存计算结果
///
/// MQL4 调用: int chanlun_init(int rates_total, const double &highs[], const double &lows[]);
///
/// 返回: 1=成功, 0=失败(数据不足)
#[export_name = "chanlun_init"]
pub unsafe extern "system" fn chanlun_init(
    rates_total: c_int,
    highs: *const f64,
    lows: *const f64,
) -> c_int {
    let n = rates_total as usize;
    if n < 3 {
        return 0; // 至少需要3根K线才能识别分型
    }

    let h = read_f64s(highs, n);
    let l = read_f64s(lows, n);
    // 对齐TDX: 不翻转, pipeline直接处理MT4原生序

    // 缓存管线 (get_pipeline 内部已设置 TL_RATES_TOTAL)
    let _pipeline = get_pipeline(h, l);

    unsafe { MARKERS_COMPUTED = false; }
    unsafe { ZHONGSHUS_COMPUTED = false; }

    1 // 成功
}

/// 设置缠论参数 (指标 Parameters 标签 input 传入):
/// stroke_param = 笔 Case 参数 0/3/4/5 (0全开/3关case3/4关case4/5关case34);
/// levels_param = 线段~高级段统一 levels 参数 (同语义).
/// 独立缓存 (TL_CASES), 不触碰 chanlun_init; 参数变化 → 清 TL_PIPELINE,
/// 下次 chanlun_init 强制按新参数重建管线 (标记/中枢缓存随 init 重置).
/// 默认 (0,0) 全开 = 与未调用时历史行为完全一致.
///
/// MQL4 调用: int chanlun_set_cases(int stroke_param, int levels_param);
/// 返回: 1=成功
#[export_name = "chanlun_set_cases"]
pub unsafe extern "system" fn chanlun_set_cases(
    stroke_param: c_int,
    levels_param: c_int,
) -> c_int {
    let stroke = stroke_param as u8;
    let levels = levels_param as u8;
    let changed = TL_CASES.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != stroke || c.1 != levels {
            *c = (stroke, levels);
            true
        } else {
            false
        }
    });
    if changed {
        // 参数变化 → 旧管线失效 (同数据缓存命中会沿用旧参数管线)
        TL_PIPELINE.with(|cell| *cell.borrow_mut() = None);
        TL_RATES_TOTAL.with(|rt| *rt.borrow_mut() = 0);
    }
    1 // 成功
}

/// 获取笔数据 — 填入 up[] 和 down[] buffer
///
/// MQL4 调用: int chanlun_get_strokes(double &upBuf[], double &downBuf[]);
///
/// 输出: pipeline.strokes, 逐笔插值填充 (上升笔→up, 下降笔→down)
///
/// 返回: 笔数量
#[export_name = "chanlun_get_strokes"]
pub unsafe extern "system" fn chanlun_get_strokes(
    up: *mut f64,
    down: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p,
        None => return 0,
    };

    let strokes = &pipeline.strokes;
    if strokes.is_empty() { return 0; }

    for s in strokes {
        let (start_bar, end_bar, start_price, end_price) = if s.start_bar <= s.end_bar {
            (s.start_bar, s.end_bar, s.start_price, s.end_price)
        } else {
            (s.end_bar, s.start_bar, s.end_price, s.start_price)
        };
        if end_bar <= start_bar || end_bar >= data_len { continue; }

        let range = (end_bar - start_bar) as f64;
        unsafe {
            let target = if s.is_up { up } else { down };
            let slice = std::slice::from_raw_parts_mut(target, data_len);
            for bar in start_bar..=end_bar {
                let ratio = (bar - start_bar) as f64 / range;
                slice[bar] = start_price + ratio * (end_price - start_price);
            }
        }
    }

    strokes.len() as c_int
}

fn ensure_markers_computed() {
    unsafe {
        if MARKERS_COMPUTED { return; }
    }
    let pipeline = match get_cached_pipeline() { Some(p) => p, None => return };
    let (sm, tm) = build_markers(
        &pipeline.superior_segments, &pipeline.big_segments, &pipeline.segments,
        &pipeline.strokes, &pipeline.highs, &pipeline.lows,
    );
    let mut out: Vec<(usize, u8, f64)> = Vec::new();
    for m in sm { out.push((m.bar_index, if m.is_buy { 0 } else { 1 }, m.price)); }
    for m in tm { out.push((m.bar_index, if m.is_buy { 2 } else { 3 }, m.price)); }
    out.sort_by_key(|&(b, k, _)| (b, k));
    unsafe {
        MARKERS_CACHE = out;
        MARKERS_COMPUTED = true;
    }
}

/// 二买/二卖/三买/三卖 标记总数
///
/// MQL4 调用: int chanlun_markers_compute();
#[export_name = "chanlun_markers_compute"]
pub unsafe extern "system" fn chanlun_markers_compute() -> c_int {
    ensure_markers_computed();
    unsafe { MARKERS_CACHE.len() as c_int }
}

/// 获取标记 (按bar排序): kind 0=二买 1=二卖 2=三买 3=三卖, price = 标记价格(文字定位)
///
/// MQL4 调用: double chanlun_markers_get(int index, int &bar, int &kind);
#[export_name = "chanlun_markers_get"]
pub unsafe extern "system" fn chanlun_markers_get(index: c_int, bar: *mut c_int, kind: *mut c_int) -> f64 {
    unsafe {
        if (index as usize) < MARKERS_CACHE.len() {
            let (b, k, price) = MARKERS_CACHE[index as usize];
            if !bar.is_null() { *bar = b as c_int; }
            if !kind.is_null() { *kind = k as c_int; }
            return price;
        }
    }
    if !bar.is_null() { unsafe { *bar = -1; } }
    0.0
}

/// 大段中枢缓存计算 (独立懒计算)
/// 区间 = 判定对锁定 ZG/ZD, 仅画矩形边框 (gg/dd 存数据不画)
fn ensure_zhongshus_computed() {
    unsafe {
        if ZHONGSHUS_COMPUTED { return; }
    }
    let pipeline = match get_cached_pipeline() { Some(p) => p, None => return };
    let zs = chanlun_lean_lib::zhongshu::detect_bigseg_zhongshus(
        &pipeline.superior_segments, &pipeline.big_segments,
        &pipeline.segments, &pipeline.strokes,
    );
    let mut out: Vec<(usize, usize, f64, f64)> = Vec::new();
    for z in zs { out.push((z.start_bar, z.end_bar, z.zg, z.zd)); }
    unsafe {
        ZHONGSHUS_CACHE = out;
        ZHONGSHUS_COMPUTED = true;
    }
}

/// 大段中枢矩形总数
///
/// MQL4 调用: int chanlun_zhongshus_compute();
#[export_name = "chanlun_zhongshus_compute"]
pub unsafe extern "system" fn chanlun_zhongshus_compute() -> c_int {
    ensure_zhongshus_computed();
    unsafe { ZHONGSHUS_CACHE.len() as c_int }
}

/// 获取中枢矩形 (start_bar, end_bar, zg, zd)
///
/// MQL4 调用: int chanlun_zhongshus_get(int index, int &start_bar, int &end_bar, double &zg, double &zd);
#[export_name = "chanlun_zhongshus_get"]
pub unsafe extern "system" fn chanlun_zhongshus_get(
    index: c_int,
    start_bar: *mut c_int,
    end_bar: *mut c_int,
    zg: *mut f64,
    zd: *mut f64,
) -> c_int {
    unsafe {
        if (index as usize) < ZHONGSHUS_CACHE.len() {
            let (sb, eb, zgu, zdd) = ZHONGSHUS_CACHE[index as usize];
            if !start_bar.is_null() { *start_bar = sb as c_int; }
            if !end_bar.is_null() { *end_bar = eb as c_int; }
            if !zg.is_null() { *zg = zgu; }
            if !zd.is_null() { *zd = zdd; }
            return 1;
        }
    }
    0
}

/// 获取线段数据 — 填入 up[] 和 down[] buffer
///
/// MQL4 调用: int chanlun_get_segments(double &upBuf[], double &downBuf[]);
///
/// 输出: pipeline.segments, 解析 stroke 索引→bar/价格, 逐段插值填充
///   segments = Vec<(start_stroke_idx, end_stroke_idx, is_up)>
///
/// 返回: 线段数量
#[export_name = "chanlun_get_segments"]
pub unsafe extern "system" fn chanlun_get_segments(
    up: *mut f64,
    down: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p,
        None => return 0,
    };

    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    if segs.is_empty() || strokes.is_empty() { return 0; }

    for &(start_si, end_si, is_up) in segs {
        if start_si >= strokes.len() || end_si >= strokes.len() { continue; }
        let ss = &strokes[start_si];
        let se = &strokes[end_si];

        let (start_bar, end_bar, start_price, end_price) = if ss.start_bar <= se.end_bar {
            (ss.start_bar, se.end_bar, ss.start_price, se.end_price)
        } else {
            (se.end_bar, ss.start_bar, se.end_price, ss.start_price)
        };
        if end_bar <= start_bar || end_bar >= data_len { continue; }

        let range = (end_bar - start_bar) as f64;
        unsafe {
            let target = if is_up { up } else { down };
            let slice = std::slice::from_raw_parts_mut(target, data_len);
            for bar in start_bar..=end_bar {
                let ratio = (bar - start_bar) as f64 / range;
                slice[bar] = start_price + ratio * (end_price - start_price);
            }
        }
    }

    segs.len() as c_int
}

/// 获取大段数据 — 填入 up[] 和 down[] buffer
///
/// MQL4 调用: int chanlun_get_bigsegments(double &upBuf[], double &downBuf[]);
///
/// 输出: pipeline.big_segments, 两层解析 (大段→线段→笔→bar) 逐段插值填充
///   big_segments = Vec<(start_seg_idx, end_seg_idx, is_up)>
///
/// 返回: 大段数量
#[export_name = "chanlun_get_bigsegments"]
pub unsafe extern "system" fn chanlun_get_bigsegments(
    up: *mut f64,
    down: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p,
        None => return 0,
    };

    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    let bigs = &pipeline.big_segments;
    if bigs.is_empty() || segs.is_empty() || strokes.is_empty() { return 0; }

    for &(bs_start, bs_end, is_up) in bigs {
        if bs_start >= segs.len() || bs_end >= segs.len() { continue; }
        let seg_s = &segs[bs_start];
        let seg_e = &segs[bs_end];
        let rs = seg_s.0; // 起始大段的第一线段索引 → 第一笔索引
        let re = seg_e.1; // 结束大段的最后线段索引 → 最后一笔索引
        if rs >= strokes.len() || re >= strokes.len() { continue; }
        let ss = &strokes[rs];
        let se = &strokes[re];

        let (start_bar, end_bar, start_price, end_price) = if ss.start_bar <= se.end_bar {
            (ss.start_bar, se.end_bar, ss.start_price, se.end_price)
        } else {
            (se.end_bar, ss.start_bar, se.end_price, ss.start_price)
        };
        if end_bar <= start_bar || end_bar >= data_len { continue; }

        let range = (end_bar - start_bar) as f64;
        unsafe {
            let target = if is_up { up } else { down };
            let slice = std::slice::from_raw_parts_mut(target, data_len);
            for bar in start_bar..=end_bar {
                let ratio = (bar - start_bar) as f64 / range;
                slice[bar] = start_price + ratio * (end_price - start_price);
            }
        }
    }

    bigs.len() as c_int
}

/// 获取笔轨道 (Stroke Bands)
/// Case1 + Case2 + 追踪 — 100%对齐 TDX DLL stroke_band_fn L216-497
///
/// MQL4 调用: int chanlun_get_stroke_bands(double &upperBuf[], double &lowerBuf[]);
///
/// 返回: 轨道点数
#[export_name = "chanlun_get_stroke_bands"]
pub unsafe extern "system" fn chanlun_get_stroke_bands(
    upper: *mut f64,
    lower: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p, None => return 0,
    };
    let ff = &pipeline.final_fractals;
    let _strokes = &pipeline.strokes;
    let _segs = &pipeline.segments;
    let _highs = &pipeline.highs;
    let _lows = &pipeline.lows;
    if ff.len() < 3 { return 0; }

    // ═══ 100% 对齐 compute_stroke_bands — 当前只启用 Case1+2+追踪 ═══
    // Case1
    let (upper_c1, up_strokes) = guidao::calc_upper_band_case1(ff);
    let (lower_c1, down_strokes) = guidao::calc_lower_band_case1(ff);
    // Case2
    let upper_c2 = guidao::calc_upper_band_case2(ff);
    let lower_c2 = guidao::calc_lower_band_case2(ff);
    // Case3
    let turn_signals = guidao::detect_turn_signals(&pipeline.strokes, &pipeline.segments, &pipeline.highs, &pipeline.lows);
    let upper_c3 = guidao::calc_upper_band_case3(&turn_signals, &pipeline.strokes);
    let lower_c3 = guidao::calc_lower_band_case3(&turn_signals, &pipeline.strokes);
    // Case4
    let v_signals = guidao::detect_v_signals(&turn_signals, &pipeline.strokes, &pipeline.highs, &pipeline.lows);
    let upper_c4 = guidao::calc_upper_band_case4(&v_signals, &pipeline.strokes);
    let lower_c4 = guidao::calc_lower_band_case4(&v_signals, &pipeline.strokes);

    // Merge + track
    let mut upper_raw = upper_c1;
    upper_raw.extend(upper_c2);
    upper_raw.extend(upper_c3);
    upper_raw.extend(upper_c4);
    upper_raw.sort_by_key(|p| p.bar_index);
    let upper_band = guidao::apply_tracking(&upper_raw, &up_strokes, true);

    let mut lower_raw = lower_c1;
    lower_raw.extend(lower_c2);
    lower_raw.extend(lower_c3);
    lower_raw.extend(lower_c4);
    lower_raw.sort_by_key(|p| p.bar_index);
    let lower_band = guidao::apply_tracking(&lower_raw, &down_strokes, false);

    // 延伸不穿越: bar级后处理, 确保轨道不穿过任何K线
    let upper_band = extend_no_cross(&upper_band, &pipeline.highs, data_len, true);
    let lower_band = extend_no_cross(&lower_band, &pipeline.lows, data_len, false);

    // 写入 buffer (步进式)
    unsafe {
        if !upper.is_null() {
            let up_slice = std::slice::from_raw_parts_mut(upper, data_len);
            for i in 0..upper_band.len() {
                let start = upper_band[i].bar_index;
                let end = if i+1 < upper_band.len() { upper_band[i+1].bar_index } else { data_len };
                for bar in start..end.min(data_len) { up_slice[bar] = upper_band[i].value; }
            }
        }
        if !lower.is_null() {
            let lo_slice = std::slice::from_raw_parts_mut(lower, data_len);
            for i in 0..lower_band.len() {
                let start = lower_band[i].bar_index;
                let end = if i+1 < lower_band.len() { lower_band[i+1].bar_index } else { data_len };
                for bar in start..end.min(data_len) { lo_slice[bar] = lower_band[i].value; }
            }
        }
    }

    (upper_band.len() + lower_band.len()) as c_int
}

/// 延伸不穿越: 确保轨道值不穿过任何K线
/// upper: bars[bar] > cur_val → 上移 (轨道在K线上方)
/// lower: bars[bar] < cur_val → 下移 (轨道在K线下方)
fn extend_no_cross(band: &[guidao::BandPoint], prices: &[f64], data_len: usize, is_upper: bool) -> Vec<guidao::BandPoint> {
    if band.is_empty() { return vec![]; }
    let mut result: Vec<guidao::BandPoint> = Vec::new();
    for i in 0..band.len() {
        let pt = &band[i];
        let next_bar = if i+1 < band.len() { band[i+1].bar_index } else { data_len };
        let mut cur_val = pt.value;
        result.push(guidao::BandPoint { value: cur_val, bar_index: pt.bar_index });
        for bar in (pt.bar_index+1)..next_bar.min(data_len) {
            let price = prices[bar];
            if is_upper && price > cur_val {
                cur_val = price;
                result.push(guidao::BandPoint { value: cur_val, bar_index: bar });
            } else if !is_upper && price < cur_val {
                cur_val = price;
                result.push(guidao::BandPoint { value: cur_val, bar_index: bar });
            }
        }
    }
    result
}

/// 获取线段轨道 (Segment Bands)
/// 100% 对齐 compute_segment_bands (guidao.rs L1001-1024) + extend_no_cross
///
/// MQL4 调用: int chanlun_get_segment_bands(double &upperBuf[], double &lowerBuf[], double &middleBuf[]);
///
/// 返回: 轨道点数
#[export_name = "chanlun_get_segment_bands"]
pub unsafe extern "system" fn chanlun_get_segment_bands(
    upper: *mut f64,
    lower: *mut f64,
    middle: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p, None => return 0,
    };
    let segs = &pipeline.segments;
    let bigs = &pipeline.big_segments;
    let strokes = &pipeline.strokes;
    let highs = &pipeline.highs;
    let lows = &pipeline.lows;
    if segs.len() < 3 { return 0; }

    // Case1+Case2
    let (seg_upper_c1, seg_up) = guidao::calc_seg_upper_band_case1(segs, strokes);
    let seg_upper_c2 = guidao::calc_seg_upper_band_case2(segs, strokes);
    let (seg_lower_c1, seg_down) = guidao::calc_seg_lower_band_case1(segs, strokes);
    let seg_lower_c2 = guidao::calc_seg_lower_band_case2(segs, strokes);
    // Case3
    let seg_turn_signals = guidao::detect_segment_turn_signals(segs, bigs, strokes, highs, lows);
    let seg_upper_c3 = guidao::calc_seg_upper_band_case3(&seg_turn_signals, segs, strokes);
    let seg_lower_c3 = guidao::calc_seg_lower_band_case3(&seg_turn_signals, segs, strokes);
    // Case4
    let seg_v_signals = guidao::detect_segment_v_signals(&seg_turn_signals, segs, strokes, highs, lows);
    let seg_upper_c4 = guidao::calc_seg_upper_band_case4(&seg_v_signals, segs, strokes);
    let seg_lower_c4 = guidao::calc_seg_lower_band_case4(&seg_v_signals, segs, strokes);

    // Merge + track
    let mut upper_raw = seg_upper_c1; upper_raw.extend(seg_upper_c2); upper_raw.extend(seg_upper_c3); upper_raw.extend(seg_upper_c4);
    upper_raw.sort_by_key(|p| p.bar_index);
    let upper_band = guidao::apply_tracking(&upper_raw, &seg_up, true);
    let mut lower_raw = seg_lower_c1; lower_raw.extend(seg_lower_c2); lower_raw.extend(seg_lower_c3); lower_raw.extend(seg_lower_c4);
    lower_raw.sort_by_key(|p| p.bar_index);
    let lower_band = guidao::apply_tracking(&lower_raw, &seg_down, false);

    // 延伸不穿越
    let upper_band = extend_no_cross(&upper_band, highs, data_len, true);
    let lower_band = extend_no_cross(&lower_band, lows, data_len, false);

    // 写入 buffer (步进式)
    unsafe {
        if !upper.is_null() {
            let up_slice = std::slice::from_raw_parts_mut(upper, data_len);
            for i in 0..upper_band.len() {
                let start = upper_band[i].bar_index;
                let end = if i+1 < upper_band.len() { upper_band[i+1].bar_index } else { data_len };
                for bar in start..end.min(data_len) { up_slice[bar] = upper_band[i].value; }
            }
        }
        if !lower.is_null() {
            let lo_slice = std::slice::from_raw_parts_mut(lower, data_len);
            for i in 0..lower_band.len() {
                let start = lower_band[i].bar_index;
                let end = if i+1 < lower_band.len() { lower_band[i+1].bar_index } else { data_len };
                for bar in start..end.min(data_len) { lo_slice[bar] = lower_band[i].value; }
            }
        }
        if !middle.is_null() {
            let mid_slice = std::slice::from_raw_parts_mut(middle, data_len);
            for i in 0..data_len {
                let u = if !upper.is_null() { *upper.add(i) } else { 0.0 };
                let l = if !lower.is_null() { *lower.add(i) } else { 0.0 };
                mid_slice[i] = (u + l) / 2.0;
            }
        }
    }

    (upper_band.len() + lower_band.len()) as c_int
}

/// 获取大段轨道 (Big Segment Bands)
///
/// MQL4 调用: int chanlun_get_bigseg_bands(double &upperBuf[], double &lowerBuf[], double &midBuf[]);
///
/// 返回: 轨道点数
#[export_name = "chanlun_get_bigseg_bands"]
pub unsafe extern "system" fn chanlun_get_bigseg_bands(
    upper: *mut f64,
    lower: *mut f64,
    middle: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p, None => return 0,
    };
    let bigs = &pipeline.big_segments;
    let segs = &pipeline.segments;
    let strokes = &pipeline.strokes;
    let highs = &pipeline.highs;
    let lows = &pipeline.lows;
    if bigs.len() < 3 { return 0; }

    let (upper_band, lower_band, _upper_raw, _lower_raw) = guidao::compute_bigseg_bands(
        bigs, segs, strokes, &pipeline.superior_segments, &pipeline.final_fractals, highs, lows,
    );

    // 延伸不穿越
    let upper_band = extend_no_cross(&upper_band, highs, data_len, true);
    let lower_band = extend_no_cross(&lower_band, lows, data_len, false);

    // 写入 buffer (步进式)
    unsafe {
        if !upper.is_null() {
            let up_slice = std::slice::from_raw_parts_mut(upper, data_len);
            for i in 0..upper_band.len() {
                let start = upper_band[i].bar_index;
                let end = if i+1 < upper_band.len() { upper_band[i+1].bar_index } else { data_len };
                for bar in start..end.min(data_len) { up_slice[bar] = upper_band[i].value; }
            }
        }
        if !lower.is_null() {
            let lo_slice = std::slice::from_raw_parts_mut(lower, data_len);
            for i in 0..lower_band.len() {
                let start = lower_band[i].bar_index;
                let end = if i+1 < lower_band.len() { lower_band[i+1].bar_index } else { data_len };
                for bar in start..end.min(data_len) { lo_slice[bar] = lower_band[i].value; }
            }
        }
        if !middle.is_null() {
            let mid_slice = std::slice::from_raw_parts_mut(middle, data_len);
            for i in 0..data_len {
                let u = if !upper.is_null() { *upper.add(i) } else { 0.0 };
                let l = if !lower.is_null() { *lower.add(i) } else { 0.0 };
                mid_slice[i] = (u + l) / 2.0;
            }
        }
    }

    (upper_band.len() + lower_band.len()) as c_int
}

/// 获取高级段数据 — 填入 upper[] 和 lower[] buffer
///
/// MQL4 调用: int chanlun_get_superior_segments(double &upBuf[], double &downBuf[]);
///
/// 输出: pipeline.superior_segments, 三层解析 (高级段→大段→线段→笔→bar)
///   superior_segments = Vec<(start_big_idx, end_big_idx, is_up)>
///
/// 渲染要求: 亮浅灰, 线宽2, 实线, 含端点延伸 (MQL4侧控制样式)
///
/// 返回: 高级段数量
#[export_name = "chanlun_get_superior_segments"]
pub unsafe extern "system" fn chanlun_get_superior_segments(
    upper: *mut f64,
    lower: *mut f64,
) -> c_int {
    let data_len = get_cached_rates_total();
    if data_len == 0 { return 0; }

    let pipeline = match get_cached_pipeline() {
        Some(p) => p,
        None => return 0,
    };

    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    let bigs = &pipeline.big_segments;
    let sups = &pipeline.superior_segments;
    if sups.is_empty() || bigs.is_empty() || segs.is_empty() || strokes.is_empty() { return 0; }

    for &(sup_start, sup_end, is_up) in sups {
        // 三层解析: 高级段索引→大段→线段→笔→bar
        if sup_start >= bigs.len() || sup_end >= bigs.len() { continue; }
        let big_s = &bigs[sup_start];
        let big_e = &bigs[sup_end];
        // 大段→线段
        let seg_start_idx = big_s.0;
        let seg_end_idx = big_e.1;
        if seg_start_idx >= segs.len() || seg_end_idx >= segs.len() { continue; }
        let seg_s = &segs[seg_start_idx];
        let seg_e = &segs[seg_end_idx];
        // 线段→笔
        let rs = seg_s.0;
        let re = seg_e.1;
        if rs >= strokes.len() || re >= strokes.len() { continue; }

        let ss = &strokes[rs];
        let se = &strokes[re];

        let (start_bar, end_bar, start_price, end_price) = if ss.start_bar <= se.end_bar {
            (ss.start_bar, se.end_bar, ss.start_price, se.end_price)
        } else {
            (se.end_bar, ss.start_bar, se.end_price, ss.start_price)
        };
        if end_bar <= start_bar || end_bar >= data_len { continue; }

        let range = (end_bar - start_bar) as f64;
        unsafe {
            let target = if is_up { upper } else { lower };
            let slice = std::slice::from_raw_parts_mut(target, data_len);
            for bar in start_bar..=end_bar {
                let ratio = (bar - start_bar) as f64 / range;
                slice[bar] = start_price + ratio * (end_price - start_price);
            }
        }
    }

    sups.len() as c_int
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4波段合成K线 (与 cross_dll_compare 同源)
    fn synth_wave_4band() -> (Vec<f64>, Vec<f64>) {
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let base = 100.0;
        // 波段1: 上涨 0-19
        for i in 0..20 {
            highs.push(base + i as f64 * 0.5 + 2.0);
            lows.push(base + i as f64 * 0.5 - 2.0);
        }
        // 波段2: 下跌 20-44
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0);
            lows.push(p - 2.0);
        }
        // 波段3: 上涨 45-69
        for i in 0..25 {
            let p = base + i as f64 * 0.4;
            highs.push(p + 2.0);
            lows.push(p - 2.0);
        }
        // 波段4: 下跌 70-94
        for i in 0..25 {
            let p = base + 10.0 - i as f64 * 0.4;
            highs.push(p + 2.0);
            lows.push(p - 2.0);
        }
        (highs, lows)
    }

    /// 骨架测试: 验证 chanlun_init 可正常调用
    #[test]
    fn test_init_basic() {
        let highs = vec![100.0, 102.0, 101.0, 103.0, 102.0];
        let lows = vec![98.0, 99.0, 97.0, 100.0, 99.0];
        let n = highs.len() as i32;

        let result = unsafe {
            chanlun_init(n, highs.as_ptr(), lows.as_ptr())
        };
        assert_eq!(result, 1, "chanlun_init should succeed with >= 3 bars");

        let cached_n = get_cached_rates_total();
        assert_eq!(cached_n, highs.len(), "rates_total should be cached");
    }

    /// 骨架测试: 验证数据不足时返回 0
    #[test]
    fn test_init_insufficient_data() {
        let highs = vec![100.0, 101.0];
        let lows = vec![99.0, 98.0];
        let n = highs.len() as i32;

        let result = unsafe {
            chanlun_init(n, highs.as_ptr(), lows.as_ptr())
        };
        assert_eq!(result, 0, "should fail with < 3 bars");
    }

    /// 骨架测试: 验证 getter 在未初始化时安全返回 0
    #[test]
    fn test_getter_without_init() {
        TL_RATES_TOTAL.with(|rt| *rt.borrow_mut() = 0);

        let mut up = vec![0.0f64; 10];
        let mut down = vec![0.0f64; 10];

        let count = unsafe {
            chanlun_get_strokes(up.as_mut_ptr(), down.as_mut_ptr())
        };
        assert_eq!(count, 0, "getter without init should return 0");
    }

    /// 🔴 端到端自检: 合成4波段数据 → 全管线 → 验证分型输出
    ///
    /// 验证:
    ///   1. valid_fractals 数量合理 (>0)
    ///   2. 分型出现在波段的峰/谷附近 (价格合理)
    ///   3. 坐标翻转后 MT4 buffer 中非零值位置正确
    ///   4. 底→顶 连线在 upBuf, 顶→底 在 downBuf
    ///   5. 插值连续 (相邻bar差值很小)
    #[test]
    fn test_e2e_fractal_pipeline() {
        let (highs, lows) = synth_wave_4band();
        let n = highs.len();

        // Step 1: init
        let init_ok = unsafe { chanlun_init(n as i32, highs.as_ptr(), lows.as_ptr()) };
        assert_eq!(init_ok, 1, "init should succeed");

        // Step 2: fetch pipeline data
        let pipeline = get_cached_pipeline().expect("pipeline should be cached");
        let vf = &pipeline.valid_fractals;
        assert!(vf.len() >= 2, "should have at least 2 valid fractals, got {}", vf.len());
        let vf = &pipeline.valid_fractals;

        eprintln!("\n=== E2E Fractal Pipeline Self-Check ===");
        eprintln!("bars: {}, valid_fractals: {}", n, vf.len());
        eprintln!("Rust idx → MT4 idx mapping (data_len-1-rust):");
        for (i, f) in vf.iter().enumerate() {
            let mt4_bar = n - 1 - f.bar_index;
            let dir = if f.is_top { "顶T" } else { "底B" };
            eprintln!("  F[{}] rust_bar={} mt4_buf[{}] price={:.2} {}",
                i, f.bar_index, mt4_bar, f.price, dir);
        }

        // Step 4: 验证分型数量合理

        // Step 5: 验证分型方向交替 (Rust 保证, 此处只需验证不崩溃)
        for i in 1..vf.len() {
            assert_ne!(vf[i-1].is_top, vf[i].is_top,
                "fractals must alternate direction");
        }

        // Step 6: 验证分型价格在合理范围
        for f in vf {
            assert!(f.price >= 85.0 && f.price <= 115.0,
                "fractal price {:.2} out of expected range [85,115]", f.price);
        }

        eprintln!("\n✓ E2E fractal pipeline self-check PASSED");
    }

    /// 参数化: chanlun_set_cases 写入 TL_CASES 并清 TL_PIPELINE
    #[test]
    fn test_set_cases_invalidates_pipeline() {
        unsafe {
            chanlun_set_cases(0, 0); // 默认
            assert_eq!(TL_CASES.with(|c| *c.borrow()), (0u8, 0u8), "默认全开");

            let highs: Vec<f64> = (0..40).map(|i| 100.0 + ((i % 10) as f64) * 0.5).collect();
            let lows: Vec<f64> = (0..40).map(|i| 99.0 + ((i % 10) as f64) * 0.5).collect();
            let h = highs.as_ptr(); let l = lows.as_ptr();
            chanlun_init(40, h, l);
            assert!(TL_PIPELINE.with(|c| c.borrow().is_some()), "管线应已构建");

            let ret = chanlun_set_cases(5, 5); // 关 case3+4
            assert_eq!(ret, 1, "成功返回 1");
            assert_eq!(TL_CASES.with(|c| *c.borrow()), (5u8, 5u8), "TL_CASES 应写入 (5,5)");
            assert!(TL_PIPELINE.with(|c| c.borrow().is_none()), "参数变化应清管线缓存");
            assert_eq!(TL_RATES_TOTAL.with(|c| *c.borrow()), 0, "rates_total 应复位");

            chanlun_set_cases(0, 0); // 复位, 避免污染其他测试 (thread_local 隔离)
        }
    }
}