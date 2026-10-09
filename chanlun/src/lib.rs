//! 缠论算法纯Rust库 — 零依赖, 可嵌入任意Rust项目
//! 分型/笔/线段/大段/高级段构建 + 三级轨道 + 大段中枢 + 买卖点标记

#[cfg(feature = "guidao")]
pub mod guidao;
#[cfg(feature = "guidao")]
pub mod zhongshu;

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

fn check_original_valid(valid_fractals: &[Fractal], i: usize, last: &Fractal, current: &Fractal) -> bool {
    for j in (0..i).rev() {

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

fn check_up_stroke_case4(valid: &[Fractal], i: usize, low_a: f64, high_a: f64) -> bool {
    let n = valid.len();
    if i + 2 >= n { return false; } 
    
    let mut down_end: usize = usize::MAX; 
    let mut up_start: usize = usize::MAX; 
    let mut up_end: usize = usize::MAX;
    let mut k = i;
    while k + 1 < n {
        if down_end == usize::MAX {
            if valid[k].is_top {

                let first_high = valid[k].price;
                let mut prev_low = valid[k + 1].price;
                let mut t = k + 2;
                let mut closed = false;
                while t + 1 < n {
                    if valid[t].is_top {
                        let cur_high = valid[t].price;
                        let cur_low = valid[t + 1].price;
                        if cur_high >= first_high { break; } 
                        if cur_low < prev_low { down_end = t; closed = true; break; }
                        prev_low = cur_low;
                    }
                    t += 2;
                }
                if closed { k = down_end; } 
            }
        } else if !valid[k].is_top {
            
            let first_low = valid[k].price;
            let mut prev_high = valid[k + 1].price;
            let mut t = k + 2;
            let mut closed = false;
            while t + 1 < n {
                if !valid[t].is_top {
                    let cur_low = valid[t].price;
                    let cur_high = valid[t + 1].price;
                    if cur_low <= first_low { break; } 
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
    

    let mut real_low = valid[i + 1].price;
    let mut j = i + 1;
    while j < up_start && j + 1 < n {
        if valid[j].is_top { real_low = real_low.min(valid[j + 1].price); }
        j += 1;
    }
    if real_low <= low_a { return false; } 

    let mut real_high = valid[i + 1].price;
    let mut j = i + 1;
    while j <= up_end && j + 1 < n {
        if !valid[j].is_top { real_high = real_high.max(valid[j + 1].price); }
        j += 1;
    }
    if real_high > high_a { return false; } 
    true
}

fn check_down_stroke_case4(valid: &[Fractal], i: usize, high_a: f64, low_a: f64) -> bool {
    let n = valid.len();
    if i + 2 >= n { return false; }
    
    let mut up_end: usize = usize::MAX; 
    let mut down_start: usize = usize::MAX; 
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
    

    let mut real_high = valid[i + 1].price;
    let mut j = i + 1;
    while j < down_start && j + 1 < n {
        if !valid[j].is_top { real_high = real_high.max(valid[j + 1].price); }
        j += 1;
    }
    if real_high >= high_a { return false; } 

    let mut real_low = valid[i + 1].price;
    let mut j = i + 1;
    while j <= down_end && j + 1 < n {
        if valid[j].is_top { real_low = real_low.min(valid[j + 1].price); }
        j += 1;
    }
    if real_low < low_a { return false; } 
    true
}

fn reverse_stroke_confirmed(valid: &[Fractal], i: usize, case4_enabled: bool, min_bars: usize) -> bool {
    let total = valid.len();
    if i + 1 >= total { return false; }
    let x = &valid[i];
    let d = &valid[i + 1];
    if x.is_top == d.is_top { return false; }

    let bar_diff = x.bar_index.abs_diff(d.bar_index);
    let merged_diff = x.merged_index.abs_diff(d.merged_index);
    if bar_diff >= min_bars && merged_diff > 1 && check_original_valid(valid, i + 1, x, d) {
        return true;
    }

    let (c2, _) = if x.is_top {
        check_down_stroke_case2(valid, total, i + 1, x.price, d.price)
    } else {
        check_up_stroke_case2(valid, total, i + 1, x.price, d.price)
    };
    if c2 { return true; }
    
    if case4_enabled {
        return if x.is_top {
            check_down_stroke_case4(valid, i + 1, x.price, d.price)
        } else {
            check_up_stroke_case4(valid, i + 1, x.price, d.price)
        };
    }
    false
}

pub fn process_strokes_fractals(valid_fractals: &[Fractal], case3_enabled: bool, min_bars: usize) -> Vec<Fractal> {
    process_strokes_fractals_with_cases(valid_fractals, case3_enabled, true, min_bars)
}

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

            
            let mut valid = false;
            let mut next_i = i + 1;
            let mut end_fractal_idx = i;


            if bar_diff >= min_bars {
                let merged_index_diff = if last.merged_index > current.merged_index { last.merged_index - current.merged_index } else { current.merged_index - last.merged_index };
                if merged_index_diff > 1 {
                    if check_original_valid(valid_fractals, i, last, current) { valid = true; }
                }
            }


            if !valid {
                if is_up {
                    let (ok, end_idx) = check_up_stroke_case2(valid_fractals, total, i, last.price, current.price);
                    if ok {
                        valid = true;
                        next_i = end_idx + 1;
                        end_fractal_idx = end_idx;


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

            

            


            


            
            

            if !valid && case3_enabled && last_idx >= 1 {
                let k = last_idx - 1;
                let broken = final_fractals[k].is_top == current.is_top
                    && if is_up { current.price > final_fractals[k].price } else { current.price < final_fractals[k].price };
                if broken {
                    


                    if !reverse_stroke_confirmed(valid_fractals, i, case4_enabled, min_bars) {
                        i += 1;
                        continue;
                    }
                    final_fractals.truncate(last_idx);
                    final_fractals[last_idx - 1] = current.clone();
                    i += 1;
                    continue;
                }
            }

            



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

    





    


    if final_fractals.len() >= 2 {
        let a0 = &final_fractals[0];
        if let Some(a0_idx) = valid_fractals
            .iter()
            .position(|f| f.bar_index == a0.bar_index && f.merged_index == a0.merged_index)
        {

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
    prev_rev_seg_start: Option<usize>,
) -> (bool, usize, usize) {
    let count = strokes.len();
    if start_idx >= count || !strokes[start_idx].is_up { return (false, start_idx, usize::MAX); }
    let Some(base) = prev_rev_seg_start else { return (false, start_idx, usize::MAX); };
    let Some(base_stroke) = strokes.get(base) else { return (false, start_idx, usize::MAX); };
    let high_a = strokes[start_idx].end_price;
    (high_a > base_stroke.start_price, start_idx, base)
}

fn check_down_segment_case3(
    strokes: &[Stroke],
    start_idx: usize,
    prev_rev_seg_start: Option<usize>,
) -> (bool, usize, usize) {
    let count = strokes.len();
    if start_idx >= count || strokes[start_idx].is_up { return (false, start_idx, usize::MAX); }
    let Some(base) = prev_rev_seg_start else { return (false, start_idx, usize::MAX); };
    let Some(base_stroke) = strokes.get(base) else { return (false, start_idx, usize::MAX); };
    let low_a = strokes[start_idx].end_price;
    (low_a < base_stroke.start_price, start_idx, base)
}

fn prev_rev_seg_start(segs: &[(usize, usize, bool)], cur_is_up: bool) -> Option<usize> {
    segs.last().filter(|s| s.2 != cur_is_up).map(|s| s.0)
}

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

    

    let real_up_high = get_real_seg_end_price(strokes, start_idx + 1, true, down_seg_start as usize);
    if real_up_high >= high_a { return (false, start_idx); }

    let real_down_low = get_real_seg_end_price(strokes, start_idx + 1, false, down_seg_end as usize + 1);
    if real_down_low < low_a { return (false, start_idx); }

    (true, start_idx)
}

pub fn process_segments(strokes: &[Stroke]) -> Vec<(usize, usize, bool)> {
    
    process_segments_with_cases(strokes, true, true)
}

pub fn process_segments_with_cases(
    strokes: &[Stroke],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    let count = strokes.len();
    if count < 2 { return vec![]; }


    let mut seg_case2_cache: Vec<(bool, usize)> = Vec::with_capacity(count);
    for i in 0..count {
        if strokes[i].is_up { seg_case2_cache.push(check_up_segment_case2(strokes, i)); }
        else { seg_case2_cache.push(check_down_segment_case2(strokes, i)); }
    }

    let mut segments: Vec<(usize, usize, bool)> = Vec::new();
    let mut c3_seg_starts: Vec<bool> = vec![false; count];
    let mut i: usize = 0;
    let mut looking_for_up = true;
    


    let mut first = true;

    while i < count {
        let mut found = false;
        let mut best_end: usize = 0;
        let mut is_c3 = false;

        if looking_for_up && strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();

            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            if enable_case3 {
                let (ok_c3, _, _) = check_up_segment_case3(strokes, i, prev_rev_seg_start(&segments, strokes[i].is_up));
                if ok_c3 { candidates.push(i); is_c3 = true; }
            }

            if enable_case4 {
                let (ok_c4, _) = check_up_segment_case4(strokes, i);
                if ok_c4 { candidates.push(i); }
            }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();

            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            if enable_case3 {
                let (ok_c3, _, _) = check_down_segment_case3(strokes, i, prev_rev_seg_start(&segments, strokes[i].is_up));
                if ok_c3 { candidates.push(i); is_c3 = true; }
            }

            if enable_case4 {
                let (ok_c4, _) = check_down_segment_case4(strokes, i);
                if ok_c4 { candidates.push(i); }
            }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !strokes[i].is_up {



            let mut candidates: Vec<usize> = Vec::new();

            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            if enable_case3 {
                let (ok_c3, _, _) = check_down_segment_case3(strokes, i, prev_rev_seg_start(&segments, strokes[i].is_up));
                if ok_c3 { candidates.push(i); is_c3 = true; }
            }

            if enable_case4 {
                let (ok_c4, _) = check_down_segment_case4(strokes, i);
                if ok_c4 { candidates.push(i); }
            }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i { c3_seg_starts[i] = true; }


            let first_seg_down = first && !strokes[i].is_up;

            segments.push((i, best_end, strokes[i].is_up));
            i = best_end + 1;
            if !first_seg_down { looking_for_up = !looking_for_up; }
            first = false;
        } else { i += 1; }
    }


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

pub fn collect_segment_case3_events(strokes: &[Stroke]) -> Vec<(usize, usize, usize, bool)> {
    collect_segment_case3_events_with_cases(strokes, true, true)
}

pub fn collect_segment_case3_events_with_cases(
    strokes: &[Stroke],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, usize, bool)> {
    let count = strokes.len();
    if count < 2 { return vec![]; }


    let mut seg_case2_cache: Vec<(bool, usize)> = Vec::with_capacity(count);
    for i in 0..count {
        if strokes[i].is_up { seg_case2_cache.push(check_up_segment_case2(strokes, i)); }
        else { seg_case2_cache.push(check_down_segment_case2(strokes, i)); }
    }

    let mut events: Vec<(usize, usize, usize, bool)> = Vec::new();
    
    let mut built: Vec<(usize, usize, bool)> = Vec::new();
    let mut c3_seg_starts: Vec<bool> = vec![false; count];
    let mut i: usize = 0;
    let mut looking_for_up = true;
    

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
            let (ok_c3, _, b) = check_up_segment_case3(strokes, i, prev_rev_seg_start(&built, strokes[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_up_segment_case4(strokes, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !strokes[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(strokes, i, prev_rev_seg_start(&built, strokes[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(strokes, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !strokes[i].is_up {

            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = seg_case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(strokes, i, prev_rev_seg_start(&built, strokes[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(strokes, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i {
                c3_seg_starts[i] = true;

                let end_i = if seg_case2_cache[base_idx].0 { seg_case2_cache[base_idx].1 } else { base_idx };

                events.push((i, base_idx, end_i, strokes[i].is_up));
            }

            let first_seg_down = first && !strokes[i].is_up;
            built.push((i, best_end, strokes[i].is_up));
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

pub fn process_big_segments(strokes: &[Stroke], segs: &[(usize, usize, bool)]) -> Vec<(usize, usize, bool)> {
    process_big_segments_with_cases(strokes, segs, true, true)
}

pub fn process_big_segments_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    if segs.len() < 2 { return vec![]; }


    let projected: Vec<Stroke> = segs
        .iter()
        .map(|(si, ei, is_up)| Stroke {
            is_up: *is_up,
            start_price: strokes[*si].start_price,
            end_price: strokes[*ei].end_price,
            start_bar: 0,  
            end_bar: 0,
        })
        .collect();

    
    process_segments_with_cases(&projected, enable_case3, enable_case4)
}

pub fn collect_superior_case3_events(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
) -> Vec<(usize, usize, usize, bool)> {
    collect_superior_case3_events_with_cases(strokes, segs, bigs, true, true)
}

pub fn collect_superior_case3_events_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, usize, bool)> {
    if bigs.len() < 2 { return vec![]; }


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
    
    let mut built: Vec<(usize, usize, bool)> = Vec::new();
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
            let (ok_c3, _, b) = check_up_segment_case3(&projected, i, prev_rev_seg_start(&built, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_up_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(&projected, i, prev_rev_seg_start(&built, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !projected[i].is_up {

            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, b) = check_down_segment_case3(&projected, i, prev_rev_seg_start(&built, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; base_idx = b; }
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }
            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i {
                c3_seg_starts[i] = true;

                let end_i = if case2_cache[base_idx].0 { case2_cache[base_idx].1 } else { base_idx };
                events.push((i, base_idx, end_i, projected[i].is_up));
            }

            let first_seg_down = first && !projected[i].is_up;
            built.push((i, best_end, projected[i].is_up));
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

pub fn process_superior_segments(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
) -> Vec<(usize, usize, bool)> {
    process_superior_segments_with_cases(strokes, segs, bigs, true, true)
}

pub fn process_superior_segments_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    if bigs.len() < 2 { return vec![]; }


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

            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            let (ok_c3, _, _) = check_up_segment_case3(&projected, i, prev_rev_seg_start(&superior_segs, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }

            let (ok_c4, _) = check_up_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();

            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            let (ok_c3, _, _) = check_down_segment_case3(&projected, i, prev_rev_seg_start(&superior_segs, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }

            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !projected[i].is_up {




            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            let (ok_c3, _, _) = check_down_segment_case3(&projected, i, prev_rev_seg_start(&superior_segs, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }

            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i { c3_seg_starts[i] = true; }


            let first_seg_down = first && !projected[i].is_up;

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

pub fn process_superior_superior_segments(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<(usize, usize, bool)> {
    process_superior_superior_segments_with_cases(strokes, segs, bigs, sups, true, true)
}

pub fn process_superior_superior_segments_with_cases(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
    enable_case3: bool,
    enable_case4: bool,
) -> Vec<(usize, usize, bool)> {
    if sups.len() < 2 { return vec![]; }



    let projected: Vec<Stroke> = sups
        .iter()
        .map(|(si, ei, is_up)| {
            let seg_s = segs[bigs[*si].0]; 
            let seg_e = segs[bigs[*ei].1]; 
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

    let mut superior_superior_segs: Vec<(usize, usize, bool)> = Vec::new();
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

            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            let (ok_c3, _, _) = check_up_segment_case3(&projected, i, prev_rev_seg_start(&superior_superior_segs, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }

            let (ok_c4, _) = check_up_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if !looking_for_up && !projected[i].is_up {
            let mut candidates: Vec<usize> = Vec::new();

            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }

            let (ok_c3, _, _) = check_down_segment_case3(&projected, i, prev_rev_seg_start(&superior_superior_segs, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }

            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        } else if first && !projected[i].is_up {

            let mut candidates: Vec<usize> = Vec::new();
            let (ok_c2, end_i_c2) = case2_cache[i];
            if ok_c2 { candidates.push(end_i_c2); }
            let (ok_c3, _, _) = check_down_segment_case3(&projected, i, prev_rev_seg_start(&superior_superior_segs, projected[i].is_up));
            if enable_case3 && ok_c3 { candidates.push(i); is_c3 = true; }
            let (ok_c4, _) = check_down_segment_case4(&projected, i);
            if enable_case4 && ok_c4 { candidates.push(i); }

            if !candidates.is_empty() { candidates.sort(); best_end = candidates[0]; found = true; }
        }

        if found {
            if is_c3 && best_end == i { c3_seg_starts[i] = true; }

            let first_seg_down = first && !projected[i].is_up;

            superior_superior_segs.push((i, best_end, projected[i].is_up));
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    
    if !superior_superior_segs.is_empty() {
        for s in 0..superior_superior_segs.len() {
            let end_si = superior_superior_segs[s].1;
            let is_up = superior_superior_segs[s].2;
            let next_start_si = if s + 1 < superior_superior_segs.len() { superior_superior_segs[s + 1].0 } else { count };

            for si in end_si + 1..next_start_si {
                if si < count && projected[si].is_up == is_up {
                    let cand_price = projected[si].end_price;
                    if is_up && cand_price >= projected[superior_superior_segs[s].1].end_price {
                        superior_superior_segs[s].1 = si;
                    } else if !is_up && cand_price <= projected[superior_superior_segs[s].1].end_price {
                        superior_superior_segs[s].1 = si;
                    }
                }
            }
        }
    }

    superior_superior_segs
}

#[inline]
fn host_case2_ok(host_parts_proj: &[Stroke], host_start: usize) -> bool {
    match host_parts_proj.get(host_start) {
        Some(h) => {
            if h.is_up {
                check_up_segment_case2(host_parts_proj, host_start).0
            } else {
                check_down_segment_case2(host_parts_proj, host_start).0
            }
        }
        None => false,
    }
}

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

pub(crate) fn resolve_sup_start_point(
    sup_start_big_idx: usize,
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, f64)> {
    let big = *bigs.get(sup_start_big_idx)?;
    let seg = *segs.get(big.0)?;
    let st = strokes.get(seg.0)?;
    Some((st.start_bar, st.start_price))
}

pub(crate) fn resolve_seg_end_point(
    seg_idx: usize,
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, f64)> {
    let seg = *segments.get(seg_idx)?;
    let st = strokes.get(seg.1)?;
    Some((st.end_bar, st.end_price))
}

pub(crate) fn resolve_seg_start_point(
    seg_idx: usize,
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, f64)> {
    let seg = *segments.get(seg_idx)?;
    let st = strokes.get(seg.0)?;
    Some((st.start_bar, st.start_price))
}

pub const SECOND_RETRACE_MIN: f64 = 0.75;

#[inline]
fn second_gate_buy(b: f64, a: f64, c: f64) -> bool {
    a > b && (c - b) >= (a - b) * SECOND_RETRACE_MIN
}

#[inline]
fn second_gate_sell(b: f64, a: f64, c: f64) -> bool {
    b > a && (b - c) >= (b - a) * SECOND_RETRACE_MIN
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondMarker {
    pub bar_index: usize,
    pub price: f64,

    pub is_buy: bool,
}

pub fn detect_second_buy_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<SecondMarker> {
    


    
    if sups.is_empty() { return vec![]; }


    

    

    let big_proj: Vec<Stroke> = bigs
        .iter()
        .map(|&(si, ei, is_up)| {
            let (_, sp) = resolve_seg_start_point(si, segs, strokes).unwrap_or((0, 0.0));
            let (_, ep) = resolve_seg_end_point(ei, segs, strokes).unwrap_or((0, 0.0));
            Stroke { is_up, start_price: sp, end_price: ep, start_bar: 0, end_bar: 0 }
        })
        .collect();
    let mut markers: Vec<SecondMarker> = Vec::new();

    for &(sup_start, ei, is_up) in sups {
        if is_up { continue; } 
        
        if !host_case2_ok(&big_proj, sup_start) { continue; }


        let Some((_, b_price)) = resolve_sup_end_point(ei, bigs, segs, strokes) else { continue; };


        let mut a_price: Option<f64> = None;
        for k in (0..ei).rev() {
            if bigs[k].2 {
                a_price = resolve_sup_end_point(k, bigs, segs, strokes).map(|(_, p)| p);
                break;
            }
        }
        if let Some(a_price) = a_price {

            let mut c_idx: usize = 0;
            let mut c_ok = false;
            for k in ei + 1..bigs.len() {
                if bigs[k].2 {
                    if let Some((_, price)) = resolve_sup_end_point(k, bigs, segs, strokes) {
                        if second_gate_buy(b_price, a_price, price) {
                            c_idx = k;
                            c_ok = true;
                        }
                    }
                    break;
                }
            }
            if c_ok {

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
    
    
    if sups.is_empty() { return vec![]; }


    
    let big_proj: Vec<Stroke> = bigs
        .iter()
        .map(|&(si, ei, is_up)| {
            let (_, sp) = resolve_seg_start_point(si, segs, strokes).unwrap_or((0, 0.0));
            let (_, ep) = resolve_seg_end_point(ei, segs, strokes).unwrap_or((0, 0.0));
            Stroke { is_up, start_price: sp, end_price: ep, start_bar: 0, end_bar: 0 }
        })
        .collect();
    let mut markers: Vec<SecondMarker> = Vec::new();

    for &(sup_start, ei, is_up) in sups {
        if !is_up { continue; } 
        
        if !host_case2_ok(&big_proj, sup_start) { continue; }


        let Some((_, b_price)) = resolve_sup_end_point(ei, bigs, segs, strokes) else { continue; };


        let mut a_price: Option<f64> = None;
        for k in (0..ei).rev() {
            if !bigs[k].2 {
                a_price = resolve_sup_end_point(k, bigs, segs, strokes).map(|(_, p)| p);
                break;
            }
        }
        if let Some(a_price) = a_price {

            let mut c_idx: usize = 0;
            let mut c_ok = false;
            for k in ei + 1..bigs.len() {
                if !bigs[k].2 {
                    if let Some((_, price)) = resolve_sup_end_point(k, bigs, segs, strokes) {
                        if second_gate_sell(b_price, a_price, price) {
                            c_idx = k;
                            c_ok = true;
                        }
                    }
                    break;
                }
            }
            if c_ok {

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

fn resolve_sup2_end_point(
    sup_end_sup_idx: usize,
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, f64)> {
    let sup = *sups.get(sup_end_sup_idx)?;
    resolve_sup_end_point(sup.1, bigs, segs, strokes)
}

pub fn detect_superior_second_buy_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<SecondMarker> {
    
    let sup2s = process_superior_superior_segments(strokes, segs, bigs, sups);
    

    
    if sup2s.is_empty() { return vec![]; }


    let sup_proj: Vec<Stroke> = sups
        .iter()
        .map(|&(si, ei, is_up)| {
            let (_, sp) = resolve_sup_start_point(si, bigs, segs, strokes).unwrap_or((0, 0.0));
            let (_, ep) = resolve_sup_end_point(ei, bigs, segs, strokes).unwrap_or((0, 0.0));
            Stroke { is_up, start_price: sp, end_price: ep, start_bar: 0, end_bar: 0 }
        })
        .collect();
    let mut markers: Vec<SecondMarker> = Vec::new();

    for &(si, ei, is_up) in &sup2s {
        if is_up { continue; } 
        
        if !host_case2_ok(&sup_proj, si) { continue; }


        let Some((_, b_price)) = resolve_sup2_end_point(ei, sups, bigs, segs, strokes) else { continue; };


        let mut a_price: Option<f64> = None;
        for k in (0..ei).rev() {
            if sups[k].2 {
                a_price = resolve_sup_end_point(sups[k].1, bigs, segs, strokes).map(|(_, p)| p);
                break;
            }
        }
        if let Some(a_price) = a_price {

            let mut c_idx: usize = 0;
            let mut c_ok = false;
            for k in ei + 1..sups.len() {
                if sups[k].2 {
                    if let Some((_, price)) = resolve_sup_end_point(sups[k].1, bigs, segs, strokes) {
                        if second_gate_buy(b_price, a_price, price) {
                            c_idx = k;
                            c_ok = true;
                        }
                    }
                    break;
                }
            }
            if c_ok {

                for k in c_idx + 1..sups.len() {
                    if !sups[k].2 {
                        if let Some((bar, price)) = resolve_sup_end_point(sups[k].1, bigs, segs, strokes) {
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

pub fn detect_superior_second_sell_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<SecondMarker> {
    
    let sup2s = process_superior_superior_segments(strokes, segs, bigs, sups);
    
    if sup2s.is_empty() { return vec![]; }


    let sup_proj: Vec<Stroke> = sups
        .iter()
        .map(|&(si, ei, is_up)| {
            let (_, sp) = resolve_sup_start_point(si, bigs, segs, strokes).unwrap_or((0, 0.0));
            let (_, ep) = resolve_sup_end_point(ei, bigs, segs, strokes).unwrap_or((0, 0.0));
            Stroke { is_up, start_price: sp, end_price: ep, start_bar: 0, end_bar: 0 }
        })
        .collect();
    let mut markers: Vec<SecondMarker> = Vec::new();

    for &(si, ei, is_up) in &sup2s {
        if !is_up { continue; } 
        
        if !host_case2_ok(&sup_proj, si) { continue; }


        let Some((_, b_price)) = resolve_sup2_end_point(ei, sups, bigs, segs, strokes) else { continue; };


        let mut a_price: Option<f64> = None;
        for k in (0..ei).rev() {
            if !sups[k].2 {
                a_price = resolve_sup_end_point(sups[k].1, bigs, segs, strokes).map(|(_, p)| p);
                break;
            }
        }
        if let Some(a_price) = a_price {

            let mut c_idx: usize = 0;
            let mut c_ok = false;
            for k in ei + 1..sups.len() {
                if !sups[k].2 {
                    if let Some((_, price)) = resolve_sup_end_point(sups[k].1, bigs, segs, strokes) {
                        if second_gate_sell(b_price, a_price, price) {
                            c_idx = k;
                            c_ok = true;
                        }
                    }
                    break;
                }
            }
            if c_ok {

                for k in c_idx + 1..sups.len() {
                    if sups[k].2 {
                        if let Some((bar, price)) = resolve_sup_end_point(sups[k].1, bigs, segs, strokes) {
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

pub fn detect_superior_cf_buy_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<guidao::SecondPlusOneMarker> {
    let sup2s = process_superior_superior_segments(strokes, segs, bigs, sups);
    if sup2s.is_empty() { return vec![]; }


    let down_sups: Vec<(usize, f64)> = sups.iter().filter(|s| !s.2)
        .filter_map(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
    let up_sups: Vec<(usize, f64)> = sups.iter().filter(|s| s.2)
        .filter_map(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes)).collect();

    let mut markers: Vec<guidao::SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_sup, is_up)) in sup2s.iter().enumerate() {
        if is_up { continue; } 

        
        let Some((dbe, d_price)) = resolve_sup2_end_point(end_sup, sups, bigs, segs, strokes) else { continue; };

        
        let mut f: Option<(usize, f64)> = None;
        for i in 2..down_sups.len() {
            let (b0, l0) = down_sups[i - 2];
            let (_b1, l1) = down_sups[i - 1];
            let (b2, l2) = down_sups[i];
            if b0 >= dbe && l2 > l0 && l1 > l0 {
                f = Some((b2, l2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        
        
        let next_down_sup2 = sup2s[k + 1..].iter().find(|s| !s.2)
            .and_then(|&(_, eb, _)| resolve_sup2_end_point(eb, sups, bigs, segs, strokes));
        if let Some((nb, _)) = next_down_sup2 {
            if third_bar >= nb { continue; }
        }

        
        let Some((_, c_price)) = up_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
        if c_price > f_price {
            markers.push(guidao::SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });
            
            
            let second_up = up_sups.iter().filter(|(b, _)| *b > dbe).nth(1);
            let ref_high = second_up.map(|(_, h)| c_price.min(*h));
            if let Some(rh) = ref_high {
                let mut order = 2;
                for (xb, xl) in down_sups.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_down_sup2 {
                        if *xb >= b { break; }
                    }
                    if rh > *xl && *xl > d_price {
                        markers.push(guidao::SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                        order += 1;
                    }
                }
            }
        }
    }

    

    


    if let Some(&(first_start, _, true)) = sup2s.first() {
        if first_start > 0 {
            let first_down_sup2_end = sup2s.iter().find(|s| !s.2)
                .and_then(|&(_, eb, _)| resolve_sup2_end_point(eb, sups, bigs, segs, strokes));
            for k in 0..first_start {
                if sups[k].2 { continue; } 
                let Some((dbe, d_price)) = resolve_sup_end_point(sups[k].1, bigs, segs, strokes) else { continue; };
                
                let mut f: Option<(usize, f64)> = None;
                for i in 2..down_sups.len() {
                    let (b0, l0) = down_sups[i - 2];
                    let (_b1, l1) = down_sups[i - 1];
                    let (b2, l2) = down_sups[i];
                    if b0 >= dbe && l2 > l0 && l1 > l0 {
                        f = Some((b2, l2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };

                if let Some((nb, _)) = first_down_sup2_end {
                    if third_bar >= nb { continue; }
                }
                
                let Some((_, c_price)) = up_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                if c_price > f_price {
                    markers.push(guidao::SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });

                    let second_up = up_sups.iter().filter(|(b, _)| *b > dbe).nth(1);
                    let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                    if let Some(rh) = ref_high {
                        let mut order = 2;
                        for (xb, xl) in down_sups.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_down_sup2_end {
                                if *xb >= b { break; }
                            }
                            if rh > *xl && *xl > d_price {
                                markers.push(guidao::SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
                                order += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    

    
    

    
    if let Some(&(first_start, _, true)) = sup2s.first() {
        if first_start == 0 {

            if let Some((dbe, d_price)) = resolve_sup_start_point(sups[0].0, bigs, segs, strokes) {
                
                let mut bottoms: Vec<(usize, f64)> = Vec::with_capacity(down_sups.len() + 1);
                bottoms.push((dbe, d_price));
                bottoms.extend(down_sups.iter().filter(|(b, _)| *b > dbe).copied());

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

                    let first_down_sup2_end = sup2s.iter().find(|s| !s.2)
                        .and_then(|&(_, eb, _)| resolve_sup2_end_point(eb, sups, bigs, segs, strokes));
                    let window_ok = match first_down_sup2_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        
                        let c = up_sups.iter().find(|(b, _)| *b > dbe).copied();
                        if let Some((_, c_price)) = c {
                            if c_price > f_price {
                                markers.push(guidao::SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: true, order: 1 });

                                let second_up = up_sups.iter().filter(|(b, _)| *b > dbe).nth(1);
                                let ref_high = second_up.map(|(_, h)| c_price.min(*h));
                                if let Some(rh) = ref_high {
                                    let mut order = 2;
                                    for (xb, xl) in down_sups.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_down_sup2_end {
                                            if *xb >= b { break; }
                                        }
                                        if rh > *xl && *xl > d_price {
                                            markers.push(guidao::SecondPlusOneMarker { bar_index: *xb, price: *xl, is_buy: true, order });
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

pub fn detect_superior_cf_sell_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<guidao::SecondPlusOneMarker> {
    let sup2s = process_superior_superior_segments(strokes, segs, bigs, sups);
    if sup2s.is_empty() { return vec![]; }

    let down_sups: Vec<(usize, f64)> = sups.iter().filter(|s| !s.2)
        .filter_map(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
    let up_sups: Vec<(usize, f64)> = sups.iter().filter(|s| s.2)
        .filter_map(|&(_, eb, _)| resolve_sup_end_point(eb, bigs, segs, strokes)).collect();

    let mut markers: Vec<guidao::SecondPlusOneMarker> = Vec::new();
    for (k, &(_, end_sup, is_up)) in sup2s.iter().enumerate() {
        if !is_up { continue; } 

        
        let Some((dbe, d_price)) = resolve_sup2_end_point(end_sup, sups, bigs, segs, strokes) else { continue; };

        
        let mut f: Option<(usize, f64)> = None;
        for i in 2..up_sups.len() {
            let (b0, h0) = up_sups[i - 2];
            let (_b1, h1) = up_sups[i - 1];
            let (b2, h2) = up_sups[i];
            if b0 >= dbe && h2 < h0 && h1 < h0 {
                f = Some((b2, h2));
                break;
            }
        }
        let Some((third_bar, f_price)) = f else { continue; };

        
        let next_up_sup2 = sup2s[k + 1..].iter().find(|s| s.2)
            .and_then(|&(_, eb, _)| resolve_sup2_end_point(eb, sups, bigs, segs, strokes));
        if let Some((nb, _)) = next_up_sup2 {
            if third_bar >= nb { continue; }
        }

        
        let Some((_, c_price)) = down_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
        if c_price < f_price {
            markers.push(guidao::SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
            
            
            let second_down = down_sups.iter().filter(|(b, _)| *b > dbe).nth(1);
            let ref_low = second_down.map(|(_, l)| c_price.max(*l));
            if let Some(rl) = ref_low {
                let mut order = 2;
                for (xb, xh) in up_sups.iter().filter(|(b, _)| *b > third_bar) {
                    if let Some((b, _)) = next_up_sup2 {
                        if *xb >= b { break; }
                    }
                    if rl < *xh && *xh < d_price {
                        markers.push(guidao::SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                        order += 1;
                    }
                }
            }
        }
    }

    

    

    if let Some(&(first_start, _, false)) = sup2s.first() {
        if first_start > 0 {
            let first_up_sup2_end = sup2s.iter().find(|s| s.2)
                .and_then(|&(_, eb, _)| resolve_sup2_end_point(eb, sups, bigs, segs, strokes));
            for k in 0..first_start {
                if !sups[k].2 { continue; } 
                let Some((dbe, d_price)) = resolve_sup_end_point(sups[k].1, bigs, segs, strokes) else { continue; };
                
                let mut f: Option<(usize, f64)> = None;
                for i in 2..up_sups.len() {
                    let (b0, h0) = up_sups[i - 2];
                    let (_b1, h1) = up_sups[i - 1];
                    let (b2, h2) = up_sups[i];
                    if b0 >= dbe && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                let Some((third_bar, f_price)) = f else { continue; };

                if let Some((nb, _)) = first_up_sup2_end {
                    if third_bar >= nb { continue; }
                }
                
                let Some((_, c_price)) = down_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                if c_price < f_price {
                    markers.push(guidao::SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                    
                    let second_down = down_sups.iter().filter(|(b, _)| *b > dbe).nth(1);
                    let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                    if let Some(rl) = ref_low {
                        let mut order = 2;
                        for (xb, xh) in up_sups.iter().filter(|(b, _)| *b > third_bar) {
                            if let Some((b, _)) = first_up_sup2_end {
                                if *xb >= b { break; }
                            }
                            if rl < *xh && *xh < d_price {
                                markers.push(guidao::SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
                                order += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    

    

    
    if let Some(&(first_start, _, false)) = sup2s.first() {
        if first_start == 0 {

            if let Some((dbe, d_price)) = resolve_sup_start_point(sups[0].0, bigs, segs, strokes) {
                
                let mut tops: Vec<(usize, f64)> = Vec::with_capacity(up_sups.len() + 1);
                tops.push((dbe, d_price));
                tops.extend(up_sups.iter().filter(|(b, _)| *b > dbe).copied());

                let mut f: Option<(usize, f64)> = None;
                for i in 2..tops.len() {
                    let (b0, h0) = tops[i - 2];
                    let (_b1, h1) = tops[i - 1];
                    let (b2, h2) = tops[i];
                    if b0 >= dbe && h2 < h0 && h1 < h0 {
                        f = Some((b2, h2));
                        break;
                    }
                }
                if let Some((third_bar, f_price)) = f {

                    let first_up_sup2_end = sup2s.iter().find(|s| s.2)
                        .and_then(|&(_, eb, _)| resolve_sup2_end_point(eb, sups, bigs, segs, strokes));
                    let window_ok = match first_up_sup2_end {
                        Some((nb, _)) => third_bar < nb,
                        None => true,
                    };
                    if window_ok {
                        
                        let c = down_sups.iter().find(|(b, _)| *b > dbe).copied();
                        if let Some((_, c_price)) = c {
                            if c_price < f_price {
                                markers.push(guidao::SecondPlusOneMarker { bar_index: third_bar, price: f_price, is_buy: false, order: 1 });
                                
                                let second_down = down_sups.iter().filter(|(b, _)| *b > dbe).nth(1);
                                let ref_low = second_down.map(|(_, l)| c_price.max(*l));
                                if let Some(rl) = ref_low {
                                    let mut order = 2;
                                    for (xb, xh) in up_sups.iter().filter(|(b, _)| *b > third_bar) {
                                        if let Some((b, _)) = first_up_sup2_end {
                                            if *xb >= b { break; }
                                        }
                                        if rl < *xh && *xh < d_price {
                                            markers.push(guidao::SecondPlusOneMarker { bar_index: *xb, price: *xh, is_buy: false, order });
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrokeCases {
    pub case3: bool,
    pub case4: bool,
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

impl Default for StrokeCases {
    fn default() -> Self {
        Self { case3: true, case4: true }
    }
}

impl StrokeCases {
    

    pub fn from_param(v: u8) -> Self {
        match v {
            3 => Self { case3: false, case4: true },
            4 => Self { case3: true, case4: false },
            5 | 34 => Self { case3: false, case4: false },
            _ => Self::default(),
        }
    }
}

impl ChanlunPipeline {
    pub fn new(highs: Vec<f64>, lows: Vec<f64>) -> Self {
        Self::new_with_cases(highs, lows, StrokeCases::default(), StrokeCases::default())
    }

    

    pub fn new_with_stroke_cases(highs: Vec<f64>, lows: Vec<f64>, cases: StrokeCases) -> Self {
        Self::new_with_cases(highs, lows, cases, StrokeCases::default())
    }

    


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

