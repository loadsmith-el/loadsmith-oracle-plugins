# loadsmith-oracle-plugins

The Oracle **source + destination** connector for [Loadsmith](https://github.com/loadsmith-el/loadsmith),
shipped **out of canon** because it links Oracle's native client
(**ODPI-C / Instant Client**) instead of a pure-Rust driver.

## Why out of canon

The canonical plugin set is pure-Rust and multi-arch "for free". Oracle's only
pure-Rust driver (`oracle-rs`) can't yet read result sets larger than one TNS
packet — a showstopper for real migrations (see the
[`loadsmith-el/oracle-rs`](https://github.com/loadsmith-el/oracle-rs) fork and
its `UPSTREAM-oracle-rs.md`). Until that's fixed upstream, Oracle ships here on
**ODPI-C**, which reads result sets natively and supports amd64 + arm64 — at the
cost of:

- a **C toolchain at build time** (the `oracle` crate compiles ODPI-C; no Instant
  Client needed to *build*), and
- the **Oracle Instant Client `.so` at runtime** (`dlopen`'d), which carries
  **OTN licensing**.

When `oracle-rs` matures, Oracle returns to the canonical set, installs into the
plain slim image via `loadsmith install`, and this repo + the
`loadsmith-oracle-odpi` image are **retired**.

## Layout

| Path | What |
|------|------|
| [`connector/`](connector) | the plugin crate — source + destination, two binaries, on the `oracle` crate (ODPI-C) |
| [`lab/`](lab) | loadsmith-lab cases (an origin); the DB fixture `images/lab-oracle` stays in the canonical images repo (cross-origin ref) |
| [`image/`](image) | the `loadsmith-oracle-odpi` delivery image (slim + Instant Client + plugin) |
| [`docs/`](docs) | how to run it: install, build your own image, Instant Client per arch, TLS wallets, licensing |
| [`ci/`](ci) + [`.github/`](.github/workflows) | release tooling (binaries + image) |
| [`index.json`](index.json) | the custom install index |

## Quick start

Use the ready image (Instant Client baked in):

```bash
loadsmith ... # inside ghcr.io/loadsmith-el/loadsmith-oracle-odpi:slim
```

Or install the plugin into your own (Instant-Client-equipped) image:

```bash
loadsmith plugin install oracle --index \
  https://raw.githubusercontent.com/loadsmith-el/loadsmith-oracle-plugins/main/index.json
```

See [`docs/`](docs) — especially the Instant Client and licensing sections.

## Develop

```bash
cargo build                    # compiles ODPI-C (needs a C compiler); no IC needed
cargo test -p loadsmith-oracle # unit tests (SQL/config; no DB)
```

Live tests and lab cases need a running Oracle + the Instant Client; see
[`lab/README.md`](lab/README.md).
