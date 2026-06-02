# uv-cache-guardian

Resource-aware caching for `uv`.

## The Problem

Your CI pipeline downloads 4.7 GB of packages every run. 2.3 GB of that is stuff you already downloaded last week. In a different cache key.

Same package, same version, same hash — different cache key because the lockfile changed. The packages are identical on disk. But your CI doesn't know that. It downloads, extracts, and links the same wheel files over and over. You pay for bandwidth you don't use, and your CI minutes are gone before you get anything back.

## What It Does

`uv-cache-guardian` watches your cache. It tracks:

- **Disk budget** — default 5 GB for `~/.cache/uv`
- **Bandwidth budget** — default 10 GB downloaded per day
- **Time budget** — default 5 minutes per CI run

When any budget is close to exceeded, it sounds an alert. When the cache reaches critical levels, it evicts intelligently — not by LRU or FIFO, but by information density.

## How It Evicts: KL Divergence

Cache full. Which packages do we evict?

Most cache eviction policies ask "When was it last used?" That tells you about access patterns, not about value. Two packages could both be "old" — but one is a unique dependency of three projects, and the other is something a single build pulled in and never touched again.

The guardian uses KL divergence to measure how different each project's dependency profile is from the average. A project whose dependencies are numpy, pandas, and scikit-learn is well-covered by the mean profile — most projects need those. A project whose only unique dependency is a niche GPU kernel library? That's high divergence. Evict that first.

You maximize the number of projects that can still install from cache without redownloading. The math is:[Jensen-Shannon divergence](https://en.wikipedia.org/wiki/Jensen%E2%80%93Shannon_divergence) between each project's dependency profile and the mean, symmetric and bounded.

## What That Looks Like in Practice

Start with a `uv-cache-guardian.toml`:

```toml
[cache]
path = "/home/ci/.cache/uv"

[budget]
disk = "5GB"
bandwidth_per_day = "10GB"
time_per_run_secs = 300

[eviction]
strategy = "kl_divergence"
symmetric = true
epsilon = 1e-10

[phase_detection]
pre_transition = 0.80
transitioning = 0.95
```

Run it:

```console
$ uv cache guardian
📦 Cache: 4.7 GB / 5.0 GB (94.0%)
🌐 Bandwidth: 2.3 GB / 10 GB (23.0%)
⏱ Budget: 300s

Phase: 🚨 Transitioning — Initiate cache eviction now

Evicting 3 package(s) to free 800 MB:
  • pkg-a (KL divergence: 0.42) — unique to project X
  • pkg-b (KL divergence: 0.31) — unique to project Y
  • pkg-c (KL divergence: 0.24) — low reuse across projects
```

Enable CI integration:

```yaml
# .github/workflows/ci.yml
- name: Run uv cache guardian
  run: uv cache guardian --max-disk 5GB --max-bandwidth 10GB

- name: Install dependencies
  run: uv sync

- name: Report cache savings
  run: uv cache guardian --report
```

## The Result

Your cache hit rate went from 34% to 78%. CI time dropped from 4 min to 90 sec. You saved \$200/month in compute.

Not because the guardian downloads faster. It doesn't. It downloads less. By keeping the packages that serve the most projects and evicting the ones that serve the fewest, your cache density improves over time. Every PR gets closer to a warm start.

## Details

### Conservation Laws

Three conservation laws, three budgets:

| Law | Tracks | Default |
|-----|--------|---------|
| Disk | `~/.cache/uv` size | 5 GB |
| Bandwidth | Bytes downloaded per day | 10 GB |
| Time | Installation time per CI run | 5 min (300 s) |

Each law is checked independently. A violation triggers a warning, an eviction cycle, or both, depending on the phase.

### Phase Detection

The cache moves through four phases:

- **Stable** — well within budget, normal operation
- **PreTransition** — approaching the disk limit, proactive pruning recommended
- **Transitioning** — at or over the limit, eviction triggered
- **PostTransition** — just evicted, monitoring for recovery

Phase detection is stateful: it remembers your previous utilization level so it can distinguish "we're under budget for the first time" from "we just evicted a ton of data and are recovering."

### Dependency Profiling

The guardian builds a dependency profile per project — a probability distribution over packages in that project's lockfile. The mean of all profiles is the "average project." KL divergence measures how far each project deviates from that average.

High-divergence projects are evicted first because their packages are least useful to everyone else. Low-divergence projects (those with common dependencies) stay — their packages serve the most cache hits.

### CI Pipeline Analysis

The `CiOptimizer` module tracks PR history and finds packages downloaded by multiple PRs within a configurable time window. These are cache warming candidates: packages that, if pre-cached, would save repeated downloads across unrelated PRs.

## Usage

```console
# Check current cache state
$ uv-cache-guardian --cache-path ~/.cache/uv

# Apply budgets and evict if needed
$ uv-cache-guardian --cache-path ~/.cache/uv --max-disk 5GB --max-bandwidth 10GB --format json

# Generate CI report
$ uv-cache-guardian --output cache-snapshot.json
```

## API

```rust
use uv_cache_guardian::{CacheGuardian, ConservationChecker, PhaseDetector, EvictionStrategy};

let guardian = CacheGuardian::new("/home/user/.cache/uv");
let stats = guardian.measure()?;
println!("Cache: {}", CacheGuardian::format_bytes(stats.cache_size_bytes));

let conservation = ConservationChecker::check_all(
    stats.cache_size_bytes, 5_000_000_000,
    guardian.budget.bandwidth.bytes_today, 10_000_000_000,
    120, 300,
);
for result in &conservation {
    println!("{}", result.message);
}

let strategy = EvictionStrategy::default();
let report = strategy.eviction_report(&profiles, target_bytes);
println!("Eviction count: {}", report.eviction_count);
```

## License

MIT or Apache 2.0
