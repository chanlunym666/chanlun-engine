//! 大段中枢 — 基于高级段内部大段序列的重叠区间识别
//!
//! 规则:
//!   1. 以高级段为单位, 每高级段只统计第一个中枢
//!   2. 门槛: 高级段内部大段数 >= 3;
//!      大段数 = 3 (仅 1 个反向段): 中枢区间 = 位置2 反向大段高低点
//!      大段数 >= 4: 需存在第2/第4位置判定对
//!   3. 判定对: 位置2段 与 位置4段 (同向, 均为高级段方向的反向段)
//!      向下高级段 = 第一个向上大段 U1 与第二个向上大段 U2
//!      向上高级段 = 第一个向下大段 D1 与第二个向下大段 D2 (对称)
//!   4. 重叠确认: zd = max(low1, low2), zg = min(high1, high2)
//!      zd < zg → 重叠中枢 (区间 = [zd,zg], 可延伸)
//!      zd >= zg → 不重叠, 退化为第一个反向段自身 (区间 = 位置2 段高低点, 不延伸)
//!       (存在两个反向段但无重叠时, 第一个反向段也要画中枢)
//!   4.x 大段高低点 = **大段两端点价** (首线段起点分型价/末线段终点分型价), 非内部
//!       线段端点极值 (大段中枢只借用大段端点, 不管大段内部)
//!   5. 延伸: 后续段与 [zd, zg] 重叠 (low <= zg && high >= zd, 恰好相等也延伸),
//!      首次不重叠 (离开区间) 即停止, 离开后即使跌回也不恢复;
//!      终点只推进到"与判定对同向"的段 (向下高级段内收向上段 U3...,
//!      向上高级段内收向下段 D3...), 右边界始终截止于同向段终点
//!   6. 不越界: 延伸范围不超出所属高级段 (反向高级段出现即止)
//!   7. GG/DD = 位置2..最后参与段全部大段极值 (存数据, 渲染不画)
//!
//! 三买/三卖 (detect_bigseg_third_marks):
//!   8. 每中枢最多 1 个标记: 从中枢最后参与段之后逐对扫描 (k, k+1)
//!      三买对 = (向上大段离开, 向下大段回试): 回试段终点bar最低价 > ZG → 标记"三买"
//!      三卖对 = (向下大段离开, 向上大段回试): 回试段终点bar最高价 < ZD → 标记"三卖"
//!      第一对满足即标记并停止; 离开段不要求创新高; 不越出所属高级段

use crate::Stroke;

/// 大段中枢 (Big Segment ZhongShu)
#[derive(Debug, Clone)]
pub struct BigSegmentZhongShu {
    /// 所属高级段索引 (superior_segments)
    pub sup_idx: usize,
    /// 矩形左边界 = 判定对第1段 (位置2) 起点 bar
    pub start_bar: usize,
    /// 矩形右边界 = 最后参与段 (含延伸) 终点 bar
    pub end_bar: usize,
    /// 中枢上沿 (判定对锁定, 固定不变)
    pub zg: f64,
    /// 中枢下沿 (判定对锁定, 固定不变)
    pub zd: f64,
    /// 参与段整体最高 (存数据, 渲染不画)
    pub gg: f64,
    /// 参与段整体最低 (存数据, 渲染不画)
    pub dd: f64,
}

/// 单个大段解析出的价格区间与时间范围
struct BigSegRange {
    low: f64,
    high: f64,
    start_bar: usize,
    end_bar: usize,
}

/// 解析大段价格区间: 大段 = (线段起始, 线段结束), 经 segments → strokes 两级解析,
/// 区间 = **大段两端点价** (首线段起点分型价 与 末线段终点分型价) 的极值,
/// 时间 = 首线段起点 bar → 末线段终点 bar。
///
/// ⚠️ 大段中枢只借用大段端点, 不用管大段内部如何;
/// 内部线段端点价极值 (旧实现) 会把大段内部突破两端的线段端点收进中枢区间
/// (单反向段: 段内最低点低于大段终点价时, zd 会误取段内最低点),
/// 并导致中枢延伸过度吸收、三买无法标记。
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

