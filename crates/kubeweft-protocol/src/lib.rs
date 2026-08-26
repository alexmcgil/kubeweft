//! Rust-side protocol-integration boundary.
//!
//! This crate owns the protocol boundary marker. It must not own domain
//! semantics, generated protocol code, transport selection, or `protoc` setup.

/// Marker module for handwritten Rust-to-protocol integration boundaries.
pub mod boundary {}
