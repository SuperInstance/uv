//! `uv-cache-guardian` — resource-aware caching for `uv` package management.
//!
//! uv is fast. This makes it smart about resources. Your CI cache budget is $50/month.
//! This keeps it there.
//!
//! The guardian applies three conservation laws:
//!
//! 1. **Disk budget** — default 5 GB for the uv cache directory
//! 2. **Bandwidth budget** — default 10 GB downloaded per day
//! 3. **Time budget** — default 5 minutes per CI run
//!
//! It detects resource phases via the phase detection model:
//! - `PreTransition` → cache growing fast, proactive pruning
//! - `Transitioning` → disk at critical levels, aggressive eviction
//! - `PostTransition` → cache just pruned, recovery monitoring
//!
//! Intelligent eviction uses KL divergence between project dependency profiles
//! to decide which packages to evict first.

pub mod monitor;
pub mod conservation;
pub mod phase;
pub mod eviction;
pub mod ci;
pub mod snapshot;
pub mod cli;

pub use monitor::{CacheGuardian, CacheStats, DiskBudget, BandwidthBudget, TimeBudget, Budget};
pub use conservation::{ConservationLaw, ConservationChecker, ConservationResult};
pub use phase::{PhaseDetector, ResourcePhase};
pub use eviction::{EvictionStrategy, DependencyProfile, ProjectProfile};
pub use ci::{CiOptimizer, PrHistory};
pub use snapshot::CacheSnapshot;
pub use cli::Cli;
