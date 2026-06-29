//! Corpus access: resolve a corpus *source* to a set of local files, and stream
//! trainable text records out of them.
//!
//! **Storage model (MULTI-1378).** The filtered CommonPile slice lives on
//! Hugging Face — not mirrored into object storage — and is pulled on demand via
//! the pure-Rust [`hf_hub`] client, pinned by repository + revision for
//! reproducibility. A [`CorpusSource::Local`] path is also supported for tiny
//! local runs (MULTI-1408) and tests.
//!
//! **On-Hub format.** Records are JSON Lines (`.jsonl` / `.jsonl.gz`): one JSON
//! object per line, with the document text under a configurable field
//! (`text` by default). Plain text (`.txt` / `.txt.gz`) is also accepted, one
//! document per line. `.gz` shards are decompressed transparently. Parquet (the
//! HF auto-converted form) is out of scope here — the native JSONL is read
//! directly.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use flate2::read::MultiGzDecoder;
use hf_hub::api::Progress as HfProgress;
use hf_hub::api::sync::{ApiBuilder, ApiRepo};
use hf_hub::{Cache, Repo, RepoType};

/// The default JSON field holding a record's document text.
pub const DEFAULT_TEXT_FIELD: &str = "text";

/// A pinned Hugging Face dataset to pull the corpus from.
#[derive(Debug, Clone)]
pub struct HfSource {
    /// Dataset repo id, e.g. `someone/filtered-commonpile`.
    pub repo: String,
    /// Revision (branch, tag, or commit sha) pinned for reproducibility.
    pub revision: String,
    /// Explicit files to fetch. Empty means "discover every corpus file in the
    /// repo at this revision".
    pub files: Vec<String>,
    /// Override for the Hugging Face cache directory (where shards are stored).
    /// `None` uses the hf-hub default (`HF_HOME` / `~/.cache/huggingface`).
    pub cache_dir: Option<PathBuf>,
}

/// Where the corpus comes from.
#[derive(Debug, Clone)]
pub enum CorpusSource {
    /// A local file, or a directory swept (non-recursively) for corpus files.
    Local(PathBuf),
    /// A pinned Hugging Face dataset, pulled via [`hf_hub`].
    Hf(HfSource),
}

impl CorpusSource {
    /// Resolve this source to a concrete, sorted list of local file paths,
    /// downloading from Hugging Face into the local cache when needed.
    pub fn resolve_files(&self) -> Result<Vec<PathBuf>> {
        match self {
            CorpusSource::Local(path) => collect_local_files(path),
            CorpusSource::Hf(source) => fetch_hf_files(source),
        }
    }
}

/// Detected on-disk shape of a corpus file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Format {
    /// Whether each line is a JSON record (vs. a raw text line).
    json: bool,
    /// Whether the file is gzip-compressed.
    gzip: bool,
}

/// Infer a file's [`Format`] from its (case-insensitive) extension.
fn detect_format(path: &Path) -> Format {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let gzip = name.ends_with(".gz");
    let base = name.strip_suffix(".gz").unwrap_or(&name);
    let json = base.ends_with(".jsonl") || base.ends_with(".ndjson");
    Format { json, gzip }
}

/// Whether `name` looks like a corpus shard we can read.
fn is_corpus_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let base = name.strip_suffix(".gz").unwrap_or(&name);
    [".jsonl", ".ndjson", ".txt", ".text"]
        .iter()
        .any(|ext| base.ends_with(ext))
}

