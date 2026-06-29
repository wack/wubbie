//! Layered configuration loading.
//!
//! A small builder over [`figment`] that merges configuration from several
//! sources of increasing precedence — hardcoded defaults, an on-disk config
//! file (which may set only *some* fields), `WUBBIE_`-prefixed environment
//! variables, and CLI flags — and extracts a fully-specified struct.
//!
//! The extracted struct has **no `Option`s**: every field is supplied by some
//! layer or by the seeded defaults, so a missing value can't leak downstream. A
//! field left unset by every layer with no default is a hard error rather than a
//! silent `None`.
//!
//! ## Environment layer
//!
//! The env layer is **injected**, not read from `std::env` here. The CLI
//! boundary captures `std::env::vars()` once into an [`EnvOverrides`] map (see
//! [`read_model_env_overrides`]) and threads that map through; tests construct
//! the map explicitly and never mutate process state. This rule exists because
//! the test suite must stay deterministic — `std::env::set_var` is
//! process-global and not thread-safe, and prior to MULTI-1407 the loader read
//! `std::env` directly inside parallel tests, which produced cross-test env
//! pollution and flakes.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use clap::Args;
use figment::{
    Figment,
    providers::{Format, Json, Serialized, Toml},
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{ModelConfig, PartialModelConfig};

/// Environment-variable prefix for the model-config layer, e.g.
/// `WUBBIE_MODEL_D_MODEL` or `WUBBIE_MODEL_NUM_LAYERS`.
pub const MODEL_ENV_PREFIX: &str = "WUBBIE_MODEL_";

/// An env-style override layer: keys with the prefix already stripped and
/// lowercased, values parsed as JSON (numbers, bools, …) with a string
/// fallback for anything that isn't valid JSON.
///
/// Builders typically construct this via [`parse_env_overrides`] from a
/// `(key, value)` iterator (process env or an explicit test map) rather than
/// populating the map by hand.
pub type EnvOverrides = BTreeMap<String, serde_json::Value>;

/// A builder that layers configuration sources by precedence and extracts a
/// fully-specified `T`. Each `merge`/`with_*` adds a higher-precedence layer:
/// later layers win on a per-field basis.
#[derive(Debug, Default)]
pub struct LayeredConfig {
    figment: Figment,
}

impl LayeredConfig {
    /// An empty layer stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed the lowest-precedence layer with a (typically fully-populated)
    /// defaults value. Without this, any field not set by a later layer has no
    /// default and [`extract`](Self::extract) errors.
    #[must_use]
    pub fn with_defaults<T: Serialize>(mut self, defaults: &T) -> Self {
        self.figment = self.figment.merge(Serialized::defaults(defaults));
        self
    }

    /// Merge a config file selected by extension (`.toml`, `.json`/`.jsonc`).
    /// The file must exist, but it may be partial — any keys it omits fall
    /// through to lower-precedence layers.
    pub fn with_file(mut self, path: &Path) -> Result<Self> {
        if !path.exists() {
            bail!("config file not found: {}", path.display());
        }
        self.figment = match path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") => self.figment.merge(Toml::file(path)),
            Some("json") | Some("jsonc") => self.figment.merge(Json::file(path)),
            other => bail!(
                "unsupported config file extension {:?} (expected .toml or .json): {}",
                other.unwrap_or(""),
                path.display(),
            ),
        };
        Ok(self)
    }

    /// Merge a config file if a path was supplied; otherwise leave the stack
    /// untouched (the file layer is optional).
    pub fn with_optional_file(self, path: Option<&Path>) -> Result<Self> {
        match path {
            Some(path) => self.with_file(path),
            None => Ok(self),
        }
    }

    /// Merge an env-style override layer.
    ///
    /// The map is treated as a `Serialized` defaults source, so a missing key
    /// contributes nothing and an unknown key surfaces as a typed extraction
    /// error rather than being silently dropped. The layer does **not** read
    /// `std::env` — see the module docs for why; build the map via
    /// [`parse_env_overrides`].
    #[must_use]
    pub fn with_env(mut self, env: &EnvOverrides) -> Self {
        self.figment = self.figment.merge(Serialized::defaults(env));
        self
    }

    /// Merge the highest-precedence override layer (typically CLI flags). The
    /// override type should skip-serialize unset fields so they don't clobber
    /// lower layers.
    #[must_use]
    pub fn with_overrides<T: Serialize>(mut self, overrides: &T) -> Self {
        self.figment = self.figment.merge(Serialized::defaults(overrides));
        self
    }

    /// Extract the fully-specified `T`, eliminating every `Option`. A field
    /// absent from all layers with no default is an error; `what` names the
    /// config in the message.
    pub fn extract<T: DeserializeOwned>(self, what: &str) -> Result<T> {
        self.figment
            .extract()
            .map_err(|err| anyhow!("invalid {what} configuration: {err}"))
    }
}

