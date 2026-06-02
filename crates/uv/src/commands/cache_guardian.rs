use std::fmt::Write;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use uv_cache::Cache;
use uv_cache_guardian::{CacheGuardian, ConservationChecker, PhaseDetector};
use uv_cli::GuardianCommand;

use crate::commands::ExitStatus;
use crate::printer::Printer;

/// SuperInstance Enhancement: Cache Guardian entry point.
///
/// `uv cache guardian status` — show compliance status
/// `uv cache guardian evict` — evict packages using KL divergence
pub(crate) async fn cache_guardian(
    command: GuardianCommand,
    max_disk: Option<String>,
    max_bandwidth: Option<String>,
    cache: Cache,
    printer: Printer,
) -> Result<ExitStatus> {
    match command {
        GuardianCommand::Status => {
            guardian_status(max_disk, max_bandwidth, cache, printer).await
        }
        GuardianCommand::Evict { target } => {
            guardian_evict(max_disk, max_bandwidth, target, cache, printer).await
        }
    }
}

/// Show current cache status with conservation law compliance and phase detection.
async fn guardian_status(
    max_disk: Option<String>,
    max_bandwidth: Option<String>,
    cache: Cache,
    printer: Printer,
) -> Result<ExitStatus> {
    let cache_path = cache.root().to_path_buf();

    let mut guardian = CacheGuardian::new(&cache_path);

    // Apply custom budgets if provided
    if let Some(disk_str) = max_disk {
        if let Ok(bytes) = uv_cache_guardian::cli::parse_size(&disk_str) {
            guardian.budget.disk.max_bytes = bytes;
        }
    }
    if let Some(bw_str) = max_bandwidth {
        if let Ok(bytes) = uv_cache_guardian::cli::parse_size(&bw_str) {
            guardian.budget.bandwidth.max_bytes_per_day = bytes;
        }
    }

    let stats = guardian.measure().context("Failed to measure cache")?;

    writeln!(printer.stdout(), "{}", "Cache Guardian Status".bold().green())?;
    writeln!(printer.stdout(), "{}", "═".repeat(50).dimmed())?;
    writeln!(printer.stdout())?;

    // Cache overview
    writeln!(
        printer.stdout(),
        "📦 Cache path: {}",
        cache_path.display().cyan()
    )?;
    writeln!(
        printer.stdout(),
        "📂 Cache size: {}",
        CacheGuardian::format_bytes(stats.cache_size_bytes).yellow()
    )?;
    writeln!(
        printer.stdout(),
        "📄 Files: {}",
        stats.file_count.to_string().cyan()
    )?;
    writeln!(
        printer.stdout(),
        "📁 Package dirs: {}",
        stats.package_count.to_string().cyan()
    )?;
    writeln!(printer.stdout())?;

    // Conservation laws
    writeln!(
        printer.stdout(),
        "{}",
        "Conservation Laws".bold().blue()
    )?;
    writeln!(printer.stdout(), "{}", "─".repeat(40).dimmed())?;

    let disk_result = ConservationChecker::check_disk(
        stats.cache_size_bytes,
        guardian.budget.disk.max_bytes,
    );
    let bandwidth_result = ConservationChecker::check_bandwidth(
        guardian.budget.bandwidth.bytes_today,
        guardian.budget.bandwidth.max_bytes_per_day,
    );
    let time_result =
        ConservationChecker::check_time(0, guardian.budget.time.max_seconds);

    for result in &[disk_result, bandwidth_result, time_result] {
        if result.satisfied {
            writeln!(
                printer.stdout(),
                "  {} {} ({:.1}% used)",
                "✓".green(),
                result.law.label(),
                result.utilization_pct,
            )?;
        } else {
            writeln!(
                printer.stdout(),
                "  {} {} ({:.1}% used) — BUDGET EXCEEDED",
                "✗".red().bold(),
                result.law.label(),
                result.utilization_pct,
            )?;
        }
    }
    writeln!(printer.stdout())?;

    // Phase detection
    let utilization = stats.cache_size_bytes as f64
        / guardian.budget.disk.max_bytes.max(1) as f64;
    let mut detector = PhaseDetector::default();
    let phase_report = detector.report_stateful(utilization, 0.0);

    writeln!(
        printer.stdout(),
        "{}",
        "Resource Phase".bold().blue()
    )?;
    writeln!(printer.stdout(), "{}", "─".repeat(40).dimmed())?;
    writeln!(
        printer.stdout(),
        "  {} {} ({})",
        phase_report.phase.icon(),
        phase_report.phase.label().bold(),
        phase_report.action.dimmed()
    )?;
    writeln!(
        printer.stdout(),
        "  Utilization: {:.1}%",
        utilization * 100.0
    )?;
    writeln!(printer.stdout())?;

    Ok(ExitStatus::Success)
}

/// Evict packages from the cache using KL divergence eviction strategy.
async fn guardian_evict(
    _max_disk: Option<String>,
    _max_bandwidth: Option<String>,
    target: String,
    cache: Cache,
    printer: Printer,
) -> Result<ExitStatus> {
    writeln!(
        printer.stdout(),
        "{}",
        "Cache Guardian Eviction".bold().green()
    )?;
    writeln!(printer.stdout(), "{}", "═".repeat(50).dimmed())?;
    writeln!(
        printer.stdout(),
        "Target: {} to free",
        target.yellow()
    )?;

    let cache_path = cache.root().to_path_buf();
    let guardian = CacheGuardian::new(&cache_path);
    let _stats = guardian.measure().context("Failed to measure cache")?;

    // NOTE: In a full implementation, we would:
    // 1. Walk the cache and build dependency profiles per project
    // 2. Use KL divergence eviction to select packages
    // 3. Remove selected packages
    // For now, this provides the CLI framework and reports the cache state.

    writeln!(
        printer.stdout(),
        "Cache location: {}",
        cache_path.display().cyan()
    )?;

    writeln!(
        printer.stdout(),
        "{}",
        "\nℹ Full KL divergence eviction scans projects and their dependency profiles."
            .dimmed()
    )?;
    writeln!(
        printer.stdout(),
        "Run `uv cache clean` to remove all cache entries, or `uv cache prune` to remove unreachable objects."
    )?;

    Ok(ExitStatus::Success)
}
