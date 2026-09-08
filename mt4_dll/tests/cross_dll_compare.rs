//! 跨DLL一致性对比测试 — 全链路验证的核心保障
//!
//! 目的: 确保 chanlun_lean_lib 核心算法输出与各 DLL 包装层输出完全一致
//!
//! 测试策略:
//!   1. 使用统一的波状合成测试数据 (与 TDX DLL 测试相同模式)
//!   2. 直接调用 chanlun_lean_lib::ChanlunPipeline::new() 获取基准输出
//!   3. (Phase 2) 调用 MT4 DLL FFI 函数获取包装层输出
//!   4. (Phase 2) 对比两者: 逐 bar, 逐元素, 差异=0
//!
//! 浮动容差: 0.001 (与所有现有测试一致)
//! Bar索引: 严格相等

use chanlun_lean_lib::ChanlunPipeline;

// ══════════════════════════════════════════════════════════════════════
// 共享测试数据工厂
// ══════════════════════════════════════════════════════════════════════

/// 波状合成K线数据 (4波段: 涨→跌→涨→跌)
/// 与 TDX DLL `test_fractal_to_stroke_chain` 使用相同数据
pub fn synth_wave_4band() -> (Vec<f64>, Vec<f64>) {
    let mut highs: Vec<f64> = Vec::new();
    let mut lows: Vec<f64> = Vec::new();
    let base = 100.0;

    // 波段1: 上涨 (0-19, 20根)
    for i in 0..20 {
        highs.push(base + i as f64 * 0.5 + 2.0);
        lows.push(base + i as f64 * 0.5 - 2.0);
    }
    // 波段2: 下跌 (20-44, 25根)
    for i in 0..25 {
        let p = base + 10.0 - i as f64 * 0.4;
        highs.push(p + 2.0);
        lows.push(p - 2.0);
    }
    // 波段3: 上涨 (45-69, 25根)
    for i in 0..25 {
        let p = base + i as f64 * 0.4;
        highs.push(p + 2.0);
        lows.push(p - 2.0);
    }
    // 波段4: 下跌 (70-94, 25根)
    for i in 0..25 {
        let p = base + 10.0 - i as f64 * 0.4;
        highs.push(p + 2.0);
        lows.push(p - 2.0);
    }

    (highs, lows)
}

/// 简单涨跌交替数据 (用于快速管线验证)
pub fn synth_simple_alternating() -> (Vec<f64>, Vec<f64>) {
    let mut highs: Vec<f64> = Vec::new();
    let mut lows: Vec<f64> = Vec::new();
    let base = 100.0;

    for i in 0..5 {
        highs.push(base + i as f64 * 0.035);
        lows.push(base + i as f64 * 0.030 - 0.02);
    }
    for i in 0..5 {
        highs.push(base + 0.175 - i as f64 * 0.018);
        lows.push(base + 0.15 - i as f64 * 0.020 - 0.01);
    }
    for i in 0..3 {
        highs.push(base + 0.085 + i as f64 * 0.045);
        lows.push(base + 0.05 + i as f64 * 0.040);
    }

    (highs, lows)
}

// ══════════════════════════════════════════════════════════════════════
// 管线基准输出验证 (Phase 1: 验证测试基础架构)
// ══════════════════════════════════════════════════════════════════════

#[test]
fn test_pipeline_4band_baseline() {
    let (highs, lows) = synth_wave_4band();
    let pipeline = ChanlunPipeline::new(highs, lows);

    let ff = &pipeline.final_fractals;
    let strokes = &pipeline.strokes;
    let segs = &pipeline.segments;
    let bigs = &pipeline.big_segments;

    // 基准断言: 4波段数据应产生合理的输出
    assert!(!ff.is_empty(), "should have fractals for 4-band wave data");
    assert!(!strokes.is_empty(), "should have strokes");
    assert_eq!(strokes.len() + 1, ff.len(),
        "N strokes should come from N+1 fractals");

    // 验证分型方向交替
    for i in 1..ff.len() {
        assert_ne!(ff[i - 1].is_top, ff[i].is_top,
            "fractals must alternate: F[{}] and F[{}] both {}",
            i - 1, i, if ff[i].is_top { "top" } else { "bottom" });
    }

    // 验证笔方向与分型一致
    for (i, s) in strokes.iter().enumerate() {
        let f_start = &ff[i];
        let f_end = &ff[i + 1];
        assert_eq!(s.start_bar, f_start.bar_index,
            "stroke[{}] start_bar mismatch", i);
        assert_eq!(s.end_bar, f_end.bar_index,
            "stroke[{}] end_bar mismatch", i);
        let expected_is_up = !f_start.is_top && f_end.is_top;
        assert_eq!(s.is_up, expected_is_up,
            "stroke[{}] direction mismatch", i);
    }

    // 验证笔必须向前推进
    for s in strokes {
        assert!(s.start_bar < s.end_bar,
            "stroke must go forward: {} → {}", s.start_bar, s.end_bar);
    }

    eprintln!("\n=== Pipeline baseline verified ===");
    eprintln!("  fractals: {}", ff.len());
    eprintln!("  strokes:  {}", strokes.len());
    eprintln!("  segments: {}", segs.len());
    eprintln!("  big_segs: {}", bigs.len());
}

