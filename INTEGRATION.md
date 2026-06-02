# 🏆 SuperInstance Enhancement: Cache Guardian

> **uv with intelligent cache budget management. Same uv. Won't fill your disk.**

## Overview

The Cache Guardian is a SuperInstance enhancement to the [uv](https://github.com/astral-sh/uv) package manager that adds resource-aware cache management. It monitors and enforces disk, bandwidth, and time budgets using conservation laws and intelligent eviction strategies.

### What it does

| Feature | Description |
|---------|-------------|
| **Budget enforcement** | Defaults: 5 GB disk, 10 GB/day bandwidth, 5 min CI time |
| **Conservation laws** | Three laws track disk, bandwidth, and time utilization |
| **Phase detection** | Detects cache resource phases (Stable → PreTransition → Transitioning → PostTransition) |
| **KL divergence eviction** | Evicts packages with unique dependency profiles first, preserving shared packages |
| **CI optimization** | Analyzes PR history for cache warming suggestions |
| **Snapshot audit trail** | JSON-serialized snapshots for monitoring and CI integration |

## Usage

```bash
# Check current cache status with conservation law compliance
uv cache guardian status

# Evict packages using KL divergence analysis
uv cache guardian evict --target 1GB
```

## Architecture

The Guardian is implemented as a standalone crate `crates/uv-cache-guardian/` that integrates into uv's existing cache management system. It uses no external state files — all monitoring is ephemeral and based on the current cache state.

### CLI integration

The `uv cache guardian` subcommand is added to the existing `CacheCommand` enum in `crates/uv-cli/src/lib.rs`, alongside `Clean`, `Prune`, `Dir`, and `Size`:

```rust
CacheCommand::Guardian(GuardianArgs)
```

### Conservation Model

Three conservation laws govern cache behavior:

1. **Disk Budget Law** — Total cache size must not exceed `max_disk` (default: 5 GB)
2. **Bandwidth Budget Law** — Daily downloads must not exceed `max_bandwidth` (default: 10 GB/day)
3. **Time Budget Law** — CI install time must not exceed `max_time` (default: 5 min)

Each law reports whether the constraint is satisfied or exceeded, along with utilization percentage.

### Phase Detection

The resource phase model uses two thresholds:
- `pre_transition`: 80% utilization — proactive monitoring
- `transitioning`: 95% utilization — aggressive action needed

Phase transitions are tracked statefully: after transitioning from Transitioning back to stable, the system briefly enters PostTransition for recovery monitoring.

### Eviction Strategy

The eviction engine uses **KL divergence** (and the symmetric Jensen-Shannon divergence) between project dependency profiles to identify packages that are:

- **Unique outliers** (high divergence) — evicted first, low impact on other projects
- **Shared dependencies** (low divergence) — preserved, high cross-project value

## Files Added

| File | Description |
|------|-------------|
| `crates/uv-cache-guardian/Cargo.toml` | Crate manifest |
| `crates/uv-cache-guardian/src/lib.rs` | Public API and re-exports |
| `crates/uv-cache-guardian/src/monitor.rs` | CacheGuardian, budgets, measurement |
| `crates/uv-cache-guardian/src/conservation.rs` | Three conservation laws |
| `crates/uv-cache-guardian/src/phase.rs` | Resource phase detection |
| `crates/uv-cache-guardian/src/eviction.rs` | KL divergence eviction strategy |
| `crates/uv-cache-guardian/src/ci.rs` | CI optimization and cache warming |
| `crates/uv-cache-guardian/src/snapshot.rs` | JSON snapshot/audit trail |
| `crates/uv-cache-guardian/src/cli.rs` | CLI argument parsing |
| `crates/uv/src/commands/cache_guardian.rs` | Guardian command handler |

## Tests

71 tests covering all modules:

```bash
cargo test -p uv-cache-guardian
```

### Test coverage areas
- **Conservation laws**: Budget satisfaction and exceeded states for all three laws
- **Phase detection**: All four phases with custom thresholds and stateful transitions
- **Eviction**: KL divergence computation, profile scoring, eviction selection
- **CI optimization**: PR history, warming suggestions, overlapping downloads
- **Snapshot**: JSON serialization round-trip, file I/O
- **Cache monitoring**: Budget defaults, measurement, bandwidth tracking

## Future Work

- **KL divergence eviction** — Full implementation walks uv's cache directory, builds dependency profiles from installed packages, computes pairwise divergences, and selectively evicts
- **Snapshot accumulation** — Store snapshots over time for trend analysis
- **GitHub Actions integration** — `uv cache guardian status --format json` for CI annotations
- **Preemptive warming** — Auto-warm cache for overlapping dependencies detected across PRs
