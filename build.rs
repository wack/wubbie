// Build-time provenance for OpenTelemetry resource attributes.
//
// Emits the canonical VCS_* and CICD_* compile-time env vars consumed by
// `src/utils/telemetry.rs` to populate OTel resource attributes following
// the CI/CD + VCS semantic conventions:
//   https://opentelemetry.io/docs/specs/semconv/resource/cicd/
//   https://opentelemetry.io/docs/specs/semconv/attributes-registry/vcs/
//
// Resolution priority for each value:
//   1. Explicit env var (set by Docker ARG -> ENV from CI), e.g. VCS_REF_HEAD_REVISION.
//   2. Local git shell-out (works for `cargo build` outside Docker).
//   3. "unknown" + cargo:warning so misconfigured builds are visible.

use std::env;
use std::process::Command;

const ATTRS: &[&str] = &[
    "VCS_REF_HEAD_REVISION",
    "VCS_REF_HEAD_NAME",
    "VCS_REF_HEAD_TYPE",
    "VCS_REPOSITORY_URL_FULL",
    "CICD_PIPELINE_NAME",
    "CICD_PIPELINE_RUN_URL_FULL",
];

fn main() {
    // Best-effort: emit VERGEN_GIT_* and other vergen env vars for future use.
    // Failure (e.g. no .git in a Docker builder) is non-fatal because the
    // canonical VCS_* vars below have their own resolution chain.
    let _ = run_vergen();

    resolve("VCS_REF_HEAD_REVISION", || git(&["rev-parse", "HEAD"]));
    resolve("VCS_REF_HEAD_NAME", || {
        git(&["rev-parse", "--abbrev-ref", "HEAD"])
    });
    resolve("VCS_REF_HEAD_TYPE", || Some("branch".to_string()));
    resolve("VCS_REPOSITORY_URL_FULL", || {
        git(&["config", "--get", "remote.origin.url"]).map(normalize_repo_url)
    });
    resolve("CICD_PIPELINE_NAME", || None);
    resolve("CICD_PIPELINE_RUN_URL_FULL", || None);

    for name in ATTRS {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // Re-run when CI toggles so the warning appears/disappears accordingly.
    println!("cargo:rerun-if-env-changed=CI");
}

fn run_vergen() -> Result<(), Box<dyn std::error::Error>> {
    let gitcl = vergen_gitcl::GitclBuilder::all_git()?;
    vergen_gitcl::Emitter::default()
        .add_instructions(&gitcl)?
        .emit()?;
    Ok(())
}

fn resolve(name: &str, fallback: impl FnOnce() -> Option<String>) {
    if let Ok(value) = env::var(name)
        && !value.is_empty()
    {
        println!("cargo:rustc-env={name}={value}");
        return;
    }
    if let Some(value) = fallback()
        && !value.is_empty()
    {
        println!("cargo:rustc-env={name}={value}");
        return;
    }
    // Only nag in CI; a missing provenance var is normal for local dev builds.
    if env::var_os("CI").is_some() {
        println!("cargo:warning=No build-time value for {name}; stamping \"unknown\"");
    }
    println!("cargo:rustc-env={name}=unknown");
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

// git remote URLs come in several forms:
//   https://github.com/wack/aviary.git
//   git@github.com:wack/aviary.git
//   ssh://git@github.com/wack/aviary.git
// Normalize to the canonical `https://github.com/<owner>/<repo>` form so the
// resource attribute matches what `${{ github.server_url }}/${{ github.repository }}`
// produces in CI.
fn normalize_repo_url(raw: String) -> String {
    let trimmed = raw.trim().trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        return format!("https://github.com/{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        return format!("https://github.com/{rest}");
    }
    trimmed.to_string()
}
