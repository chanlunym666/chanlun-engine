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

/// 检测大段中枢后的三买/三卖 (规则详见模块注释第 8 条)。
/// 每中枢最多输出 1 个标记 (第一个满足的相邻对)。
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
        // 中枢后第一段: 第一个起点 bar >= 中枢右边界的大段
        // (大段首尾相接: 参与段自身起点必 < 右边界, 其后第一段起点 == 右边界)
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
        // ［规则·三买三卖宿主与范围］「三买只能且唯一存在于**自身级别的（向上）中枢**之上」
        //   ⇒ 每条中枢只拥有**紧随其后的那一次离开**：扫描上界 = **下一条同级别中枢的成型点**（`start_bar`）；
        //      其后无新中枢 ⇒ 不设上界（同向段未成立窗口照旧放行）。
        //   两个极端方案都不可行：
        //     ① 截断到宿主段末端：把宿主段走完之后的三买/三卖整段切掉（过严）；
        //     ② 不设上界：早已结束的中枢一路扫到十几个段之后，在其他中枢领地上越界命中
        //        
        //        落点却是 09-11 的**向上**高级段终点，与同 bar 的高二卖（宿主 = 该向上段）**同 bar 同价 65.28 完全重叠**。
        //
        // ［规则·每条中枢都可命中］「**同一个向下高级段内的所有大段中枢，
        //  不再限制「大三卖」的数量**」⇒ 界只按**跨高级段**取（下一条**属于别的宿主段**的中枢），
        //  **同一宿主段内**的中枢之间**不互相设界**，各自都能拥有「紧随其后的那一次离开」。
        //  ⚠️ 对「每高级段只第一个中枢」的调用方（首段集合）此改动**恒为 no-op**
        //     （该集合内所有中枢的 `sup_idx` 互不相同）⇒ 既有行为逐条不变，回滚只需换回集合。
        let next_zs_start = zhongshus
            .iter()
            .filter(|z| z.start_bar > zs.end_bar && z.sup_idx != zs.sup_idx)
            .map(|z| z.start_bar)
            .min()
            .unwrap_or(usize::MAX);
        // ［规则·反向段成立点截断守卫］
        //    
        //    宿主中枢之后**第一个与宿主段反向的高级段成立点** = 硬上界：
        //    买链 (离开=UP) 界 = 其后第一个**向下**高级段终点 bar；卖链镜像 = 第一个**向上**高级段终点。
        //    过渡窗口 (中枢终点 → 反向高级段成立点) 内照常判定 ( 过渡期放行不变);
        //    反向高级段成立 ⇒ 旧中枢领地终结, 其后归新结构（新结构自己有没有中枢由自己的中枢决定）。
        //    案例: 欧美1m 10/16 08:18 大三买 — bz#60 的 (离开,回试) 对落在 sup#57 之后,
        //    而 sup#56 DOWN 10/14 23:08 成立即应截断 (宿主 sup#55 / 中枢 bz#60, sup#57 内无中枢)。
        //    ⚠️ 本守卫 ≠ 「宿主段终点截断」(后者  已否决, 勿混淆)。
        //    镜像自 3+N 递推同款守卫。
        let zhongshu_sup2_dir = sup_up;
        let seq_end_bar = superior_segments
            .iter()
            .enumerate()
            .filter(|(i, s)| *i > zs.sup_idx && s.2 != zhongshu_sup2_dir) // 反向高级段
            .filter_map(|(i, _)| {
                let (bs, be, _) = superior_segments[i];
                if bs > be || be >= big_segments.len() { return None; }
                let b2 = strokes.get(segments.get(big_segments[be].1)?.1)?;
                Some(b2.end_bar)
            })
            .find(|&end_bar| end_bar > zs.end_bar);
        // 逐对 (k, k+1): 三买对 = (向上, 向下), 三卖对 = (向下, 向上)
        // 回试段边界: 大段中枢三买只考虑中枢之后
        // 的大段终点价 vs ZG (大段级别, 不看内部线段) — 离开段是高级段最后大段时,
        // 若其后还有大段 (可能不属于任何高级段), 仍判定这一对.
        // ［规则·不截断到宿主段末端］: **不再截断到宿主大段末端** ——
        //    中枢之后（含同向段未成立窗口）的所有 (离开段, 回试段) 对都参与判定，命中即停（同一中枢最多一个）。
        while k + 1 < big_segments.len() {
            let Some(r1) = resolve_bigseg_range(big_segments[k + 1], segments, strokes) else {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} SKIP: 回试段解析失败", zs.sup_idx, k);
                break;
            };
            // 🔴  定版界：越过下一条同级别中枢的成型点 ⇒ 本次离开不属于本中枢（详见上方注释）
            if r1.end_bar >= next_zs_start {
                log::debug!("[3rd] 回试终点bar={} ≥ 下一条中枢成型点 {} ⇒ 越界, 停", r1.end_bar, next_zs_start);
                break;
            }
            // 🔴  过渡期截断守卫 (基准大三买/卖, 与 3+N 递推守卫同规): 回试终点越过
            //    「反向高级段成立点」⇒ 序列终结 (见上方注释; 过渡窗口内照常判定)
            if let Some(seq_end) = seq_end_bar {
                if r1.end_bar >= seq_end {
                    log::debug!("[3rd] 回试终点bar={} ≥ 反向高级段成立点 {} ⇒ 过渡期截断, 停", r1.end_bar, seq_end);
                    break;
                }
            }
            let is_up_leave = big_segments[k].2;
            // ［规则·方向过滤］:
            //    「三买前必须要有**自身级别的向上中枢**；三卖镜像」⇒
            //    **宿主高级段方向必须 == 离开段方向**（离开段方向 == 标记方向），
            //    宿主方向相反的中枢一律不产三买/三卖（跳过该对，继续向后扫描，不 break）。
            //    过渡期放行（: 不截断到宿主段末端）**保持不变**，本条件只加方向过滤。
            if is_up_leave != sup_up {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} 离开dir={} 宿主高级段dir={} ⇒ 方向不符, 跳过",
                    zs.sup_idx, k, if is_up_leave { "up" } else { "down" }, if sup_up { "up" } else { "down" });
                k += 1;
                continue;
            }
            let hit = if is_up_leave {
                // 向上大段离开 → 三买: 回试向下段终点bar最低价 > ZG
                r1.end_bar < lows.len() && lows[r1.end_bar] > zs.zg
            } else {
                // 向下大段离开 → 三卖: 回试向上段终点bar最高价 < ZD
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
                // 去重 去截断后多个中枢可能命中同一回试段 ⇒ 同一 (bar, 方向) 只登记一次
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
    // ── 初始数据特定情况 (补充检测点, 买向) ──
    // 与高3买 补充同条件: 初始数据没有向下高级段、却有向上高级段
    // (sups 首段为 UP 且其起点 > 0, 开头存在游离向下大段)
    // → 游离向下大段作为补充检测点: C = 检测点后首个向上大段终点 (前高点);
    //   检测点后第一个"回试低点 > C"的向下大段 (回试低点没与前高点重叠, K线最低价判定)
    //   → 大三买 @其终点 (与现有中枢路径判定语义一致: 回试不破前高)
    // 去重: 与中枢路径输出 (bar_index, is_buy) 相同则跳过
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
            // ［规则·两向补充检测点］: **买点与卖点都要处理** —— 开头两类游离大段
            //    (向下/向上) 都作补充检测点; 两个方向都要处理。
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(k, big_segments, segments, strokes) else { continue; };
                // C = 检测点后首个向上大段终点 (前高点)
                let Some((_, c_price)) = up_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                // 回试段: 检测点后第一个"回试低点 > C"的向下大段 (每检测点最多 1 个标记)
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
    // ── 初始数据特定情况 (补充检测点, 卖向) ──
    // sups 首段为 DOWN 且起点 > 0 (开头存在游离向上大段)
    // → 游离向上大段作为补充检测点: C = 检测点后首个向下大段终点 (前低点);
    //   检测点后第一个"回试高点 < C"的向上大段 (回试高点没与前低点重叠, K线最高价判定)
    //   → 大三卖 @其终点
    // 去重: 与中枢路径输出 (bar_index, is_buy) 相同则跳过
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
            // ［规则·两向补充检测点］: **买点与卖点都要处理** —— 开头两类游离大段
            //    (向上/向下) 都作补充检测点; 两个方向都要处理。
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(k, big_segments, segments, strokes) else { continue; };
                // C = 检测点后首个向下大段终点 (前低点)
                let Some((_, c_price)) = down_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                // 回试段: 检测点后第一个"回试高点 < C"的向上大段 (每检测点最多 1 个标记)
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

/// 检测高级段中枢后的高3买/高3卖 (规则与 detect_bigseg_third_marks 完全同构)。
/// 每中枢最多输出 1 个标记 (第一个满足的相邻高级段对)。
/// 高3买对 = (向上高级段离开, 向下高级段回试): 回试段终点bar最低价 > ZG;
/// 高3卖对 = (向下高级段离开, 向上高级段回试): 回试段终点bar最高价 < ZD。
/// 离开段是宿主最后高级段时, 其后游离高级段 (不属于任何高高级段) 仍判定这一对。
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
        // 中枢后第一段: 第一个起点 bar >= 中枢右边界的高级段
        let mut k = sup_start;
        while k <= sup_end {
            if let Some(br) = resolve_sup_range(sups[k], bigs, segs, strokes) {
                if br.2 >= zs.end_bar {
                    break;
                }
            }
            k += 1;
        }
        // ［规则］高三买/高三卖只属于**紧接其后的那一次离开** ⇒
        //   扫描上界 = 下一条高级段中枢的成型点（无 ⇒ 不设上界）。理由与害处见 `detect_bigseg_third_marks` 同处注释。
        //   规则：「**高级三买就是只能存在高级段中枢（向上的）之上**；
        //      用向上宿主段，就会发生被向上宿主段终点切割而导致三买不能及时标记的情况」
        //      ⇒ 判据挂在**中枢**上（方向 + 只拥有紧随其后的那次离开），**不得**退回"宿主段终点"截断。
        //   ［规则］「高级段的大段中枢的算法…早都从只许首个中枢变成多个中枢了」
        //      ⇒ 界只按**跨高高级段**取（下一条**属于别的宿主段**的中枢）；**同一宿主段内**的中枢互不设界。
        //      与大段级 [`detect_bigseg_third_marks`] 的 `&& z.sup_idx != zs.sup_idx` 逐条一致
        //      （此前漏了该条件会导致窗口过松而越界误标）。
        let next_zs_start = zhongshus
            .iter()
            .filter(|z| z.start_bar > zs.end_bar && z.sup_idx != zs.sup_idx)
            .map(|z| z.start_bar)
            .min()
            .unwrap_or(usize::MAX);
        // ［规则］
        //    宿主中枢之后**第一个与宿主高高级段反向的高高级段成立点** = 硬上界;
        //    买链界 = 第一个**向下**高高级段终点 bar, 卖链镜像 = 第一个**向上**高高级段终点。
        //    过渡期窗口内照常判定 ; 反向高高级段成立 ⇒ 序列终结。
        //    ⚠️ 层级: 离开/回试对 = 高级段 ⇒ 界 = **高一级 (高高级段 sup2s)** —— 与大段级
        //    (对=大段, 界=高级段) 逐条同构; 与 高2+N 家族守卫「下一个反向高高级段终点」同规。
        //    不得降级扫高级段: 高3卖的回试段本身是向上高级段, 同级扫描会让回试段自己的成立点
        //    恰好等于信号点 ⇒ 误杀 )。
        let seq_end_bar = sup2s
            .iter()
            .enumerate()
            .filter(|(i, s)| *i > zs.sup_idx && s.2 != sup2_up) // 反向高高级段
            .filter_map(|(i, _)| {
                let (ss, se, _) = sup2s[i];
                if ss > se || se >= sups.len() { return None; }
                crate::resolve_sup_end_point(sups[se].1, bigs, segs, strokes).map(|(b, _)| b)
            })
            .find(|&b| b > zs.end_bar);
        // 逐对 (k, k+1): 高3买对 = (向上, 向下), 高3卖对 = (向下, 向上)
        // ⚠️ （过渡期阶段规则）: **不再截断到所属高级段末端** ——
        //    中枢之后（含过渡期阶段）的所有 (离开段, 回试段) 对**都参与判定**，命中即停
        //    。旧实现 `pair_end = sup_end`（仅宿主范围内）
        //    + 离开段方向不匹配即退，会把过渡期阶段的买卖点整段漏掉 —— 实例: XTIUSD M1
        //    09-29 13:35（sup[47] 向上段终点，highs=97.43 < 中枢 ZD=99.39）应有高级三卖而无标记。
        while k + 1 < sups.len() {
            let Some(r1) = resolve_sup_range(sups[k + 1], bigs, segs, strokes) else {
                break;
            };
            // 🔴 越过下一条同级别中枢的成型点 ⇒ 本次离开不属于本中枢
            //   （`r1.3` = 回试高级段终点 bar，与下方命中判据同源）
            if r1.3 >= next_zs_start {
                log::debug!("[3rd] 回试终点bar={} ≥ 下一条中枢成型点 {} ⇒ 越界, 停", r1.3, next_zs_start);
                break;
            }
            // 🔴  过渡期截断守卫 (高级三买/卖, 与大段级基准+两级 3+N 同规): 回试终点越过
            //    「反向高级段成立点」⇒ 序列终结 (见上方注释; 过渡期窗口内照常判定)
            if let Some(seq_end) = seq_end_bar {
                if r1.3 >= seq_end {
                    log::debug!("[3rd] 回试终点bar={} ≥ 反向高级段成立点 {} ⇒ 过渡期截断, 停", r1.3, seq_end);
                    break;
                }
            }
            let is_up_leave = sups[k].2;
            // ⚠️ （四级同步）: 高三买必须存在于**向上高高级段**（宿主）中、
            //    高三卖必须存在于**向下高高级段**中 ⇒ 宿主高高级段方向必须 == 离开高级段方向；
            //    方向不符则跳过该对（继续扫描）。过渡期放行（）保持不变。
            if is_up_leave != sup2_up {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} 离开dir={} 宿主高高级段dir={} ⇒ 方向不符, 跳过",
                    zs.sup_idx, k, if is_up_leave { "up" } else { "down" }, if sup2_up { "up" } else { "down" });
                k += 1;
                continue;
            }
            let hit = if is_up_leave {
                // 向上高级段离开 → 高3买: 回试向下段终点bar最低价 > ZG
                r1.3 < lows.len() && lows[r1.3] > zs.zg
            } else {
                // 向下高级段离开 → 高3卖: 回试向上段终点bar最高价 < ZD
                r1.3 < highs.len() && highs[r1.3] < zs.zd
            };
            if hit {
                // 去重 : 去截断后多个中枢可能命中同一回试段 ⇒ 同一 (bar, 方向) 只登记一次
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

    // ── 新增: 初始数据特定情况 () ──
    // 与高2+N 补充同条件: 初始数据没有向下高高级段、却有向上高高级段
    // (sup2s 首段为 UP 且其起点 > 0, 开头存在游离向下高级段如美日 sup[0])
    // → 游离向下高级段作为补充检测点: C = 检测点后首个向上高级段终点 (前高点);
    //   检测点后第一个"回试低点 > C"的向下高级段 (回试低点没与前高点重叠, K线最低价判定)
    //   → 高三买 @其终点 (与现有中枢路径判定语义一致: 回试不破前高)
    // 去重: 与 sup2s 内中枢路径输出 (bar_index, is_buy) 相同则跳过
    if let Some(&(first_start, _, true)) = sup2s.first() {
        if first_start > 0 {
            let down_sups: Vec<(usize, f64)> = sups.iter().filter(|s| !s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            let up_sups: Vec<(usize, f64)> = sups.iter().filter(|s| s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            // ⚠️ )
            //    都作补充检测点; 原 `if !sups[k].2 { continue; }` 方向守卫只取一类, 已按定版解除。
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(sups[k].1, bigs, segs, strokes) else { continue; };
                // C = 检测点后首个向上高级段终点 (前高点)
                let Some((_, c_price)) = up_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                // 回试段: 检测点后第一个"回试低点 > C"的向下高级段 (每检测点最多 1 个标记)
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
    // ── 初始数据特定情况 卖镜像: sup2s 首段为 DOWN 且起点 > 0 (开头存在游离向上高级段)
    // → 游离向上高级段作为补充检测点: C = 检测点后首个向下高级段终点 (前低点);
    //   检测点后第一个"回试高点 < C"的向上高级段 (回试高点没与前低点重叠, K线最高价判定)
    //   → 高三卖 @其终点
    if let Some(&(first_start, _, false)) = sup2s.first() {
        if first_start > 0 {
            let down_sups: Vec<(usize, f64)> = sups.iter().filter(|s| !s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            let up_sups: Vec<(usize, f64)> = sups.iter().filter(|s| s.2)
                .filter_map(|&(_, eb, _)| crate::resolve_sup_end_point(eb, bigs, segs, strokes)).collect();
            // ⚠️ )
            //    都作补充检测点; 原 `if sups[k].2 { continue; }` 方向守卫只取一类, 已按定版解除。
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(sups[k].1, bigs, segs, strokes) else { continue; };
                // C = 检测点后首个向下高级段终点 (前低点)
                let Some((_, c_price)) = down_sups.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                // 回试段: 检测点后第一个"回试高点 < C"的向上高级段 (每检测点最多 1 个标记)
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


// ═══ 以下为生产入口 + 3+N 递推家族 (与最新版算法库同源同步) ═══

/// 高级段三买/三卖中枢集口径开关 (单点可控): true = 每高高级段全部中枢, false = 只第一个中枢
pub const SUPERIOR_THIRD_ALL_ZHONGSHU: bool = true;

/// 大段三买/三卖中枢集口径开关 (单点可控)
pub const BIGSEG_THIRD_ALL_ZHONGSHU: bool = true;

/// 3+N 递推标记 (大段级与高级段级共用)
/// `order`: 0 = 基准大三买/大三卖 (等同 third marks), 1 = 3+1, 2 = 3+2 ...
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BigSeg3NMarker {
    /// 回试段终点 bar
    pub bar_index: usize,
    /// 标记价格 (3+N买 = 终点bar最低价, 3+N卖 = 终点bar最高价)
    pub price: f64,
    /// true = 3+N买 (回试不破 ZG), false = 3+N卖 (回试不破 ZD)
    pub is_buy: bool,
    /// 递推序号: 0 = 基准, 1 = 3+1, 2 = 3+2 ...
    pub order: u32,
}

/// 检测每个高高级段内的**全部**高级段中枢（与 [`detect_all_bigseg_zhongshus`]
/// **逐条同构**，层级提升一级：宿主 高高级段 / 部件 高级段 / 位置索引 +1/+3 / 跳跃 +3）。
///
/// 🔴 「高级段的大段中枢的算法…**早都从只许首个中枢变成多个中枢了**」+
///    「**按照大段3+N 的逻辑来**」。本函数 = 大段级 `detect_all_bigseg_zhongshus` 的**逐行镜像**
///    （⚠️ 首版我误写成「遍历全部反向段相邻配对」，导致 `sup2#4` 内每条反向高级段都成了中枢，
///      回试段自身 bar 也被算进中枢导致误标。已按大段级索引口径重写）。
pub fn detect_superior_zhongshus_by_overlap(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<SuperiorZhongShu> {
    let sup2s = crate::process_superior_superior_segments(strokes, segs, bigs, sups);
    let mut out: Vec<SuperiorZhongShu> = Vec::new();
    for (sup2_idx, &(sup_start, sup_end, sup2_up)) in sup2s.iter().enumerate() {
        // 门槛: 内部高级段数 < 3 → 无反向段结构 → 无中枢
        if sup_end < sup_start + 2 || sup_end >= sups.len() {
            continue;
        }
        let mut s = sup_start; // 本段内下一个中枢的"位置1"
        while s + 2 <= sup_end {
            let Some(b1) = resolve_sup_range(sups[s + 1], bigs, segs, strokes) else {
                break;
            };
            // 单反向段中枢: 区间 = 位置2 段高低点 (3 高级段, 或判定对不重叠时退化)
            let single = |end_bar: usize| SuperiorZhongShu {
                sup_idx: sup2_idx,
                start_bar: b1.2,
                end_bar,
                zg: b1.1,
                zd: b1.0,
                gg: b1.1,
                dd: b1.0,
            };
            if s + 2 == sup_end {
                out.push(single(b1.3));
                break; // 本段已到末尾
            }
            let Some(b2) = resolve_sup_range(sups[s + 3], bigs, segs, strokes) else {
                break;
            };
            let zd = b1.0.max(b2.0);
            let zg = b1.1.min(b2.1);
            if zd >= zg {
                // 判定对不重叠 → 退化为位置2 反向段自身 (类中枢)
                out.push(single(b1.3));
                s += 3;
                continue;
            }
            // 中枢成立: 参与段 = 位置2..位置4 (含位置3) + 端点延伸
            let mut end_bar = b2.3;
            let mut gg = b1.1.max(b2.1);
            let mut dd = b1.0.min(b2.0);
            if let Some(bm) = resolve_sup_range(sups[s + 2], bigs, segs, strokes) {
                gg = gg.max(bm.1);
                dd = dd.min(bm.0);
            }
            // 延伸: 位置5起, 与 [zd, zg] 重叠 (low <= zg && high >= zd, 恰好相等也延伸),
            // 首次不重叠 (离开区间) 即停止, 离开后即使跌回也不恢复。
            // 终点只推进到"与判定对同向"的段 (sups[j].2 != sup2_up)
            let mut j = s + 4;
            while j <= sup_end {
                let Some(br) = resolve_sup_range(sups[j], bigs, segs, strokes) else {
                    break;
                };
                if br.0 <= zg && br.1 >= zd {
                    if sups[j].2 != sup2_up {
                        end_bar = br.3;
                    }
                    gg = gg.max(br.1);
                    dd = dd.min(br.0);
                    j += 1;
                } else {
                    break;
                }
            }
            out.push(SuperiorZhongShu { sup_idx: sup2_idx, start_bar: b1.2, end_bar, zg, zd, gg, dd });
            // 下一个中枢从本中枢右侧继续寻找 (同一高高级段内可有多个)
            s = j.max(s + 3);
        }
    }
    out
}

/// 高级段中枢集口径（单点可控）——供高三买/高3卖 与其 3+N 递推**共用同一集合**。
pub fn superior_zhongshu_set(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<SuperiorZhongShu> {
    if SUPERIOR_THIRD_ALL_ZHONGSHU {
        // 主图那套：每高高级段内**全部**中枢（与大段级 by_overlap 同构）
        detect_superior_zhongshus_by_overlap(sups, bigs, segs, strokes)
    } else {
        // 回滚口径：每高高级段**只第一个**中枢
        detect_superior_zhongshus(strokes, segs, bigs, sups)
    }
}

/// 高三买/高三卖 —— **生产入口**（按 [`SUPERIOR_THIRD_ALL_ZHONGSHU`] 决定中枢集口径），
/// 与 [`bigseg_third_marks_all_zhongshu`]（大段级）**逐条同构**。
pub fn superior_third_marks_all_zhongshu(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSegThirdMarker> {
    let zhongshus = superior_zhongshu_set(sups, bigs, segs, strokes);
    detect_superior_third_marks(sups, bigs, segs, strokes, &zhongshus, highs, lows)
}

/// 大段三买/三卖 —— **生产入口**（按 [`BIGSEG_THIRD_ALL_ZHONGSHU`] 决定中枢集口径）。
///
/// 调用方一律用本函数取代「自取中枢集 + 调 `detect_bigseg_third_marks`」，
/// 以便口径单点可控、可回滚。
pub fn bigseg_third_marks_all_zhongshu(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSegThirdMarker> {
    let zhongshus = if BIGSEG_THIRD_ALL_ZHONGSHU {
        // 主图那套：每高级段内**全部**中枢（A/B/C/扩）
        detect_bigseg_zhongshus_by_overlap(superior_segments, big_segments, segments, strokes)
    } else {
        // 回滚口径：每高级段**只第一个**中枢
        detect_bigseg_zhongshus(superior_segments, big_segments, segments, strokes)
    };
    detect_bigseg_third_marks(
        superior_segments,
        big_segments,
        segments,
        strokes,
        &zhongshus,
        highs,
        lows,
    )
}

/// 大段 **3+N买/3+N卖**: 三买的递推扩展。
///
/// 🔴 **架构保证（不变式的来源）**：`order == 0`（大三买/大三卖**基准**）直接**播种**自
///    [`detect_bigseg_third_marks`]，本函数只**追加** `order >= 1` 的递推项（大3+1 / 大3+2 …）。
///    ⇒ 「`order == 0` ≡ 现行大三买/大三卖」是**结构性**成立，与任何递推边界的选择无关；
///      初始数据补充分支的边界因此只影响**新增项**，不可能改写既有信号。
/// 3+N 重复登记防护: 同 (bar, 方向) 只保留最小 order
fn push_3n(out: &mut Vec<BigSeg3NMarker>, bar_index: usize, price: f64, is_buy: bool, order: u32) {
    if let Some(existing) = out
        .iter_mut()
        .find(|m| m.bar_index == bar_index && m.is_buy == is_buy)
    {
        log::debug!(
            "[3+N] 重复登记 bar={bar_index} buy={is_buy}: 既有 order={} 新 order={order}",
            existing.order
        );
        if order < existing.order {
            existing.order = order;
            existing.price = price;
        }
        return;
    }
    out.push(BigSeg3NMarker {
        bar_index,
        price,
        is_buy,
        order,
    });
}

pub fn detect_bigseg_3n_markers(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    zhongshus: &[BigSegmentZhongShu],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSeg3NMarker> {
    // ① 播种 order=0 = 现行大三买/大三卖 (基准, 逐条不变)
    let mut out: Vec<BigSeg3NMarker> = detect_bigseg_third_marks(
        superior_segments, big_segments, segments, strokes, zhongshus, highs, lows,
    )
    .into_iter()
    .map(|m| BigSeg3NMarker {
        bar_index: m.bar_index,
        price: m.price,
        is_buy: m.is_buy,
        order: 0,
    })
    .collect();

    // ② 递推扩展 —— 🔴 **口径 A**：
    //    **一个宿主高级段 = 一条 3+N 序列**（与 `detect_bigseg_cf_*`(2+N)「一个检测点一条序列」同构）。
    //
    //    ⚠️ 实现要点（吃过亏，勿回退）：**段内必须先按 bar 全局归并、再统一编号**。
    //    最初写成「段内每条中枢各扫一遍、共用计数器」，结果同一 bar 被两条中枢各编一次号
    //    （实测 sup#35：71327 既是 3+1 又是 3+4；72243 既是 3+2 又是 3+5）⇒ 序列交错、出现空洞。
    //    正确做法 = 段内把所有中枢扫描到的回试段终点**按 bar 去重排序**，然后**依次 0/1/2…** 编号。
    //
    //    规则原文：
    //    > 「只要满足大三卖之后，只要涉及到该三卖的中枢延伸的大段卖点，都要依次以『大3+N卖』标记，
    //    >   **不能跳过、不能省略**」
    //    ⇒ 递推项**不再**用中枢 ZG/ZD 过滤（2+N 的上界是「检测点价」，本就与中枢无关）；
    //      基准（order 0，= 现行大三买/大三卖）仍由 `detect_bigseg_third_marks` 播种，一行未动。
    //
    //    基准（order 0）= 播种集原样不动；段内**其余**扫描到的回试段按 bar 顺序编 1,2,3…
    let mut seg_bars: std::collections::HashMap<usize, Vec<(usize, bool, f64)>> =
        std::collections::HashMap::new();
    for zs in zhongshus {
        let (big_start, big_end, sup_up) = superior_segments[zs.sup_idx];
        // 中枢后第一段: 第一个起点 bar >= 中枢右边界的大段 (与 detect_bigseg_third_marks 同)
        let mut k = big_start;
        while k <= big_end {
            if let Some(br) = resolve_bigseg_range(big_segments[k], segments, strokes) {
                if br.start_bar >= zs.end_bar {
                    break;
                }
            }
            k += 1;
        }
        // 界与 detect_bigseg_third_marks 逐条一致: 只按**跨高级段**取 (同宿主段内互不设界)
        let next_zs_start = zhongshus
            .iter()
            .filter(|z| z.start_bar > zs.end_bar && z.sup_idx != zs.sup_idx)
            .map(|z| z.start_bar)
            .min()
            .unwrap_or(usize::MAX);
        // ［规则］
        //    「只要向下的高级段成立，买点必须回撤到向上的高级段内」⇒
        //    宿主中枢之后**第一个与宿主段反向的高级段成立点** = 本中枢 3+N 递推的**硬上界**：
        //    买链 (离开=UP) 界 = 其后第一个**向下**高级段终点 bar；卖链镜像 = 第一个**向上**高级段终点。
        //    回试段终点越过它 ⇒ break (序列终结, 其后归新结构)。
        //    ⚠️ 与撤回过滤的事件时序 (cut=触发段终点, 只删已存在标记) 互补: 本守卫管住**递推扫描本身**,
        //    撤回管不住「扫描在后、成立在后」的漏网项 (某历史案例即此)。
        let zhongshu_sup2_dir = sup_up;
        let seq_end_bar = superior_segments
            .iter()
            .enumerate()
            .filter(|(i, s)| *i > zs.sup_idx && s.2 != zhongshu_sup2_dir) // 反向高级段
            .filter_map(|(i, _)| {
                let (bs, be, _) = superior_segments[i];
                if bs > be || be >= big_segments.len() { return None; }
                let b2 = strokes.get(segments.get(big_segments[be].1)?.1)?;
                Some(b2.end_bar)
            })
            .find(|&end_bar| end_bar > zs.end_bar);
        while k + 1 < big_segments.len() {
            let Some(r1) = resolve_bigseg_range(big_segments[k + 1], segments, strokes) else {
                break;
            };
            if r1.end_bar >= next_zs_start {
                break;
            }
            // 🔴  过渡期截断守卫: 回试终点越过「反向高级段成立点」⇒ 序列终结 (见上方注释)
            if let Some(seq_end) = seq_end_bar {
                if r1.end_bar >= seq_end {
                    break;
                }
            }
            let is_up_leave = big_segments[k].2;
            // 方向守卫 : 宿主高级段方向必须 == 离开段方向
            if is_up_leave != sup_up {
                k += 1;
                continue;
            }
            seg_bars.entry(zs.sup_idx).or_default().push((
                r1.end_bar,
                is_up_leave,
                if is_up_leave { lows[r1.end_bar] } else { highs[r1.end_bar] },
            ));
            k += 1;
        }
    }
    // 段内: 去重 → 按 bar 排序 → 已是基准(order 0)者跳过, 其余依次 1,2,3…
    let mut seg_ids: Vec<usize> = seg_bars.keys().copied().collect();
    seg_ids.sort_unstable();
    for sid in seg_ids {
        let Some(list) = seg_bars.get(&sid) else { continue };
        let mut uniq: Vec<(usize, bool, f64)> = Vec::new();
        {
            let mut seen: std::collections::HashSet<(usize, bool)> = std::collections::HashSet::new();
            let mut sorted = list.clone();
            sorted.sort_by_key(|&(b, buy, _)| (b, buy));
            for (b, buy, p) in sorted {
                if seen.insert((b, buy)) {
                    uniq.push((b, buy, p));
                }
            }
        }
        // ［规则］3+N 是「三买/三卖的递推扩展」，
        //    **无基准 ⇒ 无递推**。段内按 bar 序遍历: 遇到基准 (order 0, 播种自
        //    detect_bigseg_third_marks) 才锚定序列; 锚定前的项 (回试价未达 ZG/ZD 的未命中对)
        //    **丢弃不占号**; 本段无基准 ⇒ 整段不产任何 3+N 项。
        //    案例: 某历史案例 — 基准大三卖未命中 (回试高点超 ZD
        //    0.82782), 旧逻辑却从 3+1 起编 ⇒ 无锚序列 (BUG 根因)。
        //    锚后口径不变: 基准之后每项依次 3+1/3+2… (不能跳过不能省略,  定版)。
        let mut order: u32 = 1;
        let mut anchored = false;
        for (b, buy, p) in uniq {
            // 基准 (播种集, order 0) ⇒ 锚定序列, 自身不重编
            if out.iter().any(|m| m.bar_index == b && m.is_buy == buy && m.order == 0) {
                anchored = true;
                continue;
            }
            // 无锚项: 基准尚未成立 ⇒ 丢弃 (不占号)
            if !anchored {
                continue;
            }
            push_3n(&mut out, b, p, buy, order);
            order += 1;
        }
    }
    // ── 初始数据特定情况 (与 detect_bigseg_third_marks 同条件; 递推项同款扩展) ──
    // 变体1: sups 首段 UP 且起点 > 0 → 开头游离向下大段作补充检测点 (C = 后首个向上大段高点)
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
            // 递推上界 = 第一个同向(向下)高级段终点 (与 2+N 变体1 的 first_down_sup_end 逐条同义)
            let first_down_sup_end = superior_segments
                .iter()
                .find(|s| !s.2)
                .and_then(|&(_, eb, _)| crate::resolve_sup_end_point(eb, big_segments, segments, strokes));
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(k, big_segments, segments, strokes) else { continue; };
                let Some((_, c_price)) = up_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                let mut order: u32 = 0;
                for (rb, _) in down_bigs.iter().filter(|(b, _)| *b > dbe) {
                    if let Some((b, _)) = first_down_sup_end {
                        if *rb >= b {
                            break;
                        }
                    }
                    if *rb < lows.len() && lows[*rb] > c_price {
                        push_3n(&mut out, *rb, lows[*rb], true, order);
                        order += 1;
                    }
                }
            }
        }
    }
    // 变体1 卖镜像: sups 首段 DOWN 且起点 > 0 → 开头游离向上大段作补充检测点
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
            // 递推上界 = 第一个同向(向上)高级段终点 (卖镜像)
            let first_up_sup_end = superior_segments
                .iter()
                .find(|s| s.2)
                .and_then(|&(_, eb, _)| crate::resolve_sup_end_point(eb, big_segments, segments, strokes));
            for k in 0..first_start {
                let Some((dbe, _)) = crate::resolve_sup_end_point(k, big_segments, segments, strokes) else { continue; };
                let Some((_, c_price)) = down_bigs.iter().find(|(b, _)| *b > dbe).copied() else { continue; };
                let mut order: u32 = 0;
                for (rb, _) in up_bigs.iter().filter(|(b, _)| *b > dbe) {
                    if let Some((b, _)) = first_up_sup_end {
                        if *rb >= b {
                            break;
                        }
                    }
                    if *rb < highs.len() && highs[*rb] < c_price {
                        push_3n(&mut out, *rb, highs[*rb], false, order);
                        order += 1;
                    }
                }
            }
        }
    }
    out
}

/// 大段 **3+N买/3+N卖 —— 生产入口**（中枢集口径与 [`bigseg_third_marks_all_zhongshu`] 同一开关）。
///
/// `BIGSEG_THIRD_ALL_ZHONGSHU == true` ⇒ 中枢集 = 主图那套（每高级段**全部**中枢, A/B/C/扩）;
/// `false` ⇒ 回滚口径（每高级段**只第一个**）。
pub fn bigseg_3n_marks_all_zhongshu(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSeg3NMarker> {
    let zhongshus = if BIGSEG_THIRD_ALL_ZHONGSHU {
        detect_bigseg_zhongshus_by_overlap(superior_segments, big_segments, segments, strokes)
    } else {
        detect_bigseg_zhongshus(superior_segments, big_segments, segments, strokes)
    };
    detect_bigseg_3n_markers(
        superior_segments,
        big_segments,
        segments,
        strokes,
        &zhongshus,
        highs,
        lows,
    )
}

/// 高级段级 **3+N买 / 3+N卖**。
///
/// 与 [`detect_bigseg_3n_markers`]（大段级）**逐条同构**，层级整体提升一级：
/// | 大段级 | 高级段级 |
/// |---|---|
/// | 宿主 = 高级段 (`superior_segments`) | 宿主 = 高高级段 (`sup2s`) |
/// | 部件 = 大段 (`big_segments`) | 部件 = 高级段 (`sups`) |
/// | 中枢 = `BigSegmentZhongShu` | 中枢 = [`SuperiorZhongShu`] |
/// | 基准 = `detect_bigseg_third_marks` | 基准 = [`detect_superior_third_marks`] |
///
/// 🔴 **口径 A（与大段级一致）**：**一个宿主高高级段 = 一条 3+N 序列**；
///    段内多条中枢**共户同一条序列**；段内先按 bar 全局归并去重再统一编号；
///    递推项**不判价格**（「不能跳过、不能省略」）。
///    编号语义：`0` = 基准高三买/高三卖，`1` = 高3+1，`2` = 高3+2 …
pub fn detect_superior_3n_markers(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    zhongshus: &[SuperiorZhongShu],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSeg3NMarker> {
    let sup2s = crate::process_superior_superior_segments(strokes, segs, bigs, sups);
    // ① 播种 order=0 = 现行高三买/高三卖（基准，逐条不变）
    let mut out: Vec<BigSeg3NMarker> = detect_superior_third_marks(
        sups, bigs, segs, strokes, zhongshus, highs, lows,
    )
    .into_iter()
    .map(|m| BigSeg3NMarker {
        bar_index: m.bar_index,
        price: m.price,
        is_buy: m.is_buy,
        order: 0,
    })
    .collect();

    // ② 递推扩展 —— 口径 A：一个宿主高高级段 = 一条序列
    let mut seg_bars: std::collections::HashMap<usize, Vec<(usize, bool, f64)>> =
        std::collections::HashMap::new();
    for zs in zhongshus {
        let Some(&(sup_start, sup_end, sup2_up)) = sup2s.get(zs.sup_idx) else { continue };
        // 中枢后第一段: 第一个起点 bar >= 中枢右边界的高级段
        let mut k = sup_start;
        while k <= sup_end {
            if let Some(br) = resolve_sup_range(sups[k], bigs, segs, strokes) {
                if br.2 >= zs.end_bar {
                    break;
                }
            }
            k += 1;
        }
        // 界与大段级/detect_superior_third_marks 逐条一致: 只按**跨高高级段**取
        let next_zs_start = zhongshus
            .iter()
            .filter(|z| z.start_bar > zs.end_bar && z.sup_idx != zs.sup_idx)
            .map(|z| z.start_bar)
            .min()
            .unwrap_or(usize::MAX);
        // 🔴 **（同向段未成立窗口买卖点截断, 高3+N 递推窗口守卫, 与 高2+N 同规）**；
        //    🔴 ** 层级修正（定版「大三买只存在于自己的高级段中」定版链推及）**：
        //    离开/回试对 = 高级段 ⇒ 界 = **高一级（高高级段 sup2s）** — 与大段级 3+N（对=大段,
        //    界=高级段）逐条同构; 不得降级扫高级段（同级 ⇒ 回试段自身成立点=信号点, 误杀合法项,
        //    见 detect_superior_third_marks 同处注释）。买链界 = 第一个反向**高高级段**成立点, 卖链镜像。
        let seq_end_bar = sup2s
            .iter()
            .enumerate()
            .filter(|(i, s)| *i > zs.sup_idx && s.2 != sup2_up) // 反向高高级段
            .filter_map(|(i, _)| {
                let (ss, se, _) = sup2s[i];
                if ss > se || se >= sups.len() { return None; }
                crate::resolve_sup_end_point(sups[se].1, bigs, segs, strokes).map(|(b, _)| b)
            })
            .find(|&b| b > zs.end_bar);
        while k + 1 < sups.len() {
            let Some(r1) = resolve_sup_range(sups[k + 1], bigs, segs, strokes) else { break };
            if r1.3 >= next_zs_start {
                break;
            }
            // 🔴  过渡期截断守卫 (与大段级 3+N 逐行同构): 回试终点越过「反向高级段成立点」⇒ 序列终结
            if let Some(seq_end) = seq_end_bar {
                if r1.3 >= seq_end {
                    break;
                }
            }
            let is_up_leave = sups[k].2;
            // 方向守卫 : 宿主高高级段方向必须 == 离开高级段方向
            if is_up_leave != sup2_up {
                k += 1;
                continue;
            }
            // 🔴 （3+N 显著特征，2+N 家族同源）：
            //    「无论 2+N 还是 3+N，其中有个显著特征：**自己本级段必然重叠**」——
            //    递推项的**自身本级段**（回试段, k+1）必须与其**左侧紧邻的同向段**（k-1,
            //    前一个回试段）价格区间存在重叠；不重叠 ⇒ 该点**不是前基准的递推项**，
            //    而是**新基准「高3卖/高3买」(order 0)**，序列从它重新起算。
            //    户户案例：2022-01-14 13:30 高3+1卖 (sup#15, 价[1.11864..1.14829])
            //    < 其左侧紧邻向上高级段 sup#13 起点 (bar 27679)，零重叠 ⇒ 应为「高3卖」。
            // 左侧紧邻同向段 = k-1 (sups 方向交替 ⇒ k+1 的同向段在 k-1); k==0 无左邻 → 免判
            let overlap_ok = match (k > 0).then(|| k - 1).and_then(|pk| resolve_sup_range(sups[pk], bigs, segs, strokes)) {
                None => true,
                Some(prev) => {
                    let (a_lo, a_hi) = (r1.0.min(r1.1), r1.0.max(r1.1));
                    let (b_lo, b_hi) = (prev.0.min(prev.1), prev.0.max(prev.1));
                    a_lo <= b_hi && a_hi >= b_lo
                }
            };
            if !overlap_ok {
                // 新基准 (order 0, 高3卖/高3买): 极性与递推项一致 (向下离开→高3卖);
                // 去重后登记, 并在段内序列中以 order -1 占位 ⇒ 编号从它**重新起算**。
                if !out.iter().any(|m| m.bar_index == r1.3 && m.is_buy == is_up_leave && m.order == 0) {
                    out.push(BigSeg3NMarker {
                        bar_index: r1.3,
                        price: if is_up_leave { lows[r1.3] } else { highs[r1.3] },
                        is_buy: is_up_leave,
                        order: 0,
                    });
                }
                seg_bars.entry(zs.sup_idx).or_default().push((r1.3, is_up_leave, f64::NEG_INFINITY));
                k += 1;
                continue;
            }
            seg_bars.entry(zs.sup_idx).or_default().push((
                r1.3,
                is_up_leave,
                if is_up_leave { lows[r1.3] } else { highs[r1.3] },
            ));
            k += 1;
        }
    }
    // 段内: 去重 → 按 bar 排序 → 基准(order 0)者跳过, 其余依次 1,2,3…
    let mut seg_ids: Vec<usize> = seg_bars.keys().copied().collect();
    seg_ids.sort_unstable();
    for sid in seg_ids {
        let Some(list) = seg_bars.get(&sid) else { continue };
        let mut uniq: Vec<(usize, bool, f64)> = Vec::new();
        {
            let mut seen: std::collections::HashSet<(usize, bool)> = std::collections::HashSet::new();
            let mut sorted = list.clone();
            sorted.sort_by_key(|&(b, buy, _)| (b, buy));
            for (b, buy, p) in sorted {
                if seen.insert((b, buy)) {
                    uniq.push((b, buy, p));
                }
            }
        }
        // 编号 (🔴 ): 遇锚 (播种基准 order==0 或
        // 不重叠转正新基准 NEG_INFINITY 占位, 均已在 out 里 order=0) ⇒ **锚定/重新起算**;
        // 锚定前的无锚项 (基准未成立的未命中对) ⇒ **丢弃不占号**; 本段无锚 ⇒ 整段不产 3+N。
        // 其余依次 1,2,3…
        let mut order: u32 = 1;
        let mut anchored = false;
        for (b, buy, p) in uniq {
            if p == f64::NEG_INFINITY {
                // 不重叠转正新基准: 锚定 + 序列重置 (自身 order=0 已登记)
                anchored = true;
                order = 1;
                continue;
            }
            // 播种基准 (order==0 已在 out) ⇒ 锚定序列, 自身不重编
            if out.iter().any(|m| m.bar_index == b && m.is_buy == buy && m.order == 0) {
                anchored = true;
                continue;
            }
            // 无锚项: 基准尚未成立 ⇒ 丢弃 (不占号)
            if !anchored {
                continue;
            }
            push_3n(&mut out, b, p, buy, order);
            order += 1;
        }
    }
    out
}

/// 高级段级 **3+N买/3+N卖 —— 生产入口**（中枢集口径由 [`SUPERIOR_THIRD_ALL_ZHONGSHU`] 控制；
/// 与大段级 [`bigseg_3n_marks_all_zhongshu`] **逐条同构**）。
///
/// ［规则］：「高级段的大段中枢的算法…早都从**只许首个中枢**变成**多个中枢**了」
///    ⇒ 中枢集一律用 **by_overlap**（每高高级段内**全部**中枢），与大段级完全一致。
pub fn superior_3n_marks_all_zhongshu(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    highs: &[f64],
    lows: &[f64],
) -> Vec<BigSeg3NMarker> {
    let zhongshus = superior_zhongshu_set(sups, bigs, segs, strokes);
    detect_superior_3n_markers(sups, bigs, segs, strokes, &zhongshus, highs, lows)
}

/// 高级段级 **3+N买/3+N卖** 的撤回过滤器 —— 与 [`crate::guidao::apply_sup_retreat_filter_3n`]
/// （大段级）**逐条同构**，层级提升一级（宿主 高高级段 / 部件 高级段）。
///
/// ［规则］：`x` = 前驱区间内**最后一个买/卖点**，
///    必须把 **3+N 自己的 bar 也算进候选池**（与「高2+N」同规）；漏算会让 3+N 被自己的撤回规则删掉。
///    ⚠️ 本函数只对高级 3+N 列表做 retain ⇒ **不改变 高二买/高2+N/高三买 既有信号**。
pub fn apply_sup2_retreat_filter_3n(
    sups: &[(usize, usize, bool)],
    bigs: &[(usize, usize, bool)],
    segs: &[(usize, usize, bool)],
    strokes: &[Stroke],
    superior_second_markers: &[crate::SecondMarker],
    superior_third_markers: &[BigSegThirdMarker],
    superior_plus_markers: &[crate::guidao::SecondPlusOneMarker],
    mut markers: Vec<BigSeg3NMarker>,
) -> Vec<BigSeg3NMarker> {
    let sup2s = crate::process_superior_superior_segments(strokes, segs, bigs, sups);
    // 买点撤回: 每个向下高高级段成立 (索引>0 才有前驱向上高高级段)
    for d_idx in (1..sup2s.len()).filter(|&i| !sup2s[i].2) {
        if let Some((sb, eb)) = sup2_range(sup2s[d_idx - 1], sups, bigs, segs, strokes) {
            let mut buy_bars: Vec<usize> = Vec::new();
            for m in superior_second_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in superior_third_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in superior_plus_markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            for m in markers.iter().filter(|m| m.is_buy) { buy_bars.push(m.bar_index); }
            let cut = sup2_range(sup2s[d_idx], sups, bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match buy_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => markers.retain(|m| !m.is_buy || m.bar_index <= x || m.bar_index > cut),
                None => markers.retain(|m| !m.is_buy || m.bar_index <= eb || m.bar_index > cut),
            }
        }
    }
    // 卖点镜像: 每个向上高高级段成立 (索引>0) → 撤回到前驱向下高高级段区间内最后卖点
    for u_idx in (1..sup2s.len()).filter(|&i| sup2s[i].2) {
        if let Some((sb, eb)) = sup2_range(sup2s[u_idx - 1], sups, bigs, segs, strokes) {
            let mut sell_bars: Vec<usize> = Vec::new();
            for m in superior_second_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in superior_third_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in superior_plus_markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            for m in markers.iter().filter(|m| !m.is_buy) { sell_bars.push(m.bar_index); }
            let cut = sup2_range(sup2s[u_idx], sups, bigs, segs, strokes).map(|(_, e)| e).unwrap_or(usize::MAX);
            match sell_bars.into_iter().filter(|&b| sb <= b && b <= eb).max() {
                Some(x) => markers.retain(|m| m.is_buy || m.bar_index <= x || m.bar_index > cut),
                None => markers.retain(|m| m.is_buy || m.bar_index <= eb || m.bar_index > cut),
            }
        }
    }
    markers
}
