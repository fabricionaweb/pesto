//! Public API of `pesto`, intended for integration with `upapasta`.
//!
//! `pesto` is a fast, lean Usenet poster: it yEnc-encodes files, posts the
//! resulting articles over NNTP and emits an `.nzb` file. See ROADMAP.md for
//! the development plan.

pub mod article;
pub mod config;
pub mod nntp;
pub mod nzb;
pub mod yenc;
