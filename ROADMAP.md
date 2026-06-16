# Roadmap

## The exit condition (why this repo is temporary)

This repo exists only until the pure-Rust `oracle-rs` driver can read multi-packet
result sets. Track that in the [`loadsmith-el/oracle-rs`](https://github.com/loadsmith-el/oracle-rs)
fork (`UPSTREAM-oracle-rs.md`, issue #7). When it lands upstream:

1. Re-add an Oracle plugin to `loadsmith-canonical-plugins` on the published
   pure-Rust `oracle-rs`.
2. Move the lab cases back to the canonical catalog.
3. Retire this repo and the `loadsmith-oracle-odpi` image.

## Open follow-ups (validated as TODO, not done)

- **Live lab validation.** The connector compiles and its unit tests pass, but
  end-to-end behavior (read 100k wide rows, write, MERGE) still needs a run
  against the `lab-oracle` fixture through an Instant-Client-equipped engine. The
  key milestone: `SELECT *` over the full 34-column table reads completely — the
  exact case `oracle-rs` failed. See [lab/README.md](lab/README.md).

- **Runner engine-image override (`--loadsmith-image`).** The lab runner pins the
  engine image repo to `ghcr.io/loadsmith-el/loadsmith` and only varies `--tag`.
  Running the ODPI-C cases needs the engine started from `loadsmith-oracle-odpi`
  (it carries the Instant Client). This wants a generic `--loadsmith-image <repo>`
  flag in `loadsmith-lab` (a small, plugin-agnostic engine change). Until then,
  validate by other means (a local engine build inside an IC container).

- **TLS wallet for the lab.** ODPI-C TLS is wallet-based, not PEM. The
  `oracle-to-jsonl-tls` case needs an auto-login wallet (`cwallet.sso`) built from
  the lab CA and mounted at the engine's `wallet_dir`. Building it needs
  `orapki`/`mkstore` (not in Instant Client Basic). Decide whether to bake a
  wallet into the delivery image or generate one in the lab. The plaintext cases
  cover read/write/merge meanwhile.

- **CI never run.** `.github/workflows/release.yml` (binaries + image) is written
  but unproven — validate on the first real `oracle-v*` tag.

- **NLS on implicit number conversion.** Numbers bind/read as text; in a session
  whose `NLS_NUMERIC_CHARACTERS` uses `,` as the decimal separator, implicit
  char↔number could misparse. The lab's default (American) territory is fine;
  revisit if a deployment uses a non-`.` decimal.

- **Instant Client version pin.** `image/Dockerfile` pins an IC Basic Lite
  version with a "latest" fallback URL. Confirm the pinned URL/version and that
  the arm64 package covers the client's 12c–19c servers.