/// Open a corpus file as a line reader, decompressing `.gz` transparently.
fn open_lines(path: &Path, gzip: bool) -> Result<Box<dyn BufRead + Send>> {
    let file =
        File::open(path).with_context(|| format!("cannot open corpus file: {}", path.display()))?;
    if gzip {
        // `MultiGzDecoder` tolerates concatenated gzip members, which some
        // sharded `.gz` corpora use.
        Ok(Box::new(BufReader::new(MultiGzDecoder::new(
            BufReader::new(file),
        ))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

/// Extract the trainable text from one raw line.
///
/// For JSON Lines, parse the line and pull `text_field`; for plain text, the
/// line *is* the text. Returns `None` for blank lines and JSON records missing
/// or empty in that field.
fn record_text(line: &str, json: bool, text_field: &str) -> Result<Option<String>> {
    if !json {
        let keep = !line.trim().is_empty();
        return Ok(keep.then(|| line.to_owned()));
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_str(trimmed).context("malformed JSONL record")?;
    Ok(value
        .get(text_field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned))
}

/// Stream the document texts from `files` as a lazy, `Send` iterator suitable
/// for feeding the tokenizer trainer.
///
/// The stream is fault-tolerant: an unreadable file, an unreadable line, or a
/// malformed JSON record is logged and skipped rather than aborting a long
/// training run. (Files are resolved/downloaded up front, so open failures here
/// are unexpected.)
pub fn text_records(
    files: Vec<PathBuf>,
    text_field: String,
) -> impl Iterator<Item = String> + Send {
    files.into_iter().flat_map(move |path| {
        let format = detect_format(&path);
        let field = text_field.clone();
        let reader = match open_lines(&path, format.gzip) {
            Ok(reader) => Some(reader),
            Err(err) => {
                tracing::error!(path = %path.display(), error = %err, "skipping unreadable corpus file");
                None
            }
        };
        reader.into_iter().flat_map(move |reader| {
            let field = field.clone();
            reader.lines().filter_map(move |line| match line {
                Ok(line) => match record_text(&line, format.json, &field) {
                    Ok(text) => text,
                    Err(err) => {
                        tracing::warn!(error = %err, "skipping malformed corpus record");
                        None
                    }
                },
                Err(err) => {
                    tracing::warn!(error = %err, "skipping unreadable corpus line");
                    None
                }
            })
        })
    })
}

/// Read up to `max_chars` characters of document text for the post-training
/// sanity checks, joining records with newlines.
pub fn read_sample(files: &[PathBuf], text_field: &str, max_chars: usize) -> Result<String> {
    let mut sample = String::new();
    let mut chars = 0;
    for text in text_records(files.to_vec(), text_field.to_owned()) {
        chars += text.chars().count() + 1; // +1 for the joining '\n'
        sample.push_str(&text);
        sample.push('\n');
        if chars >= max_chars {
            break;
        }
    }
    ensure!(
        !sample.is_empty(),
        "corpus produced no text records — check the file format and --text-field",
    );
    Ok(sample)
}

/// Resolve a local `--input` path to its corpus files.
fn collect_local_files(input: &Path) -> Result<Vec<PathBuf>> {
    let metadata = fs::metadata(input)
        .with_context(|| format!("cannot read corpus input: {}", input.display()))?;

    if metadata.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }

    let mut files: Vec<PathBuf> = fs::read_dir(input)
        .with_context(|| format!("cannot read corpus directory: {}", input.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(is_corpus_file)
        })
        .collect();
    files.sort();

    ensure!(
        !files.is_empty(),
        "no corpus files (.jsonl/.jsonl.gz/.txt) found in directory: {}",
        input.display(),
    );
    Ok(files)
}

/// The hf-hub [`Cache`] for a source, honoring its `cache_dir` override.
fn hf_cache(source: &HfSource) -> Cache {
    match &source.cache_dir {
        Some(dir) => Cache::new(dir.clone()),
        None => Cache::from_env(),
    }
}

/// The pinned dataset [`Repo`] for a source.
fn hf_repo(source: &HfSource) -> Repo {
    Repo::with_revision(
        source.repo.clone(),
        RepoType::Dataset,
        source.revision.clone(),
    )
}

/// Resolve the corpus filenames for a source: the explicit `--hf-file` list, or
/// every corpus file discovered in the repo at the pinned revision (one network
/// listing call).
fn resolve_filenames(api_repo: &ApiRepo, source: &HfSource) -> Result<Vec<String>> {
    if !source.files.is_empty() {
        return Ok(source.files.clone());
    }
    let info = api_repo.info().map_err(|err| {
        anyhow!(
            "failed to list files in dataset {}@{}: {err}",
            source.repo,
            source.revision,
        )
    })?;
    let mut names: Vec<String> = info
        .siblings
        .into_iter()
        .map(|sibling| sibling.rfilename)
        .filter(|name| is_corpus_file(name))
        .collect();
    names.sort();
    ensure!(
        !names.is_empty(),
        "no corpus files (.jsonl/.jsonl.gz/.txt) found in dataset {}@{}",
        source.repo,
        source.revision,
    );
    Ok(names)
}

/// Resolve a pinned HF dataset to local shard paths, reading **only from the
/// local cache** — training never triggers a bulk download. Missing shards are a
/// hard error pointing at `wubbie download`, so pulling 521 GB is always an
/// explicit, separate step (see [`download_hf`]).
fn fetch_hf_files(source: &HfSource) -> Result<Vec<PathBuf>> {
    let cache = hf_cache(source);
    let api = ApiBuilder::from_cache(cache.clone())
        .with_progress(false)
        .build()
        .map_err(|err| anyhow!("failed to initialize Hugging Face client: {err}"))?;
    let api_repo = api.repo(hf_repo(source));
    let cache_repo = cache.repo(hf_repo(source));

    let filenames = resolve_filenames(&api_repo, source)?;
    filenames
        .iter()
        .map(|name| {
            cache_repo.get(name).ok_or_else(|| {
                anyhow!(
                    "{name} from dataset {repo}@{rev} is not in the local cache — \
                     run `wubbie download --hf-repo {repo} --hf-revision {rev}` first",
                    name = name,
                    repo = source.repo,
                    rev = source.revision,
                )
            })
        })
        .collect()
}

/// Reports progress of a corpus [`download_hf`], one method per event. The CLI
/// implements this to render a progress display; keeping it a trait keeps
/// `corpus` free of presentation concerns.
pub trait DownloadReporter {
    /// A file's download is starting; `size` is its total size in bytes.
    fn file_started(&mut self, index: usize, total: usize, name: &str, size: u64);
    /// `delta` more bytes of the current file have been written to disk.
    fn bytes_advanced(&mut self, delta: u64);
    /// The current file finished downloading.
    fn file_finished(&mut self);
    /// A file was already in the cache and is being skipped (resume support).
    fn file_cached(&mut self, index: usize, total: usize, name: &str);
}

/// Bridges hf-hub's per-file [`HfProgress`] callbacks to our [`DownloadReporter`],
/// carrying the file's position in the overall set.
struct ProgressAdapter<'a, R: DownloadReporter> {
    reporter: &'a mut R,
    index: usize,
    total: usize,
}

impl<R: DownloadReporter> HfProgress for ProgressAdapter<'_, R> {
    fn init(&mut self, size: usize, filename: &str) {
        self.reporter
            .file_started(self.index, self.total, filename, size as u64);
    }

    fn update(&mut self, size: usize) {
        self.reporter.bytes_advanced(size as u64);
    }

    fn finish(&mut self) {
        self.reporter.file_finished();
    }
}

/// Download a pinned HF dataset's corpus files into the local cache, reporting
/// progress and **skipping any file already cached** so an interrupted run
/// resumes where it left off. Returns the local paths of all shards.
///
/// This is the explicit bulk-fetch step (`wubbie download`); training reads the
/// resulting cache via [`fetch_hf_files`] without downloading.
pub fn download_hf(
    source: &HfSource,
    reporter: &mut impl DownloadReporter,
) -> Result<Vec<PathBuf>> {
    let cache = hf_cache(source);
    let api = ApiBuilder::from_cache(cache.clone())
        // We render our own cross-file progress, so suppress hf-hub's per-file bar.
        .with_progress(false)
        .build()
        .map_err(|err| anyhow!("failed to initialize Hugging Face client: {err}"))?;
    let api_repo = api.repo(hf_repo(source));
    let cache_repo = cache.repo(hf_repo(source));

    let filenames = resolve_filenames(&api_repo, source)?;
    let total = filenames.len();
    let mut paths = Vec::with_capacity(total);
    for (index, name) in filenames.iter().enumerate() {
        if let Some(path) = cache_repo.get(name) {
            reporter.file_cached(index, total, name);
            paths.push(path);
            continue;
        }
        // `&mut *reporter` reborrows, so the adapter's borrow ends when the
        // download returns and the reporter is free for the next iteration.
        let adapter = ProgressAdapter {
            reporter: &mut *reporter,
            index,
            total,
        };
        let path = api_repo
            .download_with_progress(name, adapter)
            .map_err(|err| {
                anyhow!(
                    "failed to download {name} from dataset {}@{}: {err}",
                    source.repo,
                    source.revision,
                )
            })?;
        paths.push(path);
    }
    Ok(paths)
}

/// The local cache directory shards land in, for reporting after a download.
pub fn cache_location(source: &HfSource) -> PathBuf {
    hf_cache(source).path().clone()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn detects_format_from_extension() {
        assert_eq!(
            detect_format(Path::new("shard.jsonl")),
            Format {
                json: true,
                gzip: false
            }
        );
        assert_eq!(
            detect_format(Path::new("shard.jsonl.gz")),
            Format {
                json: true,
                gzip: true
            }
        );
        assert_eq!(
            detect_format(Path::new("notes.txt")),
            Format {
                json: false,
                gzip: false
            }
        );
        assert_eq!(
            detect_format(Path::new("notes.TXT.GZ")),
            Format {
                json: false,
                gzip: true
            }
        );
    }

    #[test]
    fn corpus_file_filter_accepts_known_extensions() {
        for ok in ["a.jsonl", "a.jsonl.gz", "a.ndjson", "a.txt", "a.txt.gz"] {
            assert!(is_corpus_file(ok), "{ok} should be accepted");
        }
        for bad in ["a.parquet", "a.json", "README.md", "a.gz"] {
            assert!(!is_corpus_file(bad), "{bad} should be rejected");
        }
    }

    #[test]
    fn extracts_text_from_jsonl_and_skips_blanks_and_missing() {
        let field = DEFAULT_TEXT_FIELD;
        assert_eq!(
            record_text(r#"{"text": "hello", "meta": 1}"#, true, field).unwrap(),
            Some("hello".to_owned())
        );
        // Missing / empty field, and blank lines, are skipped.
        assert_eq!(record_text(r#"{"meta": 1}"#, true, field).unwrap(), None);
        assert_eq!(record_text(r#"{"text": ""}"#, true, field).unwrap(), None);
        assert_eq!(record_text("   ", true, field).unwrap(), None);
        // Malformed JSON is an error (the stream logs and skips it).
        assert!(record_text("{not json", true, field).is_err());
    }

    #[test]
    fn custom_text_field_is_honored() {
        assert_eq!(
            record_text(r#"{"content": "hi"}"#, true, "content").unwrap(),
            Some("hi".to_owned())
        );
    }

    #[test]
    fn plain_text_lines_pass_through_except_blanks() {
        assert_eq!(
            record_text("a line", false, DEFAULT_TEXT_FIELD).unwrap(),
            Some("a line".to_owned())
        );
        assert_eq!(record_text("  ", false, DEFAULT_TEXT_FIELD).unwrap(), None);
    }

    #[test]
    fn streams_records_across_plain_and_gzipped_jsonl() {
        let dir = std::env::temp_dir().join(format!("wubbie-corpus-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");

        // A plain JSONL shard...
        fs::write(
            dir.join("a.jsonl"),
            "{\"text\": \"alpha\"}\n{\"text\": \"beta\"}\n{\"meta\": 1}\n",
        )
        .expect("write jsonl");
        // ...and a gzipped one.
        let gz_path = dir.join("b.jsonl.gz");
        let mut encoder = flate2::write::GzEncoder::new(
            File::create(&gz_path).expect("create gz"),
            flate2::Compression::default(),
        );
        encoder
            .write_all(b"{\"text\": \"gamma\"}\n")
            .expect("write gz");
        encoder.finish().expect("finish gz");

        let files = collect_local_files(&dir).expect("collect files");
        assert_eq!(files.len(), 2);

        let mut texts: Vec<String> = text_records(files, DEFAULT_TEXT_FIELD.to_owned()).collect();
        texts.sort();
        assert_eq!(texts, vec!["alpha", "beta", "gamma"]);

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn empty_directory_is_rejected() {
        let dir = std::env::temp_dir().join(format!("wubbie-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        let err = collect_local_files(&dir).expect_err("empty dir is rejected");
        assert!(err.to_string().contains("no corpus files"));
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn explicit_hf_files_skip_the_network_listing() {
        // With explicit `--hf-file`s, resolution must not hit the network: it
        // returns the given names verbatim. (Building the client is offline.)
        let cache = Cache::new(std::env::temp_dir().join("wubbie-hf-noop"));
        let api = ApiBuilder::from_cache(cache)
            .with_progress(false)
            .build()
            .expect("build offline api");
        let source = HfSource {
            repo: "owner/repo".to_owned(),
            revision: "main".to_owned(),
            files: vec!["a.jsonl".to_owned(), "b.jsonl.gz".to_owned()],
            cache_dir: None,
        };
        let names = resolve_filenames(&api.repo(hf_repo(&source)), &source).expect("resolves");
        assert_eq!(names, ["a.jsonl", "b.jsonl.gz"]);
    }
}
