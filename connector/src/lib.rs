//! Loadsmith Oracle connector (source + destination) on the native Oracle client
//! (ODPI-C / Instant Client), via the `oracle` crate.
//!
//! Shipped out of canon (see the workspace `Cargo.toml`): ODPI-C reads
//! multi-packet result sets natively — which the pure-Rust `oracle-rs` driver
//! cannot yet do — at the cost of a C toolchain at build time and the Instant
//! Client `.so` at runtime.
//!
//! The `oracle` crate is **synchronous**; the SDK plugin traits are async. Each
//! async method runs its blocking DB work inside [`tokio::task::block_in_place`]
//! (the plugin binaries use the multi-threaded runtime). `oracle::Connection`
//! is `Send + Sync`, so it lives in the plugin struct across calls.

pub mod conn;
pub mod destination;
pub mod source;
pub mod types;

pub use destination::OracleDestPlugin;
pub use source::OracleSourcePlugin;
