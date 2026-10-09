//! 大段中枢 + 高级段中枢 + 三买/三卖 + 高一买/卖 + 各级撤回过滤

use crate::Stroke;

#[derive(Debug, Clone)]
pub struct BigSegmentZhongShu {

    pub sup_idx: usize,

    pub start_bar: usize,

    pub end_bar: usize,

    pub zg: f64,

    pub zd: f64,

    pub gg: f64,

    pub dd: f64,
}

#[derive(Clone, Copy)]
struct BigSegRange {
    low: f64,
    high: f64,
    start_bar: usize,
    end_bar: usize,
}

fn resolve_bigseg_range(
    big: (usize, usize, bool),
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<BigSegRange> {
    let (seg_start, seg_end, _) = big;
    if seg_start > seg_end || seg_end >= segs.len() {
        return None;
    }
    let s = strokes.get(segs.get(seg_start)?.0)?;
    let e = strokes.get(segs.get(seg_end)?.1)?;
    Some(BigSegRange {
        low: s.start_price.min(e.end_price),
        high: s.start_price.max(e.end_price),
        start_bar: s.start_bar,
        end_bar: e.end_bar,
    })
}

pub fn detect_bigseg_zhongshus(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BigSegmentZhongShu> {
    let mut out: Vec<BigSegmentZhongShu> = Vec::new();
    for (sup_idx, &(big_start, big_end, sup_up)) in superior_segments.iter().enumerate() {
        
        if big_end < big_start + 2 || big_end >= big_segments.len() {
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} SKIP: 内部大段数<3 无反向段结构", sup_idx, big_start, big_end, sup_up);
            continue;
        }

        let Some(b1) = resolve_bigseg_range(big_segments[big_start + 1], segments, strokes) else {
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} SKIP: 位置2 段解析失败", sup_idx, big_start, big_end, sup_up);
            continue;
        };

        let single_reverse = |end_bar: usize| BigSegmentZhongShu {
            sup_idx,
            start_bar: b1.start_bar,
            end_bar,
            zg: b1.high,
            zd: b1.low,
            gg: b1.high,
            dd: b1.low,
        };
        
        if big_end == big_start + 2 {
            let zs = single_reverse(b1.end_bar);
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} 单反向段中枢 [{}..{}] zg={:.3} zd={:.3}", sup_idx, big_start, big_end, sup_up, zs.start_bar, zs.end_bar, zs.zg, zs.zd);
            out.push(zs);
            continue;
        }

        let Some(b2) = resolve_bigseg_range(big_segments[big_start + 3], segments, strokes) else {
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} SKIP: 位置4 段解析失败", sup_idx, big_start, big_end, sup_up);
            continue;
        };

        let zd = b1.low.max(b2.low);
        let zg = b1.high.min(b2.high);
        if zd >= zg {
            
            let zs = single_reverse(b1.end_bar);
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} 判定对不重叠 zd={:.3}>=zg={:.3} → 退化单反向段中枢 [{}..{}] zg={:.3} zd={:.3}", sup_idx, big_start, big_end, sup_up, zd, zg, zs.start_bar, zs.end_bar, zs.zg, zs.zd);
            out.push(zs);
            continue;
        }

        let mut end_bar = b2.end_bar;
        let mut gg = b1.high.max(b2.high);
        let mut dd = b1.low.min(b2.low);
        if let Some(bm) = resolve_bigseg_range(big_segments[big_start + 2], segments, strokes) {
            gg = gg.max(bm.high);
            dd = dd.min(bm.low);
        }



        
        let mut j = big_start + 4;
        while j <= big_end {
            let Some(br) = resolve_bigseg_range(big_segments[j], segments, strokes) else {
                break;
            };
            if br.low <= zg && br.high >= zd {
                if big_segments[j].2 != sup_up {
                    end_bar = br.end_bar;
                }
                gg = gg.max(br.high);
                dd = dd.min(br.low);
                j += 1;
            } else {
                break;
            }
        }

        out.push(BigSegmentZhongShu {
            sup_idx,
            start_bar: b1.start_bar,
            end_bar,
            zg,
            zd,
            gg,
            dd,
        });
        log::debug!("[zs] sup#{} 大段[{}..{}] up={} 重叠中枢 [{}..{}] zg={:.3} zd={:.3} gg={:.3} dd={:.3}", sup_idx, big_start, big_end, sup_up, b1.start_bar, end_bar, zg, zd, gg, dd);
    }
    out
}

#[derive(Debug, Clone)]
pub struct SegmentZhongShu {

    pub big_idx: usize,
    pub start_bar: usize,
    pub end_bar: usize,
    pub zg: f64,
    pub zd: f64,
    pub gg: f64,
    pub dd: f64,
}

#[derive(Debug, Clone)]
pub struct BigSegThirdMarker {

    pub bar_index: usize,

    pub price: f64,

    pub is_buy: bool,
}

