use std::collections::HashMap;
use std::env;
use std::process::Command;

// ---------------------------------------------------------------------------
// 统计函数
// ---------------------------------------------------------------------------

fn compute_stats(samples: &[f64]) -> (f64, f64, f64, f64) {
    let n = samples.len() as f64;
    let avg = samples.iter().sum::<f64>() / n;
    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let variance = samples.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / n;
    (avg, min, max, variance.sqrt())
}

// ---------------------------------------------------------------------------
// 解析一行输出 → 0~2 个 (metric_name, value_ms) 对
//   - 总耗时
//   - 平均每项耗时（如有）
// ---------------------------------------------------------------------------

fn parse_line(raw: &str) -> Vec<(String, f64)> {
    // 测试框架可能在首次 println 前添加 "test xxx ... " 前缀——找到 [sqlite/ 位置
    let pos = match raw.find("[sqlite/") {
        Some(p) => p,
        None => return vec![],
    };
    let line = &raw[pos..];

    // 跳过纯信息行
    if line.contains("seeded") || line.contains("cover 提取") {
        return vec![];
    }

    let mut results = Vec::new();

    // --- 提取总耗时（毫秒）---
    let total_ms = if line.contains("耗时") {
        // "耗时 X.XXs" → seconds → ms
        line.split("耗时")
            .nth(1)
            .and_then(|s| s.trim().split('s').next())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|secs| secs * 1000.0)
    } else if line.contains("ms,") {
        // "XXXms, 平均..." → first integer before "ms,"
        line.split("ms,")
            .next()
            .and_then(|s| s.rsplit(char::is_whitespace).next())
            .and_then(|s| s.parse::<f64>().ok())
    } else {
        None
    };

    let total_ms = match total_ms {
        Some(v) => v,
        None => return vec![],
    };

    // --- 指标名称：[sqlite/...] 之后到 ": " 之前的内容 ---
    let name = if let Some(idx) = line.find(": ") {
        line[..idx].to_string()
    } else {
        line.to_string()
    };

    results.push((name.clone(), total_ms));

    // --- extract per-item average ("平均 X.XXms/单位") ---
    if let Some(after_avg) = line.split("平均 ").nth(1) {
        if let Some(avg_str) = after_avg.split("ms/").next() {
            if let Ok(avg_val) = avg_str.trim().parse::<f64>() {
                results.push((format!("{name} (平均)"), avg_val));
            }
        }
    }

    results
}

// ---------------------------------------------------------------------------
// 运行 cargo test 一次，返回所有解析出的指标
// ---------------------------------------------------------------------------

fn run_stress_tests() -> HashMap<String, f64> {
    let output = Command::new("cargo")
        .args([
            "test",
            "--test",
            "stress_test",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .expect("failed to run cargo test --test stress_test");

    if !output.status.success() {
        eprintln!(
            "cargo test failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!("stress_test failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut metrics = HashMap::new();

    for line in stdout.lines() {
        for (name, value) in parse_line(line) {
            metrics.insert(name, value);
        }
    }

    metrics
}

// ---------------------------------------------------------------------------
// 命令行参数解析
// ---------------------------------------------------------------------------

fn parse_n_runs() -> usize {
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--n" && i + 1 < args.len() {
            if let Ok(n) = args[i + 1].parse() {
                if n > 0 {
                    return n;
                }
            }
        }
        if args[i].starts_with("--n=") {
            if let Ok(n) = args[i][4..].parse() {
                if n > 0 {
                    return n;
                }
            }
        }
    }
    10
}

fn parse_cooldown() -> u64 {
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--cooldown" || args[i] == "-c") && i + 1 < args.len() {
            if let Ok(s) = args[i + 1].parse() {
                if s > 0 {
                    return s;
                }
            }
        }
        if args[i].starts_with("--cooldown=") {
            if let Ok(s) = args[i][11..].parse() {
                if s > 0 {
                    return s;
                }
            }
        }
    }
    10
}

// ---------------------------------------------------------------------------
// 主入口
// ---------------------------------------------------------------------------

fn main() {
    let n_runs = parse_n_runs();
    let cooldown = parse_cooldown();
    eprintln!("Running stress_test {n_runs} times (cooldown {cooldown}s)...\n");

    // 指标名称 → 各次运行的耗时列表
    let mut all_metrics: HashMap<String, Vec<f64>> = HashMap::new();

    for run in 1..=n_runs {
        eprintln!("=== Iteration {run}/{n_runs} ===");
        for (name, value) in run_stress_tests() {
            all_metrics.entry(name).or_default().push(value);
        }
        if run < n_runs {
            eprintln!("  cooling down {cooldown}s...");
            std::thread::sleep(std::time::Duration::from_secs(cooldown));
        }
    }

    // 按分类分组
    let categories: &[&str] = &[
        "[sqlite/write]",
        "[sqlite/read]",
        "[sqlite/search]",
        "[sqlite/relation]",
        "[sqlite/link]",
        "[sqlite/type]",
    ];

    for cat in categories {
        let mut entries: Vec<(&String, &Vec<f64>)> = all_metrics
            .iter()
            .filter(|(name, _)| name.starts_with(*cat))
            .collect();
        if entries.is_empty() {
            continue;
        }
        entries.sort_by(|a, b| a.0.cmp(b.0));

        println!();
        println!("════════════════════════════════════════════════════════════════════════════");
        println!("  {cat}");
        println!("════════════════════════════════════════════════════════════════════════════");
        println!(
            "  {:<60} {:>8} {:>8} {:>8} {:>8} {:>10}",
            "metric", "avg", "min", "max", "σ", "σ/avg%"
        );
        println!(
            "  {:-<60} {:-<8} {:-<8} {:-<8} {:-<8} {:-<10}",
            "", "", "", "", "", ""
        );

        for (name, samples) in &entries {
            let (avg, min, max, stddev) = compute_stats(samples);
            let cv = if avg > 0.0 {
                (stddev / avg) * 100.0
            } else {
                0.0
            };
            let short = name.replacen(*cat, "", 1).trim().to_string();
            println!(
                "  {:<60} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>9.1}%",
                short, avg, min, max, stddev, cv,
            );
        }

        // 打印原始数据
        println!();
        println!("  --- raw data (ms per run) ---");
        for (name, samples) in &entries {
            let short = name.replacen(*cat, "", 1).trim().to_string();
            let vals: Vec<String> = samples.iter().map(|v| format!("{v:.2}")).collect();
            println!("  {:<60} [{}]", short, vals.join(", "));
        }
    }
    println!();
    eprintln!("Done.");
}
