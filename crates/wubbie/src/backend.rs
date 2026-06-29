//! Backend selection for the wubbie pipeline.
//!
//! Burn is generic over its compute backend. We pick the concrete backend at
//! compile time from the crate features:
//!
//! * `ndarray` (default) — a pure-Rust CPU backend that builds anywhere, used
//!   by CI and for small local runs.
//! * `cuda` — the NVIDIA CUDA backend via CubeCL, for GPU training/inference.
//!
//! Training wraps the backend in [`burn::backend::Autodiff`] to enable reverse-
//! mode autodiff; inference uses the inner [`Backend`] directly.

#[cfg(not(any(feature = "ndarray", feature = "cuda")))]
compile_error!("wubbie requires a backend feature: enable `ndarray` (default) or `cuda`.");

/// The compute backend used for inference.
#[cfg(feature = "cuda")]
pub type Backend = burn::backend::Cuda<f32, i32>;

/// The compute backend used for inference.
#[cfg(all(feature = "ndarray", not(feature = "cuda")))]
pub type Backend = burn::backend::NdArray<f32>;

/// The autodiff-enabled backend used for training.
pub type TrainBackend = burn::backend::Autodiff<Backend>;