pub fn detect_bigseg_third_marks(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    zhongshus: &[BigSegmentZhongShu],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSegThirdMarker> {
    let mut out = Vec::new();
    for zs in zhongshus {
        let (big_start, big_end, sup_up) = superior_segments[zs.sup_idx];
        log::debug!("[3rd] 中枢 sup#{} 矩形[{}..{}] zg={:.3} zd={:.3} 所属高级段大段[{}..{}]", zs.sup_idx, zs.start_bar, zs.end_bar, zs.zg, zs.zd, big_start, big_end);


        let mut k = big_start;
        while k <= big_end {
            if let Some(br) = resolve_bigseg_range(big_segments[k], segments, strokes) {
                log::debug!("[3rd] 中枢 sup#{} 扫离开段 k={} dir={} 区间[{}..{}] 起点{} vs 中枢右{} {}", zs.sup_idx, k, if big_segments[k].2 { "up" } else { "down" }, br.start_bar, br.end_bar, br.start_bar, zs.end_bar, if br.start_bar >= zs.end_bar { "→ 命中离开段" } else { "" });
                if br.start_bar >= zs.end_bar {
                    break;
                }
            }
            k += 1;
        }
        

        
        
        
        
        

        let next_zs_start = zhongshus
            .iter()
            .filter(|z| z.start_bar > zs.end_bar)
            .map(|z| z.start_bar)
            .min()
            .unwrap_or(usize::MAX);

        
        

        
        
        while k + 1 < big_segments.len() {
            let Some(r1) = resolve_bigseg_range(big_segments[k + 1], segments, strokes) else {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} SKIP: 回试段解析失败", zs.sup_idx, k);
                break;
            };
            
            if r1.end_bar >= next_zs_start {
                log::debug!("[3rd] 回试终点bar={} ≥ 下一条中枢成型点 {} ⇒ 越界, 停", r1.end_bar, next_zs_start);
                break;
            }
            let is_up_leave = big_segments[k].2;
            
            


            
            if is_up_leave != sup_up {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} 离开dir={} 宿主高级段dir={} ⇒ 方向不符, 跳过",
                    zs.sup_idx, k, if is_up_leave { "up" } else { "down" }, if sup_up { "up" } else { "down" });
                k += 1;
                continue;
            }
            let hit = if is_up_leave {

                r1.end_bar < lows.len() && lows[r1.end_bar] > zs.zg
            } else {

                r1.end_bar < highs.len() && highs[r1.end_bar] < zs.zd
            };
            let reason = if is_up_leave {
                if r1.end_bar >= lows.len() {
                    format!("回试终点bar {} 越界 lows.len={}", r1.end_bar, lows.len())
                } else {
                    format!("回试终点bar {} 最低价 {:.3} {} ZG {:.3}", r1.end_bar, lows[r1.end_bar], if hit { ">" } else { "<=" }, zs.zg)
                }
            } else if r1.end_bar >= highs.len() {
                format!("回试终点bar {} 越界 highs.len={}", r1.end_bar, highs.len())
            } else {
                format!("回试终点bar {} 最高价 {:.3} {} ZD {:.3}", r1.end_bar, highs[r1.end_bar], if hit { "<" } else { ">=" }, zs.zd)
            };
            log::debug!("[3rd] 中枢 sup#{} 判定对 k={} 离开dir={} 回试段区间[{}..{}] {} → {}", zs.sup_idx, k, if is_up_leave { "up" } else { "down" }, r1.start_bar, r1.end_bar, reason, if hit { "✅ 标记" } else { "未命中" });
            if hit {
                
                if !out.iter().any(|m: &BigSegThirdMarker| m.bar_index == r1.end_bar && m.is_buy == is_up_leave) {
                    out.push(BigSegThirdMarker {
                        bar_index: r1.end_bar,
                        price: if is_up_leave { lows[r1.end_bar] } else { highs[r1.end_bar] },
                        is_buy: is_up_leave,
                    });
                }
                break;
            }
            k += 1;
        }
    }
    


    
    


    if let Some(&(first_start, _, true)) = superior_segments.first() {
        if first_start > 0 {
            let down_bigs: Vec<(usize, f64)> = big_segments
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.2)
                .filter_map(|(i, _)| crate::resolve_sup_end_point(i, big_segments, segments, strokes))
                .collect();
            let up_bigs: Vec<(usize, f64)> = big_segments
                .iter()
                .enumerate()
                .filter(|(_, s)| s.2)
                .filter_map(|(i, _)| crate::resolve_sup_end_point(i, big_segments, segments, strokes))
                .collect();
            
            
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(k, big_segments, segments, strokes) else { continue; };
                
                let Some((_, c_price)) = up_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                
                for (rb, _) in down_bigs.iter().filter(|(b, _)| *b > dbe) {
                    if *rb < lows.len() && lows[*rb] > c_price {
                        let dup = out.iter().any(|m| m.bar_index == *rb && m.is_buy);
                        if !dup {
                            out.push(BigSegThirdMarker {
                                bar_index: *rb,
                                price: lows[*rb],
                                is_buy: true,
                            });
                        }
                        break;
                    }
                }
            }
        }
    }
    

    
    


    if let Some(&(first_start, _, false)) = superior_segments.first() {
        if first_start > 0 {
            let down_bigs: Vec<(usize, f64)> = big_segments
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.2)
                .filter_map(|(i, _)| crate::resolve_sup_end_point(i, big_segments, segments, strokes))
                .collect();
            let up_bigs: Vec<(usize, f64)> = big_segments
                .iter()
                .enumerate()
                .filter(|(_, s)| s.2)
                .filter_map(|(i, _)| crate::resolve_sup_end_point(i, big_segments, segments, strokes))
                .collect();
            
            
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(k, big_segments, segments, strokes) else { continue; };
                
                let Some((_, c_price)) = down_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                
                for (rb, _) in up_bigs.iter().filter(|(b, _)| *b > dbe) {
                    if *rb < highs.len() && highs[*rb] < c_price {
                        let dup = out.iter().any(|m| m.bar_index == *rb && !m.is_buy);
                        if !dup {
                            out.push(BigSegThirdMarker {
                                bar_index: *rb,
                                price: highs[*rb],
                                is_buy: false,
                            });
                        }
                        break;
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostRange {
    pub is_up: bool,
    pub start_bar: usize,
    pub end_bar: usize,
}

#[derive(Debug, Clone)]
pub struct SuperiorZhongShu {

    pub sup_idx: usize,
    pub start_bar: usize,
    pub end_bar: usize,
    pub zg: f64,
    pub zd: f64,
    pub gg: f64,
    pub dd: f64,
}

fn resolve_sup_range(
    sup: (usize, usize, bool),
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(f64, f64, usize, usize)> {
    let (big_start, big_end, _) = sup;
    if big_start > big_end || big_end >= bigs.len() {
        return None;
    }
    let s = strokes.get(segs.get(bigs.get(big_start)?.0)?.0)?;
    let e = strokes.get(segs.get(bigs.get(big_end)?.1)?.1)?;
    Some((
        s.start_price.min(e.end_price),
        s.start_price.max(e.end_price),
        s.start_bar,
        e.end_bar,
    ))
}

pub fn detect_superior_zhongshus(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
) -> Vec<SuperiorZhongShu> {
    let sup2s = crate::process_superior_superior_segments(strokes, segs, bigs, sups);
    let mut out: Vec<SuperiorZhongShu> = Vec::new();
    for (sup2_idx, &(sup_start, sup_end, sup_up)) in sup2s.iter().enumerate() {
        
        if sup_end < sup_start + 2 || sup_end >= sups.len() {
            continue;
        }

        let Some(b1) = resolve_sup_range(sups[sup_start + 1], bigs, segs, strokes) else {
            continue;
        };

        let single_reverse = |end_bar: usize| SuperiorZhongShu {
            sup_idx: sup2_idx,
            start_bar: b1.2,
            end_bar,
            zg: b1.1,
            zd: b1.0,
            gg: b1.1,
            dd: b1.0,
        };
        
        if sup_end == sup_start + 2 {
            out.push(single_reverse(b1.3));
            continue;
        }

        let Some(b2) = resolve_sup_range(sups[sup_start + 3], bigs, segs, strokes) else {
            continue;
        };

        let zd = b1.0.max(b2.0);
        let zg = b1.1.min(b2.1);
        if zd >= zg {

            out.push(single_reverse(b1.3));
            continue;
        }

        let mut end_bar = b2.3;
        let mut gg = b1.1.max(b2.1);
        let mut dd = b1.0.min(b2.0);
        if let Some(bm) = resolve_sup_range(sups[sup_start + 2], bigs, segs, strokes) {
            gg = gg.max(bm.1);
            dd = dd.min(bm.0);
        }



        let mut j = sup_start + 4;
        while j <= sup_end {
            let Some(br) = resolve_sup_range(sups[j], bigs, segs, strokes) else {
                break;
            };
            if br.0 <= zg && br.1 >= zd {
                if sups[j].2 != sup_up {
                    end_bar = br.3;
                }
                gg = gg.max(br.1);
                dd = dd.min(br.0);
                j += 1;
            } else {
                break;
            }
        }

        out.push(SuperiorZhongShu {
            sup_idx: sup2_idx,
            start_bar: b1.2,
            end_bar,
            zg,
            zd,
            gg,
            dd,
        });
    }
    out
}

pub fn detect_superior_third_marks(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    zhongshus: &[SuperiorZhongShu],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSegThirdMarker> {
    let sup2s = crate::process_superior_superior_segments(strokes, segs, bigs, sups);
    let mut out = Vec::new();
    for zs in zhongshus {
        let Some(&(sup_start, sup_end, sup2_up)) = sup2s.get(zs.sup_idx) else {
            continue;
        };

        let mut k = sup_start;
        while k <= sup_end {
            if let Some(br) = resolve_sup_range(sups[k], bigs, segs, strokes) {
                if br.2 >= zs.end_bar {
                    break;
                }
            }
            k += 1;
        }
        

        


        let next_zs_start = zhongshus
            .iter()
            .filter(|z| z.start_bar > zs.end_bar)
            .map(|z| z.start_bar)
            .min()
            .unwrap_or(usize::MAX);

        
        
        
        

        while k + 1 < sups.len() {
            let Some(r1) = resolve_sup_range(sups[k + 1], bigs, segs, strokes) else {
                break;
            };
            
            
            if r1.3 >= next_zs_start {
                log::debug!("[3rd] 回试终点bar={} ≥ 下一条中枢成型点 {} ⇒ 越界, 停", r1.3, next_zs_start);
                break;
            }
            let is_up_leave = sups[k].2;
            

            
            if is_up_leave != sup2_up {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} 离开dir={} 宿主高高级段dir={} ⇒ 方向不符, 跳过",
                    zs.sup_idx, k, if is_up_leave { "up" } else { "down" }, if sup2_up { "up" } else { "down" });
                k += 1;
                continue;
            }
            let hit = if is_up_leave {

                r1.3 < lows.len() && lows[r1.3] > zs.zg
            } else {

                r1.3 < highs.len() && highs[r1.3] < zs.zd
            };
            if hit {
                
                if !out.iter().any(|m: &BigSegThirdMarker| m.bar_index == r1.3 && m.is_buy == is_up_leave) {
                    out.push(BigSegThirdMarker {
                        bar_index: r1.3,
                        price: if is_up_leave { lows[r1.3] } else { highs[r1.3] },
                        is_buy: is_up_leave,
                    });
                }
                break;
            }
            k += 1;
        }
    }

    

    
    
    


    if let Some(&(first_start, _, true)) = sup2s.first() {
        if first_start > 0 {
            let down_sups: Vec<(usize, f64)> = sups.iter().filter(|s| !s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            let up_sups: Vec<(usize, f64)> = sups.iter().filter(|s| s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            
            
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(sups[k].1, bigs, segs, strokes) else { continue; };
                
                let Some((_, c_price)) = up_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                
                for (rb, _) in down_sups.iter().filter(|(b, _)| *b > dbe) {
                    if *rb < lows.len() && lows[*rb] > c_price {
                        let dup = out.iter().any(|m| m.bar_index == *rb && m.is_buy);
                        if !dup {
                            out.push(BigSegThirdMarker {
                                bar_index: *rb,
                                price: lows[*rb],
                                is_buy: true,
                            });
                        }
                        break;
                    }
                }
            }
        }
    }
    
    
    

    if let Some(&(first_start, _, false)) = sup2s.first() {
        if first_start > 0 {
            let down_sups: Vec<(usize, f64)> = sups.iter().filter(|s| !s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            let up_sups: Vec<(usize, f64)> = sups.iter().filter(|s| s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            
            
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(sups[k].1, bigs, segs, strokes) else { continue; };
                
                let Some((_, c_price)) = down_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                
                for (rb, _) in up_sups.iter().filter(|(b, _)| *b > dbe) {
                    if *rb < highs.len() && highs[*rb] < c_price {
                        let dup = out.iter().any(|m| m.bar_index == *rb && !m.is_buy);
                        if !dup {
                            out.push(BigSegThirdMarker {
                                bar_index: *rb,
                                price: highs[*rb],
                                is_buy: false,
                            });
                        }
                        break;
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BigSegFirstMarker {
    pub bar_index: usize,
    pub price: f64,
    
    
    pub is_buy: bool,
}

pub fn detect_bigseg_first_buy_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<BigSegFirstMarker> {
    if bigs.len() < 2 {
        return vec![];
    }
    let infos = classify_sup_segments(strokes, segs, bigs, cases);
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

    let mut out: Vec<BigSegFirstMarker> = Vec::new();
    for p in 0..infos.len().saturating_sub(1) {
        let q = infos[p];
        let n = infos[p + 1];


        if !q.is_up || q.build != SupBuild::Case3 || !q.base_adjacent || q.extended {
            continue;
        }

        let c2 = crate::check_up_segment_case2(&projected, q.start_big);
        if c2.0 && c2.1 == q.end_big {
            continue;
        }

        if n.is_up || n.build != SupBuild::Case2 {
            continue;
        }
        
        
        let up_inside = (q.start_big..=q.end_big)
            .filter(|&k| k < bigs.len() && bigs[k].2 == q.is_up)
            .count();
        if up_inside != 1 {
            continue;
        }

        let Some((end_bar, end_price)) = crate::resolve_sup_end_point(n.end_big, bigs, segs, strokes)
        else {
            continue;
        };

        let up_start_low = projected[q.start_big].start_price;
        if end_price <= up_start_low {
            continue;
        }
        out.push(BigSegFirstMarker {
            bar_index: end_bar,
            price: end_price,
            is_buy: true,
        });
    }
    out.sort_by_key(|m| m.bar_index);
    out.dedup_by_key(|m| m.bar_index);
    out
}

pub fn detect_bigseg_first_sell_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<BigSegFirstMarker> {
    if bigs.len() < 2 {
        return vec![];
    }
    let infos = classify_sup_segments(strokes, segs, bigs, cases);
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

    let mut out: Vec<BigSegFirstMarker> = Vec::new();
    for p in 0..infos.len().saturating_sub(1) {
        let q = infos[p];
        let n = infos[p + 1];


        if q.is_up || q.build != SupBuild::Case3 || !q.base_adjacent || q.extended {
            continue;
        }

        let c2 = crate::check_down_segment_case2(&projected, q.start_big);
        if c2.0 && c2.1 == q.end_big {
            continue;
        }

        if !n.is_up || n.build != SupBuild::Case2 {
            continue;
        }
        
        let down_inside = (q.start_big..=q.end_big)
            .filter(|&k| k < bigs.len() && bigs[k].2 == q.is_up)
            .count();
        if down_inside != 1 {
            continue;
        }

        let Some((end_bar, end_price)) = crate::resolve_sup_end_point(n.end_big, bigs, segs, strokes)
        else {
            continue;
        };

        let down_start_high = projected[q.start_big].start_price;
        if end_price >= down_start_high {
            continue;
        }
        out.push(BigSegFirstMarker {
            bar_index: end_bar,
            price: end_price,
            is_buy: false,
        });
    }
    out.sort_by_key(|m| m.bar_index);
    out.dedup_by_key(|m| m.bar_index);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sup2TurnEvent {

    pub sup2_idx: usize,

    pub start_sup_idx: usize,

    pub end_sup_idx: usize,

    pub is_turn_long: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuperiorFirstMarker {
    pub bar_index: usize,
    pub price: f64,

    pub is_buy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sup2Build {

    Case3,

    Case2,

    Case4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sup2SegmentInfo {
    pub start_sup: usize,
    pub end_sup: usize,
    pub is_up: bool,
    pub build: Sup2Build,

    pub extended: bool,


    

    pub base_adjacent: bool,
}

pub fn classify_sup2_segments(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<Sup2SegmentInfo> {
    if sups.len() < 2 {
        return vec![];
    }
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
    let c2_cache: Vec<(bool, usize)> = (0..count)
        .map(|i| {
            if projected[i].is_up {
                crate::check_up_segment_case2(&projected, i)
            } else {
                crate::check_down_segment_case2(&projected, i)
            }
        })
        .collect();
    let mut out: Vec<Sup2SegmentInfo> = Vec::new();
    let mut c3_seg_starts = vec![false; count];
    let mut i = 0usize;
    let mut looking_for_up = true;
    let mut first = true;
    while i < count {
        let mut found = false;
        let mut best_end = 0usize;
        let mut build = Sup2Build::Case2;
        let mut ok_c3 = false;
        let mut base_c3 = usize::MAX;
        if (looking_for_up && projected[i].is_up)
            || (!looking_for_up && !projected[i].is_up)
            || (first && !projected[i].is_up)
        {
            let mut cands: Vec<usize> = Vec::new();
            if c2_cache[i].0 {
                cands.push(c2_cache[i].1);
            }
            
            let prev_rev = out.last().filter(|s| s.is_up != projected[i].is_up).map(|s| s.start_sup);
            let (ok3, _start_c3, base3) = if projected[i].is_up {
                crate::check_up_segment_case3(&projected, i, prev_rev)
            } else {
                crate::check_down_segment_case3(&projected, i, prev_rev)
            };
            ok_c3 = ok3;
            base_c3 = base3;
            if cases.case3 && ok_c3 {
                cands.push(i);
            }
            let (ok_c4, _) = if projected[i].is_up {
                crate::check_up_segment_case4(&projected, i)
            } else {
                crate::check_down_segment_case4(&projected, i)
            };
            if cases.case4 && ok_c4 {
                cands.push(i);
            }
            if !cands.is_empty() {
                cands.sort();
                best_end = cands[0];
                let is_c3 = cases.case3 && ok_c3 && best_end == i;
                build = if is_c3 {
                    Sup2Build::Case3
                } else if ok_c4 && best_end == i {
                    Sup2Build::Case4
                } else {
                    Sup2Build::Case2
                };
                found = true;
            }
        }
        if found {
            if build == Sup2Build::Case3 {
                c3_seg_starts[i] = true;
            }
            let first_seg_down = first && !projected[i].is_up;
            let base_adjacent = ok_c3
                && base_c3 != usize::MAX
                && c2_cache[base_c3].0
                && c2_cache[base_c3].1 + 1 == i;
            out.push(Sup2SegmentInfo {
                start_sup: i,
                end_sup: best_end,
                is_up: projected[i].is_up,
                build,
                extended: false,
                base_adjacent,
            });
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    for s in 0..out.len() {
        let end_si = out[s].end_sup;
        let is_up = out[s].is_up;
        let next_start = if s + 1 < out.len() {
            out[s + 1].start_sup
        } else {
            count
        };
        for si in end_si + 1..next_start {
            if si < count && projected[si].is_up == is_up {
                let cand = projected[si].end_price;
                let cur = projected[out[s].end_sup].end_price;
                if (is_up && cand >= cur) || (!is_up && cand <= cur) {
                    out[s].end_sup = si;
                    out[s].extended = true;
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BigSegBuild {

    Case3,

    Case2,

    Case4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigSegInfo {
    pub start_seg: usize,
    pub end_seg: usize,
    pub is_up: bool,
    pub build: BigSegBuild,

    pub extended: bool,


    pub base_adjacent: bool,
}

pub fn classify_big_segments(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<BigSegInfo> {
    if segs.len() < 2 {
        return vec![];
    }

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
    let count = projected.len();
    let c2_cache: Vec<(bool, usize)> = (0..count)
        .map(|i| {
            if projected[i].is_up {
                crate::check_up_segment_case2(&projected, i)
            } else {
                crate::check_down_segment_case2(&projected, i)
            }
        })
        .collect();
    let mut out: Vec<BigSegInfo> = Vec::new();
    let mut c3_seg_starts = vec![false; count];
    let mut i = 0usize;
    let mut looking_for_up = true;
    let mut first = true;
    while i < count {
        let mut found = false;
        let mut best_end = 0usize;
        let mut build = BigSegBuild::Case2;
        let mut ok_c3 = false;
        let mut base_c3 = usize::MAX;
        if (looking_for_up && projected[i].is_up)
            || (!looking_for_up && !projected[i].is_up)
            || (first && !projected[i].is_up)
        {
            let mut cands: Vec<usize> = Vec::new();
            if c2_cache[i].0 {
                cands.push(c2_cache[i].1);
            }
            
            let prev_rev = out.last().filter(|s| s.is_up != projected[i].is_up).map(|s| s.start_seg);
            let (ok3, _start_c3, base3) = if projected[i].is_up {
                crate::check_up_segment_case3(&projected, i, prev_rev)
            } else {
                crate::check_down_segment_case3(&projected, i, prev_rev)
            };
            ok_c3 = ok3;
            base_c3 = base3;
            if cases.case3 && ok_c3 {
                cands.push(i);
            }
            let (ok_c4, _) = if projected[i].is_up {
                crate::check_up_segment_case4(&projected, i)
            } else {
                crate::check_down_segment_case4(&projected, i)
            };
            if cases.case4 && ok_c4 {
                cands.push(i);
            }
            if !cands.is_empty() {
                cands.sort();
                best_end = cands[0];
                let is_c3 = cases.case3 && ok_c3 && best_end == i;
                build = if is_c3 {
                    BigSegBuild::Case3
                } else if ok_c4 && best_end == i {
                    BigSegBuild::Case4
                } else {
                    BigSegBuild::Case2
                };
                found = true;
            }
        }
        if found {
            if build == BigSegBuild::Case3 {
                c3_seg_starts[i] = true;
            }
            let first_seg_down = first && !projected[i].is_up;
            let base_adjacent = ok_c3
                && base_c3 != usize::MAX
                && c2_cache[base_c3].0
                && c2_cache[base_c3].1 + 1 == i;
            out.push(BigSegInfo {
                start_seg: i,
                end_seg: best_end,
                is_up: projected[i].is_up,
                build,
                extended: false,
                base_adjacent,
            });
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    for s in 0..out.len() {
        let end_si = out[s].end_seg;
        let is_up = out[s].is_up;
        let next_start = if s + 1 < out.len() {
            out[s + 1].start_seg
        } else {
            count
        };
        for si in end_si + 1..next_start {
            if si < count && projected[si].is_up == is_up {
                let cand = projected[si].end_price;
                let cur = projected[out[s].end_seg].end_price;
                if (is_up && cand >= cur) || (!is_up && cand <= cur) {
                    out[s].end_seg = si;
                    out[s].extended = true;
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupBuild {

    Case3,

    Case2,

    Case4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupSegmentInfo {
    pub start_big: usize,
    pub end_big: usize,
    pub is_up: bool,
    pub build: SupBuild,

    pub extended: bool,



    pub base_adjacent: bool,
}

pub fn classify_sup_segments(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<SupSegmentInfo> {
    if bigs.len() < 2 {
        return vec![];
    }
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
    let c2_cache: Vec<(bool, usize)> = (0..count)
        .map(|i| {
            if projected[i].is_up {
                crate::check_up_segment_case2(&projected, i)
            } else {
                crate::check_down_segment_case2(&projected, i)
            }
        })
        .collect();
    let mut out: Vec<SupSegmentInfo> = Vec::new();
    let mut c3_seg_starts = vec![false; count];
    let mut i = 0usize;
    let mut looking_for_up = true;
    let mut first = true;
    while i < count {
        let mut found = false;
        let mut best_end = 0usize;
        let mut build = SupBuild::Case2;
        let mut ok_c3 = false;
        let mut base_c3 = usize::MAX;
        if (looking_for_up && projected[i].is_up)
            || (!looking_for_up && !projected[i].is_up)
            || (first && !projected[i].is_up)
        {
            let mut cands: Vec<usize> = Vec::new();
            if c2_cache[i].0 {
                cands.push(c2_cache[i].1);
            }
            
            let prev_rev = out.last().filter(|s| s.is_up != projected[i].is_up).map(|s| s.start_big);
            let (ok3, _start_c3, base3) = if projected[i].is_up {
                crate::check_up_segment_case3(&projected, i, prev_rev)
            } else {
                crate::check_down_segment_case3(&projected, i, prev_rev)
            };
            ok_c3 = ok3;
            base_c3 = base3;
            if cases.case3 && ok_c3 {
                cands.push(i);
            }
            let (ok_c4, _) = if projected[i].is_up {
                crate::check_up_segment_case4(&projected, i)
            } else {
                crate::check_down_segment_case4(&projected, i)
            };
            if cases.case4 && ok_c4 {
                cands.push(i);
            }
            if !cands.is_empty() {
                cands.sort();
                best_end = cands[0];
                let is_c3 = cases.case3 && ok_c3 && best_end == i;
                build = if is_c3 {
                    SupBuild::Case3
                } else if ok_c4 && best_end == i {
                    SupBuild::Case4
                } else {
                    SupBuild::Case2
                };
                found = true;
            }
        }
        if found {
            if build == SupBuild::Case3 {
                c3_seg_starts[i] = true;
            }
            let first_seg_down = first && !projected[i].is_up;
            let base_adjacent = ok_c3
                && base_c3 != usize::MAX
                && c2_cache[base_c3].0
                && c2_cache[base_c3].1 + 1 == i;
            out.push(SupSegmentInfo {
                start_big: i,
                end_big: best_end,
                is_up: projected[i].is_up,
                build,
                extended: false,
                base_adjacent,
            });
            i = best_end + 1;
            if !first_seg_down {
                looking_for_up = !looking_for_up;
            }
            first = false;
        } else {
            i += 1;
        }
    }

    for s in 0..out.len() {
        let end_si = out[s].end_big;
        let is_up = out[s].is_up;
        let next_start = if s + 1 < out.len() {
            out[s + 1].start_big
        } else {
            count
        };
        for si in end_si + 1..next_start {
            if si < count && projected[si].is_up == is_up {
                let cand = projected[si].end_price;
                let cur = projected[out[s].end_big].end_price;
                if (is_up && cand >= cur) || (!is_up && cand <= cur) {
                    out[s].end_big = si;
                    out[s].extended = true;
                }
            }
        }
    }
    out
}

pub fn detect_superior_first_buy_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<SuperiorFirstMarker> {
    if sups.len() < 2 {
        return vec![];
    }
    let infos = classify_sup2_segments(strokes, segs, bigs, sups, cases);
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

    let mut out: Vec<SuperiorFirstMarker> = Vec::new();
    for p in 0..infos.len().saturating_sub(1) {
        let q = infos[p];
        let n = infos[p + 1];



        
        if !q.is_up || q.build != Sup2Build::Case3 || !q.base_adjacent || q.extended {
            continue;
        }

        let c2 = crate::check_up_segment_case2(&projected, q.start_sup);
        if c2.0 && c2.1 == q.end_sup {
            continue;
        }

        if n.is_up || n.build != Sup2Build::Case2 {
            continue;
        }
        
        
        
        let up_inside = (q.start_sup..=q.end_sup)
            .filter(|&k| k < sups.len() && sups[k].2 == q.is_up)
            .count();
        if up_inside != 1 {
            continue;
        }

        let Some((_, _, _, end_bar)) = resolve_sup_range(sups[n.end_sup], bigs, segs, strokes)
        else {
            continue;
        };
        let end_price = strokes[segs[bigs[sups[n.end_sup].1].1].1].end_price;

        
        let up_start_low = projected[q.start_sup].start_price;
        if end_price <= up_start_low {
            continue;
        }
        out.push(SuperiorFirstMarker {
            bar_index: end_bar,
            price: end_price,
            is_buy: true,
        });
    }
    out.sort_by_key(|m| m.bar_index);
    out.dedup_by_key(|m| m.bar_index);
    out
}

pub fn detect_superior_first_sell_markers(
    strokes: &[Stroke],
    segs: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    sups: &[(usize, usize, bool)],
    cases: crate::StrokeCases,
) -> Vec<SuperiorFirstMarker> {
    if sups.len() < 2 {
        return vec![];
    }
    let infos = classify_sup2_segments(strokes, segs, bigs, sups, cases);
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

    let mut out: Vec<SuperiorFirstMarker> = Vec::new();
    for p in 0..infos.len().saturating_sub(1) {
        let q = infos[p];
        let n = infos[p + 1];


        if q.is_up || q.build != Sup2Build::Case3 || !q.base_adjacent || q.extended {
            continue;
        }

        let c2 = crate::check_down_segment_case2(&projected, q.start_sup);
        if c2.0 && c2.1 == q.end_sup {
            continue;
        }

        if !n.is_up || n.build != Sup2Build::Case2 {
            continue;
        }
        
        let down_inside = (q.start_sup..=q.end_sup)
            .filter(|&k| k < sups.len() && sups[k].2 == q.is_up)
            .count();
        if down_inside != 1 {
            continue;
        }

        let Some((_, _, _, end_bar)) = resolve_sup_range(sups[n.end_sup], bigs, segs, strokes)
        else {
            continue;
        };
        let end_price = strokes[segs[bigs[sups[n.end_sup].1].1].1].end_price;

        let down_start_high = projected[q.start_sup].start_price;
        if end_price >= down_start_high {
            continue;
        }
        out.push(SuperiorFirstMarker {
            bar_index: end_bar,
            price: end_price,
            is_buy: false,
        });
    }
    out.sort_by_key(|m| m.bar_index);
    out.dedup_by_key(|m| m.bar_index);
    out
}

pub struct SuperiorRetreatResult {
    pub superior_second_markers: Vec<crate::SecondMarker>,
    pub superior_third_markers: Vec<BigSegThirdMarker>,
    pub superior_plus_markers: Vec<crate::guidao::SecondPlusOneMarker>,
}

fn sup2_range(
    sup2: (usize, usize, bool),
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Option<(usize, usize)> {
    let (ss, se, _) = sup2;
    let start_bar = resolve_sup_range(sups[ss], bigs, segs, strokes)?.2;
    let end_bar = resolve_sup_range(sups[se], bigs, segs, strokes)?.3;
    Some((start_bar, end_bar))
}

pub fn apply_sup2_retreat_filter(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    mut superior_second_markers: Vec<crate::SecondMarker>,
    mut superior_third_markers: Vec<BigSegThirdMarker>,
    mut superior_plus_markers: Vec<crate::guidao::SecondPlusOneMarker>,
) -> SuperiorRetreatResult {
    let sup2s = crate::process_superior_superior_segments(strokes, segs, bigs, sups);

    for d_idx in (1..sup2s.len()).filter(|&i| !sup2s[i].2) {
        if let Some((sb, eb)) = sup2_range(sup2s[d_idx - 1], sups, bigs, segs, strokes) {
            let mut buy_bars: Vec<usize> = Vec::new();
            for m in superior_second_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in superior_third_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in superior_plus_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            let cut = sup2_range(sup2s[d_idx], sups, bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match buy_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => {
                    superior_second_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    superior_third_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    superior_plus_markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut);
                }
                None => {

                    superior_second_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    superior_third_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    superior_plus_markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                }
            }
        }
    }
    
    for u_idx in (1..sup2s.len()).filter(|&i| sup2s[i].2) {
        if let Some((sb, eb)) = sup2_range(sup2s[u_idx - 1], sups, bigs, segs, strokes) {
            let mut sell_bars: Vec<usize> = Vec::new();
            for m in superior_second_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in superior_third_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in superior_plus_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            let cut = sup2_range(sup2s[u_idx], sups, bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match sell_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => {
                    superior_second_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    superior_third_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                    superior_plus_markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut);
                }
                None => {
                    
                    superior_second_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    superior_third_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                    superior_plus_markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut);
                }
            }
        }
    }
    SuperiorRetreatResult { superior_second_markers, superior_third_markers, superior_plus_markers }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrendZhongShuLabel {

    pub index: usize,
    pub label: String,
}

pub fn detect_bigseg_trend_zhongshus(
    zhongshus: &[BigSegmentZhongShu],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<TrendZhongShuLabel> {
    if zhongshus.is_empty() {
        return vec![];
    }

    let info: Vec<(f64, f64, bool)> = zhongshus
        .iter()
        .map(|z| {
            let n = big_segments
                .iter()
                .filter(|&b| {
                    resolve_bigseg_range(*b, segments, strokes)
                        .map(|r| r.start_bar <= z.end_bar && r.end_bar >= z.start_bar)
                        .unwrap_or(false)
                })
                .count();
            (z.dd, z.gg, n == 1)
        })
        .collect();

    let mut out: Vec<TrendZhongShuLabel> = Vec::new();
    let mut seq = 0usize; 
    let mut last_trend: Option<usize> = None; 
    for i in 0..info.len() {
        
        if i > 0 && zhongshus[i].sup_idx != zhongshus[i - 1].sup_idx {
            seq = 0;
            last_trend = None;
        }

        let is_extension = match last_trend {
            Some(t) => !(info[i].0 > info[t].1 || info[i].1 < info[t].0),
            None => false,
        };
        let core = if is_extension {
            "扩".to_string()
        } else {
            let s = col_name(seq);
            seq += 1;
            last_trend = Some(i);
            s
        };
        out.push(TrendZhongShuLabel {
            index: i,
            label: format!("{}{}", if info[i].2 { "L" } else { "" }, core),
        });
    }
    out.sort_by_key(|l| l.index);
    out
}

fn col_name(mut n: usize) -> String {
    let mut s: Vec<char> = Vec::new();
    loop {
        s.push((b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s.iter().rev().collect()
}

pub fn detect_bigseg_zhongshus_by_overlap(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BigSegmentZhongShu> {
    let mut out: Vec<BigSegmentZhongShu> = Vec::new();
    for (sup_idx, &(big_start, big_end, sup_up)) in superior_segments.iter().enumerate() {
        if big_end >= big_segments.len() {
            continue;
        }
        
        let mut revs: Vec<BigSegRange> = Vec::new();
        for j in big_start..=big_end {
            if big_segments[j].2 == sup_up {
                continue;
            }
            if let Some(r) = resolve_bigseg_range(big_segments[j], segments, strokes) {
                revs.push(r);
            }
        }
        let mk = |r: &BigSegRange| BigSegmentZhongShu {
            sup_idx,
            start_bar: r.start_bar,
            end_bar: r.end_bar,
            zg: r.high,
            zd: r.low,
            gg: r.high,
            dd: r.low,
        };
        let mut i = 0usize;
        while i < revs.len() {
            let r1 = revs[i];
            if i + 1 >= revs.len() {
                out.push(mk(&r1)); 
                break;
            }
            let r2 = revs[i + 1];
            let zd = r1.low.max(r2.low);
            let zg = r1.high.min(r2.high);
            if zd >= zg {

                out.push(mk(&r1));
                i += 1;
                continue;
            }

            let start_bar = r1.start_bar;
            let mut end_bar = r2.end_bar;
            let mut gg = r1.high.max(r2.high);
            let mut dd = r1.low.min(r2.low);
            let mut k = i + 2;
            while k < revs.len() {
                let rk = revs[k];
                if rk.low <= zg && rk.high >= zd {
                    end_bar = rk.end_bar; 
                    gg = gg.max(rk.high);
                    dd = dd.min(rk.low);
                    k += 1;
                } else {
                    break; 
                }
            }
            out.push(BigSegmentZhongShu { sup_idx, start_bar, end_bar, zg, zd, gg, dd });
            i = k;
        }
    }
    out
}

