# CLAUDE.md

Operating instructions for this repo. For *what* it is and *why*, read
[README.md](README.md) and the [docs](docs).

## Conventions

- **English only.** All committed artifacts — code, comments, commit messages,
  identifiers, docs — in English, even when the user writes in Portuguese.
- **This is out of canon on purpose.** It links the native Oracle client
  (ODPI-C / Instant Client). Do NOT try to make it pure-Rust or add it to the
  canonical index — that's the *future* path (via the `oracle-rs` fork), not this
  repo. Keep the canonical/pure-Rust posture for the OTHER plugins.
- **Mirror the canonical connector shape:** one crate, shared `conn`/`types`,
  two separate binaries (`src/bin/source.rs`, `src/bin/destination.rs`). The
  driver-agnostic SQL (projected `TO_CHAR` reads, `TO_*` binds, `MERGE`) is
  carried over verbatim from the canonical Oracle connector — keep it that way so
  it stays portable back to pure-Rust later.
- **Keep docs in sync.** Changing behavior/commands/shipping → check README,
  docs/, ROADMAP, and the image/lab READMEs.

## The async↔sync bridge

The `oracle` crate is synchronous; the SDK traits are async. Every DB call runs
inside `tokio::task::block_in_place` (the plugin bins use the multi-thread
runtime). `oracle::Connection` is `Send + Sync`, so it lives in the plugin struct
across calls. Don't reach for `spawn_blocking` (it would force moving the
non-`'static` connection); `block_in_place` is the pattern here.

## Build / test

```bash
cargo build                          # compiles bundled ODPI-C C (needs `cc`)
cargo test -p loadsmith-oracle       # unit tests only (no DB, no Instant Client)
```

Building needs a C compiler but NOT the Instant Client (ODPI-C dlopens
`libclntsh` at runtime). Live behavior needs a real Oracle + the client — see
[lab/README.md](lab/README.md).

## Releasing

Tag `oracle-v<version>` (matching `connector/Cargo.toml`). The
[release workflow](.github/workflows/release.yml) builds the binaries
(amd64+arm64), publishes a GitHub Release + updates `index.json`, and builds +
pushes the `loadsmith-oracle-odpi` image. Push tags one at a time.
