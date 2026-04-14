//! Pure-Rust port of the tree-sitter C runtime.
//!
//! This crate is built as a `staticlib` that exports the tree-sitter C ABI.
//! It is intended to progressively replace `lib/src/*.c`. Each module below
//! corresponds to a C translation unit that has been ported; the remaining
//! symbols continue to be provided by the C sources listed in `CMakeLists.txt`.
//!
//! Ported so far:
//!   - `point.c`  (see [`point`])

#![allow(clippy::missing_safety_doc)]

pub mod point;
