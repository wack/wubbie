//! wubbie — a fully-open small language model pipeline, from corpus to inference.
//!
//! This crate hosts the model definition, training loop, and inference entry
//! points for an all-Rust SLM pipeline built on [`burn`]. It is generic over
//! the Burn backend: the CPU (`ndarray`) backend is the default, and the NVIDIA
//! CUDA backend (CubeCL) is available via the `cuda` crate feature.
//!
//! Trained weights are **not** stored in this repository; they live in a
//! separate HuggingFace model repo. The [`weights`] module reads and writes the
//! on-disk [`safetensors`] format used to move them around.

pub use config::Cli;

pub mod backend;
pub mod cmd;
pub mod config;
pub mod corpus;
pub mod inference;
pub mod model;
pub mod tokenizer;
pub mod training;
pub mod weights;