/// Filter, strip-prefix, lowercase, and JSON-parse a `(key, value)` iterator
/// (typically `std::env::vars()`) into an [`EnvOverrides`] map.
///
/// Keys not starting with `prefix` are dropped. Each retained value is parsed
/// as JSON so `"2222"` becomes a number, `"true"` a bool, etc.; any value that
/// is not valid JSON is kept as a raw string so figment can surface a typed
/// error against the destination field rather than silently dropping the
/// override.
pub fn parse_env_overrides<I, K, V>(prefix: &str, vars: I) -> EnvOverrides
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut out = EnvOverrides::new();
    for (key, value) in vars {
        let Some(stripped) = key.as_ref().strip_prefix(prefix) else {
            continue;
        };
        let key = stripped.to_ascii_lowercase();
        let raw = value.as_ref();
        let value =
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_owned()));
        out.insert(key, value);
    }
    out
}

/// Read the process environment for [`MODEL_ENV_PREFIX`]-prefixed variables.
///
/// This is the one CLI-side place that touches `std::env`; the loader itself
/// stays pure. Tests construct an [`EnvOverrides`] map directly and never call
/// this.
pub fn read_model_env_overrides() -> EnvOverrides {
    parse_env_overrides(MODEL_ENV_PREFIX, std::env::vars())
}

/// CLI flags overriding individual model dimensions.
///
/// Flattened into subcommands that build a [`ModelConfig`]. Every field is
/// optional so an unset flag contributes nothing to the merge — this is why none
/// carry a clap `default_value`.
#[derive(Debug, Args, Clone, Default)]
pub struct ModelConfigArgs {
    /// Override the tokenizer vocabulary size.
    #[arg(long)]
    vocab_size: Option<usize>,
    /// Override the context length.
    #[arg(long)]
    context_length: Option<usize>,
    /// Override the residual-stream width (`d_model`).
    #[arg(long)]
    d_model: Option<usize>,
    /// Override the feed-forward inner dimension (`d_ff`).
    #[arg(long)]
    d_ff: Option<usize>,
    /// Override the number of transformer blocks.
    #[arg(long)]
    num_layers: Option<usize>,
    /// Override the number of attention heads.
    #[arg(long)]
    num_heads: Option<usize>,
    /// Override the layer-norm placement (`true` = pre-norm).
    #[arg(long)]
    norm_first: Option<bool>,
}

impl ModelConfigArgs {
    /// The override layer carrying only the flags the user actually set.
    pub fn to_partial(&self) -> PartialModelConfig {
        PartialModelConfig {
            vocab_size: self.vocab_size,
            context_length: self.context_length,
            d_model: self.d_model,
            d_ff: self.d_ff,
            num_layers: self.num_layers,
            num_heads: self.num_heads,
            norm_first: self.norm_first,
        }
    }
}

/// Resolve a fully-specified [`ModelConfig`] from layered sources.
///
/// Precedence (low → high): the named-size `base`, an optional config `file`,
/// the `env` layer, then the CLI override flags. The result has no `Option`s;
/// because `base` supplies every field, resolution always succeeds for the
/// named sizes.
///
/// The `env` map is supplied by the caller — typically
/// [`read_model_env_overrides`] at the CLI boundary, or an empty/explicit map
/// in tests. The loader itself does not touch `std::env`.
pub fn load_model_config(
    base: &ModelConfig,
    file: Option<&Path>,
    env: &EnvOverrides,
    overrides: &ModelConfigArgs,
) -> Result<ModelConfig> {
    LayeredConfig::new()
        .with_defaults(base)
        .with_optional_file(file)?
        .with_env(env)
        .with_overrides(&overrides.to_partial())
        .extract("model")
}

#[cfg(test)]
mod tests {
    // `figment::Jail`'s closure returns a large `Result`; unavoidable given the
    // API, so allow it in these hermetic tests.
    #![allow(clippy::result_large_err)]

    use std::path::Path;

    use figment::Jail;

    use super::*;

    fn overrides_with_layers(num_layers: usize) -> ModelConfigArgs {
        ModelConfigArgs {
            num_layers: Some(num_layers),
            ..ModelConfigArgs::default()
        }
    }

    fn no_env() -> EnvOverrides {
        EnvOverrides::new()
    }

    #[test]
    fn defaults_only_returns_the_base_unchanged() {
        let base = ModelConfig::gpt2_small();
        let resolved =
            load_model_config(&base, None, &no_env(), &ModelConfigArgs::default()).expect("ok");
        assert_eq!(resolved, base);
    }

