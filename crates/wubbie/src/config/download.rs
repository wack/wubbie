//! Arguments for `wubbie download`.

use std::path::{Path, PathBuf};

use clap::Args;

use crate::corpus::HfSource;

/// `wubbie download`: pull a pinned Hugging Face dataset's corpus shards into the
/// local cache.
///
/// This is the explicit bulk-fetch step. The filtered CommonPile slice is large
/// (hundreds of GB), so downloading is its own command with progress reporting,
/// kept separate from `wubbie tokenizer` (which then trains from the cache
/// without downloading). Already-cached shards are skipped, so an interrupted
/// download resumes where it left off.
#[derive(Debug, Args, Clone)]
pub struct DownloadSubcommand {
    /// Hugging Face dataset repo id to pull (e.g. `owner/filtered-commonpile`).
    #[arg(long, value_name = "REPO_ID")]
    hf_repo: String,

    /// Revision (branch, tag, or commit sha) to pin for reproducibility.
    #[arg(long, value_name = "REV", default_value = "main")]
    hf_revision: String,

    /// Specific file(s) to fetch (repeatable). When omitted, every corpus file
    /// in the repo at the pinned revision is downloaded.
    #[arg(long = "hf-file", value_name = "FILE")]
    hf_files: Vec<String>,

    /// Override the Hugging Face cache directory to download into. Defaults to
    /// the hf-hub location (`HF_HOME` / `~/.cache/huggingface`). Point this at a
    /// large-capacity volume for big corpora.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,
}

impl DownloadSubcommand {
    /// The pinned Hugging Face source described by these arguments.
    pub fn hf_source(&self) -> HfSource {
        HfSource {
            repo: self.hf_repo.clone(),
            revision: self.hf_revision.clone(),
            files: self.hf_files.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }

    /// The cache-directory override, if one was supplied.
    pub fn cache_dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: DownloadSubcommand,
    }

    fn parse(argv: &[&str]) -> Result<DownloadSubcommand, clap::Error> {
        Harness::try_parse_from(argv).map(|h| h.args)
    }

    #[test]
    fn hf_repo_is_required() {
        assert!(parse(&["download"]).is_err());
    }

    #[test]
    fn builds_the_pinned_source_with_defaults() {
        let args = parse(&["download", "--hf-repo", "owner/repo"]).expect("parses");
        let source = args.hf_source();
        assert_eq!(source.repo, "owner/repo");
        assert_eq!(source.revision, "main");
        assert!(source.files.is_empty());
        assert!(source.cache_dir.is_none());
    }

    #[test]
    fn pins_revision_files_and_cache_dir() {
        let args = parse(&[
            "download",
            "--hf-repo",
            "owner/repo",
            "--hf-revision",
            "deadbeef",
            "--hf-file",
            "data/a.jsonl.gz",
            "--cache-dir",
            "/mnt/big/hf",
        ])
        .expect("parses");
        let source = args.hf_source();
        assert_eq!(source.revision, "deadbeef");
        assert_eq!(source.files, ["data/a.jsonl.gz"]);
        assert_eq!(source.cache_dir.as_deref(), Some(Path::new("/mnt/big/hf")));
        assert_eq!(args.cache_dir(), Some(Path::new("/mnt/big/hf")));
    }
}
