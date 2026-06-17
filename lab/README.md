# Oracle (ODPI-C) lab cases

These are `loadsmith-lab` cases for the out-of-canon Oracle connector. This
directory is a **local origin** that ships only cases — the Oracle **DB fixture**
`images/lab-oracle` stays in the canonical images repo
(`loadsmith-lab-canonical-images`) and is referenced **cross-origin**.

## Setup

```bash
cd ../loadsmith-lab            # the lab engine
# register both origins: the canonical images (for the DB fixture) and these cases
cargo run -p loadsmith-lab-cli -- origin local add images    ../loadsmith-lab-canonical-images
cargo run -p loadsmith-lab-cli -- origin local add oracle-lab ../loadsmith-oracle-plugins/lab
cargo run -p loadsmith-lab-cli -- list                       # see oracle-lab/oracle-to-*
```

## Running

The connector needs the Instant Client at runtime, so the engine must run from
the delivery image (which bakes it in), **not** the plain slim. Each case pins
that image itself via its `loadsmith.image` key — a per-case engine-image
override the runner already supports ([`resolve_loadsmith_image`](https://github.com/loadsmith-el/loadsmith-lab/blob/main/crates/loadsmith-lab-runner/src/image.rs)) —
so no special flag is needed:

```bash
loadsmith-lab run --select oracle-lab/oracle-to-jsonl
```

The image must be resolvable: either pull/publish
`ghcr.io/loadsmith-el/loadsmith-oracle-odpi:slim`, or build it locally and tag it
under that ref (`docker tag <local> ghcr.io/loadsmith-el/loadsmith-oracle-odpi:slim`)
— the runner uses a locally-present image as-is before attempting a pull.

## Cases

| Case | What | State |
|------|------|-------|
| `oracle-to-jsonl` | read 100k wide rows → JSONL (content/type smoke) | ready |
| `oracle-to-jsonl-tls` | read over TCPS (wallet-verified) | ready |
| `oracle-to-jsonl-incremental` | watermark read (cursor > 50000 → upper 50k) | ready |
| `oracle-to-oracle` | load 100k rows (atomic INSERT) | ready |
| `oracle-to-oracle-staged-merge` | load 100k via `MERGE` upsert by id | ready |
| `oracle-to-oracle-lob` | round-trip RAW/BLOB/CLOB Oracle→Oracle | ready |
| `oracle-to-oracle-nls` | load under a comma-decimal `NLS_LANG` | ready |

## The TLS wallet

ODPI-C verifies TLS against an Oracle **wallet** (`cwallet.sso`), not a PEM. The
`oracle-to-jsonl-tls` case points `tls.wallet_dir` at `/wallet` and mounts an
auto-login wallet there via `loadsmith.volumes`. The wallet (built from the lab
CA — it holds only the public cert, so it is committed) lives in
`cases/oracle-to-jsonl-tls/wallet/cwallet.sso`; regenerate it with the adjacent
`wallet/generate.sh`. That script runs orapki's main class from Maven Central's
PKI jars in a stock JRE container — `orapki`/`mkstore` are stripped from the
Instant Client and the slim DB images, so no Oracle image is needed. The lab
stunnel must serve the full leaf → CA chain (Oracle's NZ TLS won't build a path
from a wallet-held CA); see `loadsmith-lab-canonical-images/images/lab-oracle`.

## The validation that matters

`oracle-to-jsonl` reading **all 100k rows of the full 34-column table** is the
case the pure-Rust `oracle-rs` driver failed (multi-packet truncation). ODPI-C
reads it natively; confirming this end-to-end is the headline proof the pivot
worked.
