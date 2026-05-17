//! PAR2 generation.
//!
//! A pure-Rust PAR2 (Parity Volume Set 2.0) creator. Parity is produced in the
//! same single read pass used for posting: each slice, as it is read and
//! yEnc-encoded, is also accumulated into the Reed-Solomon recovery buffers.
//!
//! See ROADMAP.md phase 7 for the development plan. This module currently
//! holds [`gf16`] — the GF(2^16) field and Reed-Solomon matrix (phase 7a).

pub mod gf16;