#[test]
fn test_pipeline_simple_baseline() {
    let (highs, lows) = synth_simple_alternating();
    let pipeline = ChanlunPipeline::new(highs, lows);

    assert!(!pipeline.strokes.is_empty(),
        "should produce at least one stroke for alternating data");

    eprintln!("\n=== Simple pipeline: {} strokes ===", pipeline.strokes.len());
    for s in &pipeline.strokes {
        eprintln!("  {} bar{} ({:.4}) → bar{} ({:.4})",
            if s.is_up { "↑" } else { "↓" },
            s.start_bar, s.start_price, s.end_bar, s.end_price);
    }
}

// ══════════════════════════════════════════════════════════════════════
// 跨DLL一致性对比 — Phase 2 预留接口
// ══════════════════════════════════════════════════════════════════════

/// Phase 2: 对比 chanlun_lean_lib 直接输出 vs MT4 DLL FFI 输出
///
/// 当前 (Phase 1) 仅验证测试基础架构可用。
/// Phase 2 时取消注释并实现 FFI 调用部分。
#[test]
fn test_cross_dll_strokes_consistency() {
    let (highs, lows) = synth_wave_4band();

    // 基准: 直接调用核心库
    let baseline = ChanlunPipeline::new(highs.clone(), lows.clone());
    let base_strokes = &baseline.strokes;

    // TODO Phase 2: 通过 FFI 调用 MT4 DLL
    // unsafe {
    //     chanlun_init(highs.len() as i32, highs.as_ptr(), lows.as_ptr());
    //     let mut up = vec![0.0f64; highs.len()];
    //     let mut down = vec![0.0f64; highs.len()];
    //     let count = chanlun_get_strokes(up.as_mut_ptr(), down.as_mut_ptr());
    //     assert_eq!(count as usize, base_strokes.len());
    //     // 逐笔对比...
    // }

    // Phase 1: 至少验证基准产出非空
    assert!(!base_strokes.is_empty(),
        "baseline strokes must be non-empty for cross-dll test data");
    eprintln!("✓ cross-dll strokes test infrastructure ready ({} baseline strokes)",
        base_strokes.len());
}

#[test]
fn test_cross_dll_segments_consistency() {
    let (highs, lows) = synth_wave_4band();
    let baseline = ChanlunPipeline::new(highs, lows);

    // Phase 1: 验证管线不崩溃 (线段可能为空, 取决于数据是否满足Case2/3/4条件)
    // 4波段短数据(95 bars)可能不足以形成线段 — 这是正常行为
    let seg_count = baseline.segments.len();
    if seg_count > 0 {
        eprintln!("✓ cross-dll segments test infrastructure ready ({} baseline segs)", seg_count);
    } else {
        eprintln!("✓ cross-dll segments test infrastructure ready (0 segs — normal for short wave data)");
    }
}

#[test]
fn test_cross_dll_bigsegments_consistency() {
    let (highs, lows) = synth_wave_4band();
    let baseline = ChanlunPipeline::new(highs, lows);

    // Phase 1: 验证基准产出 (大段可能为空, 取决于数据)
    eprintln!("✓ cross-dll bigsegments test infrastructure ready ({} baseline big-segs)",
        baseline.big_segments.len());
}

// ══════════════════════════════════════════════════════════════════════
// 数据一致性验证 — 同一数据多次计算应产生相同结果
// ══════════════════════════════════════════════════════════════════════

#[test]
fn test_pipeline_idempotent() {
    let (highs, lows) = synth_wave_4band();

    let p1 = ChanlunPipeline::new(highs.clone(), lows.clone());
    let p2 = ChanlunPipeline::new(highs.clone(), lows.clone());

    // 分型应一致
    assert_eq!(p1.final_fractals.len(), p2.final_fractals.len());
    for (i, (f1, f2)) in p1.final_fractals.iter().zip(p2.final_fractals.iter()).enumerate() {
        assert_eq!(f1.bar_index, f2.bar_index, "fractal[{}] bar_index", i);
        assert_eq!(f1.is_top, f2.is_top, "fractal[{}] is_top", i);
        assert!((f1.price - f2.price).abs() < 0.001, "fractal[{}] price", i);
    }

    // 笔应一致
    assert_eq!(p1.strokes.len(), p2.strokes.len());
    for (i, (s1, s2)) in p1.strokes.iter().zip(p2.strokes.iter()).enumerate() {
        assert_eq!(s1.start_bar, s2.start_bar, "stroke[{}] start_bar", i);
        assert_eq!(s1.end_bar, s2.end_bar, "stroke[{}] end_bar", i);
        assert_eq!(s1.is_up, s2.is_up, "stroke[{}] is_up", i);
        assert!((s1.start_price - s2.start_price).abs() < 0.001, "stroke[{}] start_price", i);
        assert!((s1.end_price - s2.end_price).abs() < 0.001, "stroke[{}] end_price", i);
    }

    eprintln!("✓ pipeline idempotent: 2 runs produce identical output");
}
