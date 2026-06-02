/// Example: Basic monitoring with uv-cache-guardian.
///
/// Run:
/// ```sh
/// cargo run --example basic_monitoring
/// ```
use std::path::PathBuf;

use uv_cache_guardian::{
    CacheGuardian, ConservationChecker, PhaseDetector,
};

fn main() {
    // Use the current uv cache path
    let cache_path = PathBuf::from(
        std::env::var("UV_CACHE_DIR").unwrap_or_else(|_| "~/.cache/uv".to_string()),
    );

    let guardian = CacheGuardian::new(&cache_path);
    println!("Cache Guardian — monitoring: {}", cache_path.display());

    // Measure current cache state
    match guardian.measure() {
        Ok(stats) => {
            println!("  Cache size: {}", CacheGuardian::format_bytes(stats.cache_size_bytes));
            println!("  Files: {}", stats.file_count);
            println!("  Package dirs: {}", stats.package_count);

            // Check conservation laws
            let disk_result = ConservationChecker::check_disk(
                stats.cache_size_bytes,
                guardian.budget.disk.max_bytes,
            );
            let bandwidth_result = ConservationChecker::check_bandwidth(
                guardian.budget.bandwidth.bytes_today,
                guardian.budget.bandwidth.max_bytes_per_day,
            );

            println!("\n  Conservation:");
            println!("    {}", disk_result.message);
            println!("    {}", bandwidth_result.message);

            // Phase detection
            let utilization = stats.cache_size_bytes as f64 / guardian.budget.disk.max_bytes as f64;
            let mut detector = PhaseDetector::default();
            let phase_report = detector.report_stateful(utilization, 0.0);
            println!("\n  Phase: {} ({})", phase_report.phase.icon(), phase_report.action);
        }
        Err(e) => {
            eprintln!("Failed to measure cache: {e}");
        }
    }
}
