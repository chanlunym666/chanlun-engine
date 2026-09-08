//! 笔算法输出对比测试 (C1+C2+C3) — 同一组模拟K线验证分型/笔输出
//! 
//! 用法: cargo test compare -- --nocapture
//! 
//! 用同一组模拟K线数据, 跑两边算法, 对比分型/笔输出

use chanlun_lean_lib::*;

/// 模拟一段趋势+震荡的K线数据
fn test_ohlc() -> (Vec<f64>, Vec<f64>) {
    let highs = vec![
        10.0, 12.0, 11.0, 13.0, 12.5, 14.0, 15.0, 14.5, 16.0, 15.5,
        17.0, 16.0, 18.0, 17.5, 19.0, 18.0, 20.0, 19.0, 21.0, 20.5,
        20.0, 19.0, 18.5, 19.5, 18.0, 17.0, 16.5, 17.5, 16.0, 15.0,
        15.5, 16.5, 15.0, 14.0, 14.5, 15.5, 16.0, 15.0, 14.0, 13.5,
        14.0, 15.0, 16.0, 15.5, 14.0, 13.0, 14.5, 15.5, 14.0, 13.0,
    ];
    let lows = vec![
        8.0,  9.0,  7.0,  10.0, 8.5,  11.0, 12.0, 11.5, 13.0, 12.0,
        14.0, 13.0, 15.0, 14.0, 16.0, 15.0, 17.0, 16.0, 18.0, 17.5,
        17.0, 16.0, 15.5, 16.5, 15.0, 14.0, 13.5, 14.5, 13.0, 12.0,
        12.5, 13.5, 12.0, 11.0, 11.5, 12.5, 13.0, 12.0, 11.0, 10.5,
        11.0, 12.0, 13.0, 12.5, 11.0, 10.0, 11.5, 12.5, 11.0, 10.0,
    ];
    (highs, lows)
}

#[test]
fn compare_fractal_output() {
    let (highs, lows) = test_ohlc();

    let merged = process_merged_candles(&highs, &lows);
    println!("\n=== 缠K合并: {} → {} 根缠K ===", highs.len(), merged.len());
    for (i, m) in merged.iter().enumerate() {
        println!("  M[{}] h={:.2} l={:.2} hbi={} lbi={}",
            i, m.high, m.low, m.high_bar_index, m.low_bar_index);
    }

    let points = identify_fractals(&merged);
    println!("\n=== 分型识别: {} 个 ===", points.len());
    for fp in &points {
        println!("  {} bar={} price={:.2} merged={}",
            if fp.is_top { "▲ TOP" } else { "▼ BOT" },
            fp.bar_index, fp.price, fp.merged_index);
    }

    let valid = filter_fractals(&points);
    println!("\n=== 分型过滤后: {} 个 ===", valid.len());
    for fp in &valid {
        println!("  {} bar={} price={:.2} merged={}",
            if fp.is_top { "▲ TOP" } else { "▼ BOT" },
            fp.bar_index, fp.price, fp.merged_index);
    }

    let ff = process_strokes_fractals(&valid, true, 4);
    println!("\n=== 笔处理结果 (C1+C2+C3 Time-Race): {} 个终点分型 ===", ff.len());
    for (i, fp) in ff.iter().enumerate() {
        println!("  F[{}] {} bar={} price={:.2}",
            i,
            if fp.is_top { "▲ TOP" } else { "▼ BOT" },
            fp.bar_index, fp.price);
    }

    let strokes = build_strokes(&ff);
    println!("\n=== 构建笔: {} 笔 ===", strokes.len());
    for (i, s) in strokes.iter().enumerate() {
        println!("  Stroke[{}] {} bar[{}→{}] price[{:.2}→{:.2}]",
            i,
            if s.is_up { "↑ UP" } else { "↓ DN" },
            s.start_bar, s.end_bar, s.start_price, s.end_price);
    }

    let segs = process_segments(&strokes);
    println!("\n=== 线段: {} 段 ===", segs.len());
    for (i, (si, ei, is_up)) in segs.iter().enumerate() {
        println!("  Seg[{}] {} stroke[{}→{}] bar[{}→{}]",
            i,
            if *is_up { "↑ UP" } else { "↓ DN" },
            si, ei,
            strokes[*si].start_bar, strokes[*ei].end_bar);
    }

    // 基本断言
    assert!(merged.len() > 0, "应有缠K输出");
    assert!(valid.len() > 0, "应有有效分型");
    assert!(ff.len() > 0, "应有笔端点");
    assert!(strokes.len() > 0, "应有笔");
    println!("\n=== ✅ 算法管线全通, 未崩 ===");
}

#[test]
fn test_pipeline_stats() {
    let (highs, lows) = test_ohlc();
    let pipeline = ChanlunPipeline::new(highs, lows);
    let stats = pipeline.stats();

    println!("\n=== 一键管线统计 ===");
    println!("  原始K线: {}", stats.bars);
    println!("  缠K合并: {}", stats.merged);
    println!("  分型(初): {}", stats.fractals);
    println!("  分型(终): {}", stats.final_fractals);
    println!("  笔: {}", stats.strokes);
    println!("  线段: {}", stats.segments);
    println!("  大段: {}", stats.big_segments);

    assert!(stats.bars == 50);
    assert!(stats.merged >= 1);
    assert!(stats.strokes >= 1);
}
