//! Backend selection for the wubbie pipeline.
//!
//! Burn is generic over its compute backend. We pick the concrete backend at
//! compile time from the crate features, and pair each backend with the
//! precision it ships with:
//!
//! * `ndarray` (default) — a pure-Rust CPU backend that builds anywhere, used
//!   by CI and for small local runs. Runs in **f32**.
//! * `cuda` — the NVIDIA CUDA backend via CubeCL, the cloud-training path.
//!   Runs in **bf16** (the locked cloud recipe — see MULTI-1386).
//! * `wgpu` — the cross-platform WGPU backend via CubeCL. Dispatches to Metal
//!   on Apple hardware, Vulkan on Linux/Windows, and DirectX 12 on Windows.
//!   Runs in **f32**: Metal does not implement bf16 arithmetic over WGPU, so
//!   the local-development path stays at f32. This is the parallel local path
//!   introduced in MULTI-1407.
//!
//! Exactly one backend feature must be enabled. If more than one is requested
//! the higher-priority backend wins (`cuda` > `wgpu` > `ndarray`); the model
//! and training code remain backend-generic and don't change with the
//! selection.
//!
//! Training wraps the backend in [`burn::backend::Autodiff`] to enable reverse-
//! mode autodiff; inference uses the inner [`Backend`] directly.

#[cfg(not(any(feature = "ndarray", feature = "cuda", feature = "wgpu")))]
compile_error!("wubbie requires a backend feature: enable `ndarray` (default), `cuda`, or `wgpu`.");

/// The compute backend used for inference.
///
/// The CUDA backend runs in bf16 — the locked cloud-training recipe.
#[cfg(feature = "cuda")]
pub type Backend = burn::backend::Cuda<burn::tensor::bf16, i32>;

/// The compute backend used for inference.
///
/// The WGPU backend runs in f32: Metal (the macOS path) doesn't implement bf16
/// arithmetic via WGPU, so the local path keeps full precision.
#[cfg(all(feature = "wgpu", not(feature = "cuda")))]
pub type Backend = burn::backend::Wgpu<f32, i32>;

/// The compute backend used for inference.
#[cfg(all(feature = "ndarray", not(any(feature = "cuda", feature = "wgpu"))))]
pub type Backend = burn::backend::NdArray<f32>;

/// The autodiff-enabled backend used for training.
pub type TrainBackend = burn::backend::Autodiff<Backend>;

#[cfg(test)]
mod tests {
    use super::*;
    use burn::tensor::Tensor;

    /// The active backend instantiates a tensor and runs a forward + backward
    /// pass — the smallest end-to-end exercise of the selected stack. The same
    /// test covers each backend choice: on CI this runs against `ndarray`, on
    /// a CUDA host it runs against `cuda`, and on a Mac with `--features wgpu`
    /// it runs against Metal (via WGPU).
    #[test]
    fn forward_backward_runs_on_active_backend() {
        let device = Default::default();
        let x: Tensor<TrainBackend, 1> =
            Tensor::from_floats([1.0, 2.0, 3.0], &device).require_grad();
        let loss = (x.clone() * 2.0).sum();
        let grads = loss.backward();
        let grad = x.grad(&grads).expect("input must have a gradient");
        assert_eq!(grad.shape().num_elements(), 3);
    }
}
