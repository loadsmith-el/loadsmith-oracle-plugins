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

- **~~Live lab validation~~ (DONE — all 7 cases green, 2026-06-16).** Validated
  end-to-end against `lab-oracle` through the `loadsmith-oracle-odpi` image. The
  full suite — `oracle-to-jsonl` (the headline `SELECT *` 100k×34-col full read,
  the exact case `oracle-rs` failed, ~110k rows/s), `oracle-to-jsonl-tls`,
  `oracle-to-jsonl-incremental`, `oracle-to-oracle` (atomic),
  `oracle-to-oracle-staged-merge`, `oracle-to-oracle-lob`, and
  `oracle-to-oracle-nls` — passes. The first three plaintext cases surfaced and
  fixed three real issues (the TLS/incremental/LOB/NLS/arm64 items below came
  from extending the suite):
  - **Identifier case-folding.** Oracle folds unquoted names to upper-case;
    `quote_ident` now folds simple identifiers (with `fold_ident` for comparing
    config names like `merge_key`/`cursor_column` against the catalog).
  - **`staged_merge` was O(n²).** It ran a per-row `MERGE … FROM dual` whose plan
    was hard-parsed on the empty target → full scan per row. Rewritten to stage
    into a session-private global temporary table + one set-based `MERGE`
    (100k in ~4.6s, was 32k in 12 min).
  - **Image glibc gotcha.** Plugin binaries must be built in `rust:1-bookworm`
    (glibc 2.36) to run on the slim base — a host build (newer glibc) fails at
    load. (CI already builds per-arch; matters for local image builds.)
  - **Lab readiness race.** The seed COMMIT becomes visible before `events_sink`
    exists, so the destination cases now probe the last init.sql object
    (`PK_SINK`). Only `oracle-to-jsonl-tls` is left (needs the wallet, below).

- **~~Runner engine-image override~~ (resolved — no engine change needed).** The
  ODPI-C cases need the engine started from `loadsmith-oracle-odpi` (it carries
  the Instant Client), not the plain slim. The runner already supports this: a
  case's `loadsmith.image` key is a per-case full-ref engine-image override
  (`resolve_loadsmith_image`). Each Oracle case now pins
  `ghcr.io/loadsmith-el/loadsmith-oracle-odpi:slim` directly — no new
  `--loadsmith-image` flag, no canonical engine change.

- **~~TLS wallet for the lab~~ (DONE — `oracle-to-jsonl-tls` green, 2026-06-16).**
  ODPI-C TLS is wallet-based, not PEM. An auto-login wallet (`cwallet.sso`) built
  from the lab CA is committed at `lab/cases/oracle-to-jsonl-tls/wallet/` (it holds
  only the public CA — safe to commit) and mounted at `/wallet` via the case's
  `loadsmith.volumes`. The wallet tooling (`orapki`) ships only in the full client
  — the Instant Client and slim DB images strip it — so `wallet/generate.sh` runs
  orapki's main class from Maven Central's PKI jars in a stock JRE container (no
  Oracle image needed). The run surfaced and fixed two more issues:
  - **ORA-29024 (cert validation).** Oracle's NZ TLS layer validates only against
    the chain the SERVER presents — it won't build a path from a CA it merely
    holds in its wallet (unlike openssl/rustls). The lab stunnel served only the
    leaf; `tls/gen-certs.sh` + `server.pem` now serve the full leaf → CA chain.
  - **Readiness race.** The TLS case connects on stunnel's 2484, but the readiness
    probe gated on 1521; it now gates `tcp: 2484` (stunnel starts after init.sql,
    so an open 2484 implies the seed is loaded too).
  - Mounting the wallet needed a small generic runner change: a case
    `loadsmith.volumes[].host` is now resolved relative to the case dir (so a case
    can ship its own fixtures portably).

- **~~Incremental read~~ (DONE — `oracle-to-jsonl-incremental` green).** New case
  with `incremental.cursor_column = event_sequence`, `initial_value = 50000` reads
  exactly the upper 50k — exercises the cursor predicate, the NLS-safe typed cast,
  and case-insensitive cursor matching.

- **~~Binary/LOB coverage~~ (DONE — `oracle-to-oracle-lob` green).** A small
  `lob_roundtrip` fixture (RAW/BLOB/CLOB + a NULL row) added to the lab-oracle
  init.sql; the case round-trips it Oracle→Oracle. Byte fidelity confirmed with
  `DBMS_LOB.COMPARE` = 0 for BLOB and CLOB and exact RAW equality.

- **~~NLS on implicit number conversion~~ (FOUND + FIXED — `oracle-to-oracle-nls`
  green).** Numbers cross as text, so a comma-decimal locale (`NLS_LANG=GERMAN…`)
  broke the round-trip with ORA-01722. `conn::connect` now pins
  `NLS_NUMERIC_CHARACTERS = '.,'` on every session, making the text↔NUMBER
  conversions deterministic regardless of client/server locale. Verified: 0
  decimal mismatches across 100k rows under the hostile locale.

- **~~Instant Client arm64 URL~~ (FOUND + FIXED).** The Dockerfile built the arm64
  IC URL like x64 (`linux.arm64-<version>.zip`) — that URL 404s for every version.
  Oracle ships the ARM64 Instant Client ONLY at the versionless path with a hyphen
  (`instantclient-basiclite-linux-arm64.zip`, currently IC 23.26). Fixed; the arm64
  image now builds, the plugin bins are AArch64, and `libclntsh` is in the ldconfig
  cache (smoke-tested under QEMU). Real arm64 data-path e2e runs in CI (below).

- **CI never run.** `.github/workflows/release.yml` (binaries + image) is written
  but unproven — validate on the first real `oracle-v*` tag. A native-arm64 lab
  e2e job (GitHub `ubuntu-24.04-arm`, real hardware, not QEMU) is planned to prove
  the arm64 data path and guard regressions — the first automated lab e2e.
