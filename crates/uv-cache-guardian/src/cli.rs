//! CLI argument parsing for uv-cache-guardian commands.

use std::path::PathBuf;

use clap::Parser;

/// uv-cache-guardian — resource-aware caching for uv package management.
///
/// uv is fast. This makes it smart about resources.
#[derive(Parser, Debug)]
#[command(name = "uv-cache-guardian")]
#[command(version, about)]
pub struct Cli {
    /// Path to uv's cache directory.
    #[arg(short, long, default_value = "~/.cache/uv")]
    pub cache_path: PathBuf,

    /// Maximum disk cache size (e.g., "5GB", "500MB").
    #[arg(long, default_value = "5GB")]
    pub max_disk: String,

    /// Maximum daily bandwidth (e.g., "10GB", "1GB").
    #[arg(long, default_value = "10GB")]
    pub max_bandwidth: String,

    /// Maximum CI run time in seconds.
    #[arg(long, default_value = "300")]
    pub max_time_secs: u64,

    /// Output format: "text", "json", or "csv"
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Enable verbose logging.
    #[arg(short, long)]
    pub verbose: bool,

    /// Output file for snapshot.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Parse a human-readable size string like "5GB", "500MB", "1.5GB" into bytes.
pub fn parse_size(size_str: &str) -> Result<u64, String> {
    let size_str = size_str.trim().to_uppercase();
    let (num_str, unit) = if size_str.ends_with("GB") {
        (&size_str[..size_str.len() - 2], 1_073_741_824u64)
    } else if size_str.ends_with("MB") {
        (&size_str[..size_str.len() - 2], 1_048_576u64)
    } else if size_str.ends_with("KB") {
        (&size_str[..size_str.len() - 2], 1024u64)
    } else if size_str.ends_with('B') {
        (&size_str[..size_str.len() - 1], 1u64)
    } else {
        (size_str.as_str(), 1u64)
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| format!("Invalid size: {size_str}"))?;

    Ok((num * unit as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_gb() {
        assert_eq!(parse_size("5GB").unwrap(), 5 * 1_073_741_824);
    }

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(parse_size("500MB").unwrap(), 500 * 1_048_576);
    }

    #[test]
    fn test_parse_size_kb() {
        assert_eq!(parse_size("100KB").unwrap(), 100 * 1024);
    }

    #[test]
    fn test_parse_size_bytes() {
        assert_eq!(parse_size("1024B").unwrap(), 1024);
    }

    #[test]
    fn test_parse_size_lowercase() {
        assert_eq!(parse_size("2gb").unwrap(), 2 * 1_073_741_824);
    }

    #[test]
    fn test_parse_size_invalid() {
        assert!(parse_size("xyz").is_err());
    }

    #[test]
    fn test_parse_size_decimal() {
        assert_eq!(parse_size("1.5GB").unwrap(), 1_610_612_736); // 1.5 GiB
    }

    #[test]
    fn test_parse_size_blank() {
        assert!(parse_size("").is_err());
    }
}
