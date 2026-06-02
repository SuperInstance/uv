/// Example: CI optimization with cache warming suggestions.
///
/// Run:
/// ```sh
/// cargo run --example ci_optimizer
/// ```
use chrono::Utc;
use uv_cache_guardian::{
    CiOptimizer, PrHistory, PrRecord,
};

fn main() {
    // Simulate CI history
    let mut history = PrHistory::new();

    let now = Utc::now();

    history.add(PrRecord {
        pr_id: "PR-42".to_string(),
        timestamp: now,
        packages_downloaded: vec![
            "numpy".to_string(),
            "pandas".to_string(),
            "scikit-learn".to_string(),
            "matplotlib".to_string(),
        ],
        total_bytes: 50_000_000,
        duration_seconds: 120,
        cache_warmed: false,
    });

    history.add(PrRecord {
        pr_id: "PR-43".to_string(),
        timestamp: now,
        packages_downloaded: vec![
            "numpy".to_string(),
            "torch".to_string(),
            "transformers".to_string(),
        ],
        total_bytes: 800_000_000,
        duration_seconds: 300,
        cache_warmed: false,
    });

    history.add(PrRecord {
        pr_id: "PR-44".to_string(),
        timestamp: now,
        packages_downloaded: vec![
            "numpy".to_string(),
            "pandas".to_string(),
            "requests".to_string(),
        ],
        total_bytes: 45_000_000,
        duration_seconds: 110,
        cache_warmed: false,
    });

    let optimizer = CiOptimizer::new(history);

    println!("=== CI Optimization Report ===\n");

    let top = optimizer.top_packages(5);
    println!("Top downloaded packages:");
    for (pkg, count) in &top {
        println!("  {pkg}: {count} PR(s)");
    }

    let overlapping = optimizer.overlapping_downloads();
    println!("\nPackages downloaded by multiple PRs (cache warming candidates):");
    for pkg in &overlapping {
        println!("  {pkg}");
    }

    println!("\nCache warming suggestions:");
    let suggestions = optimizer.warming_suggestions();
    for suggestion in &suggestions {
        println!(
            "  {} — estimated savings: {}",
            suggestion.suggestion,
            uv_cache_guardian::CacheGuardian::format_bytes(suggestion.estimated_savings_bytes),
        );
    }
}