/// 检测全部高级段内的第一个大段中枢 (规则详见模块注释)。
pub fn detect_bigseg_zhongshus(
    superior_segments: &[(usize, usize, bool)],
    big_segments: &[(usize, usize, bool)],
    segments: &[(usize, usize, bool)],
    strokes: &[Stroke],
) -> Vec<BigSegmentZhongShu> {
    let mut out: Vec<BigSegmentZhongShu> = Vec::new();
    for (sup_idx, &(big_start, big_end, sup_up)) in superior_segments.iter().enumerate() {
        // 门槛: 内部大段数 < 3 → 无反向段结构 → 无中枢
        if big_end < big_start + 2 || big_end >= big_segments.len() {
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} SKIP: 内部大段数<3 无反向段结构", sup_idx, big_start, big_end, sup_up);
            continue;
        }
        // 位置2 = 第一个反向段
        let Some(b1) = resolve_bigseg_range(big_segments[big_start + 1], segments, strokes) else {
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} SKIP: 位置2 段解析失败", sup_idx, big_start, big_end, sup_up);
            continue;
        };
        // 单反向段中枢: 区间 = 位置2 段高低点 (3 大段, 或判定对不重叠时退化)
        let single_reverse = |end_bar: usize| BigSegmentZhongShu {
            sup_idx,
            start_bar: b1.start_bar,
            end_bar,
            zg: b1.high,
            zd: b1.low,
            gg: b1.high,
            dd: b1.low,
        };
        // 仅 1 个反向段 (内部 3 大段: 正-反-正): 中枢区间 = 位置2 段高低点
        if big_end == big_start + 2 {
            let zs = single_reverse(b1.end_bar);
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} 单反向段中枢 [{}..{}] zg={:.3} zd={:.3}", sup_idx, big_start, big_end, sup_up, zs.start_bar, zs.end_bar, zs.zg, zs.zd);
            out.push(zs);
            continue;
        }
        // 判定对: 位置4 (big_start+3), 与位置2 同向段
        let Some(b2) = resolve_bigseg_range(big_segments[big_start + 3], segments, strokes) else {
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} SKIP: 位置4 段解析失败", sup_idx, big_start, big_end, sup_up);
            continue;
        };
        // 重叠确认: zd < zg 才构成重叠中枢
        let zd = b1.low.max(b2.low);
        let zg = b1.high.min(b2.high);
        if zd >= zg {
            // 不重叠: 退化为第一个反向段自身
            let zs = single_reverse(b1.end_bar);
            log::debug!("[zs] sup#{} 大段[{}..{}] up={} 判定对不重叠 zd={:.3}>=zg={:.3} → 退化单反向段中枢 [{}..{}] zg={:.3} zd={:.3}", sup_idx, big_start, big_end, sup_up, zd, zg, zs.start_bar, zs.end_bar, zs.zg, zs.zd);
            out.push(zs);
            continue;
        }
        // 中枢成立: 区间 [zd, zg] 锁定; 参与段 = 位置2..位置4 (含中间段位置3)
        let mut end_bar = b2.end_bar;
        let mut gg = b1.high.max(b2.high);
        let mut dd = b1.low.min(b2.low);
        if let Some(bm) = resolve_bigseg_range(big_segments[big_start + 2], segments, strokes) {
            gg = gg.max(bm.high);
            dd = dd.min(bm.low);
        }
        // 延伸: 位置5起, 与 [zd, zg] 重叠 (low <= zg && high >= zd, 恰好相等也延伸),
        // 首次不重叠 (离开区间) 即停止, 离开后即使跌回也不恢复。
        // 终点只推进到"与判定对同向"的段 (big[j].2 != sup_up): 向下高级段内收向上段,
        // 向上高级段内收向下段, 保证右边界始终截止于同向段终点
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
        // 每高级段只统计第一个中枢
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

