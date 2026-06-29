//! The `wubbie download` command handler: fetch the corpus from Hugging Face.

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::config::DownloadSubcommand;
use crate::corpus::{self, DownloadReporter};

/// How often to emit a progress line during a download.
const LOG_EVERY: Duration = Duration::from_secs(2);

/// Handler for `wubbie download`.
pub struct Download {
    args: DownloadSubcommand,
}

impl Download {
    pub fn new(args: DownloadSubcommand) -> Result<Self> {
        Ok(Self { args })
    }

    pub fn dispatch(self) -> Result<()> {
        let source = self.args.hf_source();
        tracing::info!(
            repo = %source.repo,
            revision = %source.revision,
            "downloading corpus shards from Hugging Face",
        );

        let mut reporter = ProgressReporter::new();
        let paths = corpus::download_hf(&source, &mut reporter)?;

        println!(
            "Corpus ready: {} shard(s) — {} downloaded, {} already cached; {} fetched this run → {}",
            paths.len(),
            reporter.downloaded_files,
            reporter.cached_files,
            format_bytes(reporter.run_bytes),
            corpus::cache_location(&source).display(),
        );
        Ok(())
    }
}

/// Renders cross-file download progress: per-file percentage plus cumulative
/// bytes and throughput for the run, logged at most once per [`LOG_EVERY`].
struct ProgressReporter {
    started: Instant,
    last_log: Instant,
    total_files: usize,
    downloaded_files: usize,
    cached_files: usize,
    /// Bytes actually downloaded this run (cached files contribute nothing).
    run_bytes: u64,
    current_name: String,
    current_size: u64,
    current_done: u64,
}

impl ProgressReporter {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            last_log: now,
            total_files: 0,
            downloaded_files: 0,
            cached_files: 0,
            run_bytes: 0,
            current_name: String::new(),
            current_size: 0,
            current_done: 0,
        }
    }

    /// Bytes per second averaged over the run so far.
    fn rate(&self) -> f64 {
        let secs = self.started.elapsed().as_secs_f64();
        if secs > 0.0 {
            self.run_bytes as f64 / secs
        } else {
            0.0
        }
    }

    fn log_progress(&self) {
        let pct = if self.current_size > 0 {
            (self.current_done as f64 / self.current_size as f64) * 100.0
        } else {
            0.0
        };
        // `files_seen` is 1-based: the current file plus everything finished.
        let files_seen = self.downloaded_files + self.cached_files + 1;
        let message = format!(
            "[{files_seen}/{total}] {name}: {pct:.0}% ({done}/{size}) | {run} this run @ {rate}/s",
            total = self.total_files,
            name = self.current_name,
            done = format_bytes(self.current_done),
            size = format_bytes(self.current_size),
            run = format_bytes(self.run_bytes),
            rate = format_bytes(self.rate() as u64),
        );
        tracing::info!("{message}");
    }
}

impl DownloadReporter for ProgressReporter {
    fn file_started(&mut self, _index: usize, total: usize, name: &str, size: u64) {
        self.total_files = total;
        self.current_name = name.to_owned();
        self.current_size = size;
        self.current_done = 0;
        // Always announce the file boundary, then throttle the byte updates.
        self.last_log = Instant::now();
        let message = format!(
            "[{}/{}] downloading {} ({})",
            self.downloaded_files + self.cached_files + 1,
            total,
            name,
            format_bytes(size),
        );
        tracing::info!("{message}");
    }

    fn bytes_advanced(&mut self, delta: u64) {
        self.current_done += delta;
        self.run_bytes += delta;
        if self.last_log.elapsed() >= LOG_EVERY {
            self.log_progress();
            self.last_log = Instant::now();
        }
    }

    fn file_finished(&mut self) {
        self.downloaded_files += 1;
    }

    fn file_cached(&mut self, _index: usize, total: usize, name: &str) {
        self.total_files = total;
        self.cached_files += 1;
        let message = format!(
            "[{}/{}] {} already cached, skipping",
            self.downloaded_files + self.cached_files,
            total,
            name,
        );
        tracing::info!("{message}");
    }
}

/// Format a byte count with a binary unit (`B`/`KiB`/`MiB`/`GiB`/`TiB`).
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_with_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn reporter_accumulates_run_bytes_and_file_counts() {
        let mut reporter = ProgressReporter::new();
        reporter.file_started(0, 2, "a.jsonl.gz", 1000);
        reporter.bytes_advanced(400);
        reporter.bytes_advanced(600);
        reporter.file_finished();
        reporter.file_cached(1, 2, "b.jsonl.gz");
        assert_eq!(reporter.run_bytes, 1000);
        assert_eq!(reporter.downloaded_files, 1);
        assert_eq!(reporter.cached_files, 1);
        assert_eq!(reporter.total_files, 2);
    }
}