    #[test]
    fn unset_flags_do_not_clobber_the_base() {
        let base = ModelConfig::debug_tiny();
        let resolved =
            load_model_config(&base, None, &no_env(), &ModelConfigArgs::default()).expect("ok");
        assert_eq!(resolved, base);
    }

    #[test]
    fn partial_file_overrides_only_its_own_fields() {
        Jail::expect_with(|jail| {
            jail.create_file("model.toml", "d_model = 1024\nnum_heads = 16\n")?;
            let base = ModelConfig::gpt2_small();
            let resolved = load_model_config(
                &base,
                Some(Path::new("model.toml")),
                &no_env(),
                &ModelConfigArgs::default(),
            )
            .expect("resolves");

            assert_eq!(resolved.d_model, 1024); // from file
            assert_eq!(resolved.num_heads, 16); // from file
            assert_eq!(resolved.num_layers, base.num_layers); // from base
            assert_eq!(resolved.d_ff, base.d_ff); // from base
            Ok(())
        });
    }

    #[test]
    fn flag_beats_file_beats_base() {
        Jail::expect_with(|jail| {
            jail.create_file("model.toml", "num_layers = 6\n")?;
            let base = ModelConfig::gpt2_small(); // num_layers = 12
            let resolved = load_model_config(
                &base,
                Some(Path::new("model.toml")), // num_layers = 6
                &no_env(),
                &overrides_with_layers(3), // num_layers = 3 (wins)
            )
            .expect("resolves");
            assert_eq!(resolved.num_layers, 3);
            Ok(())
        });
    }

    #[test]
    fn env_beats_file_and_flag_beats_env() {
        Jail::expect_with(|jail| {
            jail.create_file("model.toml", "d_ff = 1111\n")?;
            // Inject env via the explicit map, not the process env — so this
            // test cannot pollute concurrent tests (the whole reason
            // `with_env` no longer reads `std::env`).
            let env = parse_env_overrides(MODEL_ENV_PREFIX, [("WUBBIE_MODEL_D_FF", "2222")]);
            let base = ModelConfig::gpt2_small();

            // Env outranks the file...
            let resolved = load_model_config(
                &base,
                Some(Path::new("model.toml")),
                &env,
                &ModelConfigArgs::default(),
            )
            .expect("resolves");
            assert_eq!(resolved.d_ff, 2222);

            // ...and a flag outranks env.
            let overrides = ModelConfigArgs {
                d_ff: Some(3333),
                ..ModelConfigArgs::default()
            };
            let resolved =
                load_model_config(&base, Some(Path::new("model.toml")), &env, &overrides)
                    .expect("ok");
            assert_eq!(resolved.d_ff, 3333);
            Ok(())
        });
    }

    #[test]
    fn missing_field_with_no_default_is_an_error() {
        Jail::expect_with(|jail| {
            // A partial file and NO defaults layer: most fields are unsupplied,
            // so extraction into the non-optional `ModelConfig` must fail rather
            // than invent values.
            jail.create_file("model.toml", "d_model = 768\n")?;
            let result: Result<ModelConfig> = LayeredConfig::new()
                .with_file(Path::new("model.toml"))
                .expect("file exists")
                .extract("model");
            assert!(result.is_err(), "expected a missing-field error");
            Ok(())
        });
    }

    #[test]
    fn missing_file_path_is_rejected() {
        let err = LayeredConfig::new()
            .with_file(Path::new("does-not-exist.toml"))
            .expect_err("a missing explicit config file is an error");
        assert!(err.to_string().contains("config file not found"));
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        Jail::expect_with(|jail| {
            jail.create_file("model.yaml", "d_model: 768\n")?;
            let err = LayeredConfig::new()
                .with_file(Path::new("model.yaml"))
                .expect_err("yaml is unsupported");
            assert!(
                err.to_string()
                    .contains("unsupported config file extension")
            );
            Ok(())
        });
    }

    #[test]
    fn parse_env_overrides_strips_prefix_and_parses_values() {
        let env = parse_env_overrides(
            "WUBBIE_MODEL_",
            [
                ("WUBBIE_MODEL_D_FF", "2222"),
                ("WUBBIE_MODEL_NORM_FIRST", "true"),
                ("UNRELATED_VAR", "ignored"),
            ],
        );
        assert_eq!(env.get("d_ff"), Some(&serde_json::json!(2222)));
        assert_eq!(env.get("norm_first"), Some(&serde_json::json!(true)));
        assert!(!env.contains_key("unrelated_var"));
    }

    #[test]
    fn parse_env_overrides_falls_back_to_string_for_invalid_json() {
        let env = parse_env_overrides("WUBBIE_MODEL_", [("WUBBIE_MODEL_D_FF", "not-a-number")]);
        assert_eq!(
            env.get("d_ff"),
            Some(&serde_json::Value::String("not-a-number".to_owned())),
        );
    }
}