/// 大段中枢三买/三卖标记
#[derive(Debug, Clone, PartialEq)]
pub struct BigSegThirdMarker {
    /// 回试段终点 bar
    pub bar_index: usize,
    /// 标记价格 (三买=终点bar最低价, 三卖=终点bar最高价)
    pub price: f64,
    /// true=三买 (回试低点不破 ZG), false=三卖 (回试高点不破 ZD)
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
        let (big_start, big_end, _) = superior_segments[zs.sup_idx];
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
        // 逐对 (k, k+1): 三买对 = (向上, 向下), 三卖对 = (向下, 向上)
        // 回试段边界: 大段中枢三买只考虑中枢之后
        // 的大段终点价 vs ZG (大段级别, 不看内部线段) — 离开段是高级段最后大段时,
        // 若其后还有大段 (可能不属于任何高级段), 仍判定这一对.
        let pair_end = if k == big_end && k + 1 < big_segments.len() { k + 1 } else { big_end };
        while k + 1 <= pair_end {
            let Some(r1) = resolve_bigseg_range(big_segments[k + 1], segments, strokes) else {
                log::debug!("[3rd] 中枢 sup#{} 判定对 k={} SKIP: 回试段解析失败", zs.sup_idx, k);
                break;
            };
            let is_up_leave = big_segments[k].2;
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
                out.push(BigSegThirdMarker {
                    bar_index: r1.end_bar,
                    price: if is_up_leave { lows[r1.end_bar] } else { highs[r1.end_bar] },
                    is_buy: is_up_leave,
                });
                break;
            }
            k += 1;
        }
        if k + 1 > pair_end {
            log::debug!("[3rd] 中枢 sup#{} 遍历完判定范围[{}..{}] 无三买/三卖", zs.sup_idx, big_start, pair_end);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(is_up: bool, start_price: f64, end_price: f64, start_bar: usize, end_bar: usize) -> Stroke {
        Stroke { is_up, start_price, end_price, start_bar, end_bar }
    }

    /// 简易映射模型: 每笔 = 一线段 = 一大段
    fn ladder(n: usize, strokes: Vec<Stroke>) -> (Vec<Stroke>, Vec<(usize, usize, bool)>, Vec<(usize, usize, bool)>) {
        let segs: Vec<(usize, usize, bool)> =
            (0..n).map(|i| (i, i, strokes[i].is_up)).collect();
        let bigs: Vec<(usize, usize, bool)> =
            (0..n).map(|i| (i, i, strokes[i].is_up)).collect();
        (strokes, segs, bigs)
    }

    #[test]
    fn down_sup_4_bigs_overlap_zhongshu() {
        // 向下高级段: D1(下) U1(上) D2(下) U2(上), U1 与 U2 重叠 → 中枢 [105,115]
        let (strokes, segs, bigs) = ladder(4, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
        ]);
        let sups = vec![(0usize, 3usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.sup_idx, 0);
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 40);
        assert_eq!(zs.zd, 105.0);
        assert_eq!(zs.zg, 115.0);
        assert_eq!(zs.gg, 118.0);
        assert_eq!(zs.dd, 100.0);
    }

    fn down_sup_extend_overlapping() {
        // 6 大段: 延伸段 D3/U3 与 [105,115] 重叠 → 延伸至末尾
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 110.0, 40, 50),
            stroke(true, 110.0, 116.0, 50, 60),
        ]);
        let sups = vec![(0usize, 5usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 60); // 延伸至高级段末尾
        assert_eq!(zs.zd, 105.0);
        assert_eq!(zs.zg, 115.0);
        assert_eq!(zs.gg, 118.0);
        assert_eq!(zs.dd, 100.0);
    }

    #[test]
    fn extend_stops_on_leave_no_resume() {
        // D3 完全离开区间上方 (low=116 > zg=115) → 延伸停止; U3 即使存在也不参与
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 116.0, 40, 50),
            stroke(true, 116.0, 130.0, 50, 60),
        ]);
        let sups = vec![(0usize, 5usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.end_bar, 40); // 只到 U2 终点, D3/U3 不参与
        assert_eq!(zs.gg, 118.0);
        assert_eq!(zs.dd, 100.0);
    }

    #[test]
    fn extend_touching_boundary_overlap_continues_same_dir_advances() {
        // D3 触及上沿 (low == zg) 仍算重叠 (恰好相等也延伸), 延伸循环继续;
        // 但 D3 为异向段不推进终点, U3 (同向) 重叠才推进终点
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 115.0, 40, 50), // [115,118]: low == zg 恰好相等
            stroke(true, 115.0, 116.0, 50, 60),  // U3 与 [105,115] 重叠
        ]);
        let sups = vec![(0usize, 5usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end_bar, 60); // D3 不推进, U3 推进
    }

    #[test]
    fn down_sup_opposite_dir_overlap_end_kept() {
        // 向下高级段: D3 (向下, 异向) 与中枢重叠 → 终点保持 U2 终点, 不推进到 D3;
        // GG/DD 仍统计 D3
        let (strokes, segs, bigs) = ladder(5, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 110.0, 40, 50), // D3 与 [105,115] 重叠
        ]);
        let sups = vec![(0usize, 4usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start_bar, 10);
        assert_eq!(out[0].end_bar, 40); // 保持 U2 终点 (向上段)
        assert_eq!(out[0].zd, 105.0);
        assert_eq!(out[0].zg, 115.0);
        assert_eq!(out[0].gg, 118.0); // GG/DD 仍统计 D3
        assert_eq!(out[0].dd, 100.0);
    }

    #[test]
    fn up_sup_extend_only_same_dir_end() {
        // 向上高级段 (对称): U3 (向上, 异向) 重叠不推进终点; D3 (向下, 同向) 重叠才推进
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(true, 100.0, 120.0, 0, 10),
            stroke(false, 120.0, 108.0, 10, 20),
            stroke(true, 108.0, 118.0, 20, 30),
            stroke(false, 118.0, 102.0, 30, 40),
            stroke(true, 102.0, 110.0, 40, 50),  // U3 与 [108,118] 重叠
            stroke(false, 110.0, 104.0, 50, 60), // D3 与 [108,118] 重叠
        ]);
        let sups = vec![(0usize, 5usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start_bar, 10);
        assert_eq!(out[0].end_bar, 60); // D3 终点 (向下段)
        assert_eq!(out[0].zd, 108.0);
        assert_eq!(out[0].zg, 118.0);
        assert_eq!(out[0].gg, 120.0);
        assert_eq!(out[0].dd, 102.0);
    }

    #[test]
    fn up_sup_symmetric() {
        // 向上高级段: U1(上) D1(下) U2(上) D2(下), D1 与 D2 重叠 → 中枢 [108,118]
        let (strokes, segs, bigs) = ladder(4, vec![
            stroke(true, 100.0, 120.0, 0, 10),
            stroke(false, 120.0, 108.0, 10, 20),
            stroke(true, 108.0, 118.0, 20, 30),
            stroke(false, 118.0, 102.0, 30, 40),
        ]);
        let sups = vec![(0usize, 3usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.zd, 108.0);
        assert_eq!(zs.zg, 118.0);
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 40);
    }

    #[test]
    fn inner_3_bigs_single_reverse_zhongshu() {
        // 仅 1 个反向段 (向下高级段 3 大段 D1 U1 D2): 也画中枢,
        // 区间 = 位置2 反向大段 U1 [100,115] 的高低点
        let (strokes, segs, bigs) = ladder(3, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
        ]);
        let sups = vec![(0usize, 2usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.sup_idx, 0);
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 20);
        assert_eq!(zs.zd, 100.0);
        assert_eq!(zs.zg, 115.0);
        assert_eq!(zs.gg, 115.0);
        assert_eq!(zs.dd, 100.0);
    }

    #[test]
    fn up_sup_3_bigs_single_reverse_zhongshu() {
        // 向上高级段 3 大段 U1 D1 U2 (仅 1 个反向段 D1): 区间 = D1 [108,120] 高低点
        let (strokes, segs, bigs) = ladder(3, vec![
            stroke(true, 100.0, 120.0, 0, 10),
            stroke(false, 120.0, 108.0, 10, 20),
            stroke(true, 108.0, 118.0, 20, 30),
        ]);
        let sups = vec![(0usize, 2usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.sup_idx, 0);
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 20);
        assert_eq!(zs.zd, 108.0);
        assert_eq!(zs.zg, 120.0);
        assert_eq!(zs.gg, 120.0);
        assert_eq!(zs.dd, 108.0);
    }

    #[test]
    fn inner_2_bigs_no_zhongshu() {
        // 内部仅 2 大段 (反向段未被第二个正向段确认): 不画中枢
        let (strokes, segs, bigs) = ladder(2, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
        ]);
        let sups = vec![(0usize, 1usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert!(out.is_empty());
    }

    #[test]
    fn no_overlap_falls_back_to_first_reverse() {
        // U1 [100,105] 与 U2 [85,95] 不重叠 → 退化为第一个反向段 U1 自身中枢,
        // 区间 = U1 高低点, 矩形 = U1 起止
        let (strokes, segs, bigs) = ladder(4, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 105.0, 10, 20),
            stroke(false, 105.0, 85.0, 20, 30),
            stroke(true, 85.0, 95.0, 30, 40),
        ]);
        let sups = vec![(0usize, 3usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.sup_idx, 0);
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 20);
        assert_eq!(zs.zd, 100.0);
        assert_eq!(zs.zg, 105.0);
        assert_eq!(zs.gg, 105.0);
        assert_eq!(zs.dd, 100.0);
    }

    #[test]
    fn up_sup_no_overlap_falls_back_to_first_reverse() {
        // 向上高级段: D1 [120,130] 与 D2 [140,145] 不重叠 (回调依次抬高)
        // → 退化画 D1 自身, 区间 [120,130], 矩形 = D1 起止
        let (strokes, segs, bigs) = ladder(4, vec![
            stroke(true, 100.0, 130.0, 0, 10),
            stroke(false, 130.0, 120.0, 10, 20),
            stroke(true, 120.0, 145.0, 20, 30),
            stroke(false, 145.0, 140.0, 30, 40),
        ]);
        let sups = vec![(0usize, 3usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 20);
        assert_eq!(zs.zd, 120.0);
        assert_eq!(zs.zg, 130.0);
        assert_eq!(zs.gg, 130.0);
        assert_eq!(zs.dd, 120.0);
    }

    #[test]
    fn only_first_zhongshu_per_sup() {
        // 6 大段且后续延伸段持续重叠: 只输出第一个中枢, 不再统计后续潜在中枢
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 106.0, 40, 50),
            stroke(true, 106.0, 120.0, 50, 60),
        ]);
        let sups = vec![(0usize, 5usize, false)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end_bar, 60);
    }

    #[test]
    fn single_reverse_zs_endpoints_not_inner() {
        // 单反向段回归场景: 向上高级段 3 大段 U1 D1 U2,
        // 仅 1 个反向段 D1; D1 内部 3 线段 (DN-UP-DN): 内部低点 108 < 大段终点 110 →
        // 中枢 zd 必须取大段端点 110 (大段中枢只借用大段端点, 不用管大段内部如何)
        let strokes = vec![
            stroke(true, 100.0, 120.0, 0, 10),   // 线段0: U1
            stroke(false, 120.0, 108.0, 10, 20),  // 线段1: D1 内部 (DN)
            stroke(true, 108.0, 115.0, 20, 30),   // 线段2: D1 内部 (UP)
            stroke(false, 115.0, 110.0, 30, 40),  // 线段3: D1 内部 (DN) = 大段终点 110
            stroke(true, 110.0, 118.0, 40, 50),   // 线段4: U2
        ];
        let segs: Vec<(usize, usize, bool)> = (0..5).map(|i| (i, i, strokes[i].is_up)).collect();
        let bigs = vec![(0usize, 0usize, true), (1usize, 3usize, false), (4usize, 4usize, true)];
        let sups = vec![(0usize, 2usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.zd, 110.0); // 大段端点 (终点价), 不是内部低点 108
        assert_eq!(zs.zg, 120.0); // 大段端点 (起点价)
        assert_eq!(zs.start_bar, 10);
        assert_eq!(zs.end_bar, 40);
    }

    #[test]
    fn overlap_pair_zs_endpoints_not_inner() {
        // 双反向段重叠中枢同样只比较大段端点:
        // 向上高级段 U1 D1 U2 D2; D1 内部低点 108 < 端点 110, D2 内部低点 105 < 端点 112 →
        // 重叠 zd = max(110, 112) = 112 (用端点), 内部极值版会得 max(108,105)=108 (错)
        let strokes = vec![
            stroke(true, 100.0, 120.0, 0, 10),   // 线段0: U1
            stroke(false, 120.0, 108.0, 10, 20),  // 线段1: D1 内部 (DN)
            stroke(true, 108.0, 115.0, 20, 30),   // 线段2: D1 内部 (UP)
            stroke(false, 115.0, 110.0, 30, 40),  // 线段3: D1 终点 110
            stroke(true, 110.0, 125.0, 40, 50),   // 线段4: U2
            stroke(false, 125.0, 105.0, 50, 60),  // 线段5: D2 内部 (DN)
            stroke(true, 105.0, 118.0, 60, 70),   // 线段6: D2 内部 (UP)
            stroke(false, 118.0, 112.0, 70, 80),  // 线段7: D2 终点 112
        ];
        let segs: Vec<(usize, usize, bool)> = (0..8).map(|i| (i, i, strokes[i].is_up)).collect();
        let bigs = vec![
            (0usize, 0usize, true),
            (1usize, 3usize, false),
            (4usize, 4usize, true),
            (5usize, 7usize, false),
        ];
        let sups = vec![(0usize, 3usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 1);
        let zs = &out[0];
        assert_eq!(zs.zd, 112.0); // max(110, 112) 端点重叠
        assert_eq!(zs.zg, 120.0); // min(120, 125)
    }

    #[test]
    fn third_buy_after_leave_and_pullback() {
        // 向下高级段: 中枢 [U1,U2]=[105,115] (end_bar=40);
        // D3 回试不破上沿 (118→116, low=116>115) → 延伸停止;
        // U3 向上离开 (116→130), D4 再回试 (130→120, 终点bar70 最低价 120>115) → 三买
        let (strokes, segs, bigs) = ladder(7, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 116.0, 40, 50), // D3: 回试不破上沿, 延伸停止
            stroke(true, 116.0, 130.0, 50, 60),  // U3: 向上离开
            stroke(false, 130.0, 120.0, 60, 70), // D4: 回试, 终点bar70
        ]);
        let sups = vec![(0usize, 6usize, false)];
        let zhongshus = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(zhongshus.len(), 1);
        assert_eq!(zhongshus[0].end_bar, 40); // D3 未参与中枢
        let mut lows = vec![0.0; 71];
        lows[70] = 120.0; // D4 终点bar最低价 > ZG=115
        let highs = vec![999.0; 71]; // 默认高价: 不误触发三卖
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].bar_index, 70);
        assert_eq!(marks[0].price, 120.0);
        assert!(marks[0].is_buy);
    }

    #[test]
    fn up_sup_third_buy_symmetric() {
        // 向上高级段: 中枢 [D1,D2]=[108,118] (end_bar=40);
        // U3 从中枢下沿下方涨起 (与中枢重叠, 异向不推进终点);
        // D3 回试 (126→120, 终点bar60 最低价 120>118) → 三买
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(true, 100.0, 130.0, 0, 10),
            stroke(false, 130.0, 108.0, 10, 20),
            stroke(true, 108.0, 118.0, 20, 30),
            stroke(false, 118.0, 102.0, 30, 40),
            stroke(true, 102.0, 126.0, 40, 50),  // U3: 与中枢重叠, 异向不推进
            stroke(false, 126.0, 120.0, 50, 60), // D3: 回试 low=120>118 → 延伸停止
        ]);
        let sups = vec![(0usize, 5usize, true)];
        let zhongshus = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(zhongshus.len(), 1);
        assert_eq!(zhongshus[0].end_bar, 40);
        let mut lows = vec![0.0; 61];
        lows[60] = 120.0; // D3 终点bar最低价 > ZG=118
        let highs = vec![999.0; 61]; // 默认高价: 不误触发三卖
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].bar_index, 60);
        assert_eq!(marks[0].price, 120.0);
        assert!(marks[0].is_buy);
    }

    /// 回归: 离开段是高级段最后大段, 回试段是其后游离大段
    /// (不属于任何高级段) → 仍判定这一对. 且回试大段内部存在跌破 ZG 的向下线段
    /// (线段5 终点 158.4 < ZG 159.388) — 大段中枢只看大段终点价 (bar60 最低价
    /// 160.052 > ZG), 不看内部线段 → 三买成立 (层级正确).
    #[test]
    fn third_buy_retest_bigseg_outside_sup() {
        let strokes = vec![
            stroke(true, 158.0, 159.0, 0, 10),   // 大段0
            stroke(false, 159.0, 158.5, 10, 20), // 大段1
            stroke(true, 158.5, 159.2, 20, 30),  // 大段2
            stroke(false, 159.2, 158.8, 30, 40), // 大段3 (中枢右边界=40)
            stroke(true, 158.8, 160.1, 40, 50),  // 大段4 离开段(up)
            stroke(false, 160.1, 158.4, 50, 55), // 线段5: 内部跌破 ZG 159.388
            stroke(true, 158.4, 160.052, 55, 60),// 线段6: 大段5终点 bar 60
        ];
        let segs: Vec<(usize, usize, bool)> = (0..7).map(|i| (i, i, strokes[i].is_up)).collect();
        let mut bigs: Vec<(usize, usize, bool)> = (0..5).map(|i| (i, i, strokes[i].is_up)).collect();
        bigs.push((5, 6, false)); // 大段5 = (线段5,线段6) down, 游离于高级段外
        let sups = vec![(0usize, 4usize, true)]; // 高级段 sup#0 大段[0..4]
        let zhongshus = vec![BigSegmentZhongShu {
            sup_idx: 0, start_bar: 10, end_bar: 40,
            zg: 159.388, zd: 159.059, gg: 159.5, dd: 158.5,
        }];
        let mut lows = vec![159.0; 61];
        lows[60] = 160.052; // 回试大段终点bar 60 最低价 > ZG
        let highs = vec![999.0; 61];
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert_eq!(marks.len(), 1, "{:?}", marks);
        assert_eq!(marks[0].bar_index, 60);
        assert_eq!(marks[0].price, 160.052);
        assert!(marks[0].is_buy);
    }

    /// 卖点镜像: 离开段是高级段最后大段(down), 回试段是其后游离向上大段 → 三卖.
    #[test]
    fn third_sell_retest_bigseg_outside_sup() {
        let strokes = vec![
            stroke(false, 161.0, 160.0, 0, 10), // 大段0
            stroke(true, 160.0, 160.5, 10, 20),  // 大段1
            stroke(false, 160.5, 160.2, 20, 30), // 大段2
            stroke(true, 160.2, 160.6, 30, 40),  // 大段3 (中枢右边界=40)
            stroke(false, 160.6, 160.3, 40, 50), // 大段4 离开段(down)
            stroke(true, 160.3, 160.7, 50, 60),  // 大段5 回试段(up) 游离, 终点bar60
        ];
        let segs: Vec<(usize, usize, bool)> = (0..6).map(|i| (i, i, strokes[i].is_up)).collect();
        let bigs: Vec<(usize, usize, bool)> = (0..6).map(|i| (i, i, strokes[i].is_up)).collect();
        let sups = vec![(0usize, 4usize, false)];
        let zhongshus = vec![BigSegmentZhongShu {
            sup_idx: 0, start_bar: 10, end_bar: 40,
            zg: 160.6, zd: 160.3, gg: 160.7, dd: 160.0,
        }];
        let mut highs = vec![160.5; 61];
        highs[60] = 160.25; // 回试大段终点bar60 最高价 < ZD 160.3
        let lows = vec![0.0; 61];
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert_eq!(marks.len(), 1, "{:?}", marks);
        assert_eq!(marks[0].bar_index, 60);
        assert_eq!(marks[0].price, 160.25);
        assert!(!marks[0].is_buy);
    }

    /// 边界: 离开段是全局最后大段 (其后无任何大段) → 无回试段 → 不标记 (维持原语义).
    #[test]
    fn third_no_retest_without_next_bigseg() {
        let (strokes, segs, bigs) = ladder(5, vec![
            stroke(true, 158.0, 159.0, 0, 10),
            stroke(false, 159.0, 158.5, 10, 20),
            stroke(true, 158.5, 159.2, 20, 30),
            stroke(false, 159.2, 158.8, 30, 40),
            stroke(true, 158.8, 160.1, 40, 50), // 大段4 = 全局最后大段
        ]);
        let sups = vec![(0usize, 4usize, true)];
        let zhongshus = vec![BigSegmentZhongShu {
            sup_idx: 0, start_bar: 10, end_bar: 40,
            zg: 159.388, zd: 159.059, gg: 159.5, dd: 158.5,
        }];
        let lows = vec![159.0; 51];
        let highs = vec![999.0; 51];
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert!(marks.is_empty(), "{:?}", marks);
    }

    /// 卖点 (原有): 向下高级段离开后弱反弹 → 三卖 (离开段非最后大段的常规场景).
    #[test]
    fn third_sell_after_down_leave_and_weak_rebound() {
        // 向下高级段: 中枢 [U1,U2]=[105,115];
        // D3 跌破下沿 (118→95, 区间仍与中枢重叠 → 延伸继续);
        // U3 重叠推进终点 (end_bar=60); D4 续跌 (110→90);
        // U4 弱反弹 (90→100, 终点bar80 最高价 100<105) → 三卖
        let (strokes, segs, bigs) = ladder(8, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 95.0, 40, 50),  // D3: 跌破下沿但区间重叠
            stroke(true, 95.0, 110.0, 50, 60),   // U3: 重叠, 同向推进终点
            stroke(false, 110.0, 90.0, 60, 70),  // D4: 续跌, 与中枢重叠 (异向不推进)
            stroke(true, 90.0, 100.0, 70, 80),   // U4: 弱反弹 high=100<105 → 延伸停止
        ]);
        let sups = vec![(0usize, 7usize, false)];
        let zhongshus = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(zhongshus.len(), 1);
        assert_eq!(zhongshus[0].end_bar, 60); // U3 推进终点
        let mut highs = vec![999.0; 81]; // 默认高价, 仅 U4 终点bar 设为 100
        let lows = vec![0.0; 81];
        highs[80] = 100.0;
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].bar_index, 80);
        assert_eq!(marks[0].price, 100.0);
        assert!(!marks[0].is_buy);
    }

    #[test]
    fn no_third_buy_when_pullback_enters_zhongshu() {
        // D4 回试终点bar70 最低价 114 <= ZG=115 → 跌回中枢 → 无三买
        let (strokes, segs, bigs) = ladder(7, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 116.0, 40, 50),
            stroke(true, 116.0, 130.0, 50, 60),  // U3: 向上离开
            stroke(false, 130.0, 114.0, 60, 70), // D4: 回试跌回中枢
        ]);
        let sups = vec![(0usize, 6usize, false)];
        let zhongshus = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        let lows = vec![114.0; 71]; // 终点bar最低价 114 <= ZG=115
        let highs = vec![999.0; 71];
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert!(marks.is_empty());
    }

    #[test]
    fn only_first_third_mark_per_zhongshu() {
        // U3 离开 → D4 回试 (120>115) 三买成立即停止;
        // 后续 U4/D5 即使也满足三买条件也不再检查
        let (strokes, segs, bigs) = ladder(9, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 116.0, 40, 50),
            stroke(true, 116.0, 130.0, 50, 60),  // U3: 向上离开
            stroke(false, 130.0, 120.0, 60, 70), // D4: 回试 → 三买 (bar70)
            stroke(true, 120.0, 140.0, 70, 80),  // U4
            stroke(false, 140.0, 130.0, 80, 90), // D5: 也满足三买, 但已停止
        ]);
        let sups = vec![(0usize, 8usize, false)];
        let zhongshus = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        let mut lows = vec![0.0; 91];
        lows[70] = 120.0;
        lows[90] = 130.0;
        let highs = vec![999.0; 91]; // 默认高价: 不误触发三卖
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].bar_index, 70);
        assert!(marks[0].is_buy);
    }

    #[test]
    fn no_third_mark_cross_sup_boundary() {
        // 高级段0 (含中枢) 之后仅剩 D3 一段, 无法成对 → 无标记;
        // 不跨到高级段1 (U1 起点 bar60, 即使其终点低价 120 也不检查)
        let (strokes, segs, bigs) = ladder(6, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(false, 118.0, 116.0, 40, 50), // D3: 高级段0 最后一段
            stroke(true, 116.0, 130.0, 50, 60),  // 高级段1: U1
        ]);
        let sups = vec![(0usize, 4usize, false), (5usize, 5usize, true)];
        let zhongshus = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(zhongshus.len(), 1);
        let lows = vec![120.0; 61];
        let highs = vec![999.0; 61];
        let marks = detect_bigseg_third_marks(&sups, &bigs, &segs, &strokes, &zhongshus, &highs, &lows);
        assert!(marks.is_empty());
    }

    #[test]
    fn no_cross_sup_boundary() {
        // 高级段1 (向下, 含中枢) + 高级段2 (向上, 对称含中枢): 各出 1 个, 互不越界
        let (strokes, segs, bigs) = ladder(8, vec![
            stroke(false, 120.0, 100.0, 0, 10),
            stroke(true, 100.0, 115.0, 10, 20),
            stroke(false, 115.0, 105.0, 20, 30),
            stroke(true, 105.0, 118.0, 30, 40),
            stroke(true, 90.0, 110.0, 40, 50),
            stroke(false, 110.0, 98.0, 50, 60),
            stroke(true, 98.0, 108.0, 60, 70),
            stroke(false, 108.0, 94.0, 70, 80),
        ]);
        let sups = vec![(0usize, 3usize, false), (4usize, 7usize, true)];
        let out = detect_bigseg_zhongshus(&sups, &bigs, &segs, &strokes);
        assert_eq!(out.len(), 2);
        // 中枢1: 属于高级段0, 右边界止于 bigs[3] (bar 40), 不越界到高级段2
        assert_eq!(out[0].sup_idx, 0);
        assert_eq!(out[0].start_bar, 10);
        assert_eq!(out[0].end_bar, 40);
        assert_eq!(out[0].zd, 105.0);
        assert_eq!(out[0].zg, 115.0);
        // 中枢2: 属于高级段1, 判定对 = bigs[5] (D1) 与 bigs[7] (D2)
        assert_eq!(out[1].sup_idx, 1);
        assert_eq!(out[1].start_bar, 50);
        assert_eq!(out[1].end_bar, 80);
        assert_eq!(out[1].zd, 98.0);
        assert_eq!(out[1].zg, 108.0);
    }
}
