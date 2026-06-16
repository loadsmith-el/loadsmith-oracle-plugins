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
the delivery image (which bakes it in), **not** the plain slim:

```bash
loadsmith-lab run --select oracle-lab/oracle-to-jsonl \
  --loadsmith-image ghcr.io/loadsmith-el/loadsmith-oracle-odpi
```

> ⚠️ **`--loadsmith-image` does not exist in the runner yet** — it pins the engine
> repo to `ghcr.io/loadsmith-el/loadsmith` and only varies `--tag`. Adding a
> generic engine-image override is a tracked follow-up (see [ROADMAP](../ROADMAP.md)).
> Until then the cases can't run unchanged through the standard runner path.

## Cases

| Case | What | State |
|------|------|-------|
| `oracle-to-jsonl` | read 100k wide rows → JSONL (content/type smoke) | plaintext, ready |
| `oracle-to-oracle` | load 100k rows (atomic INSERT) | plaintext, ready |
| `oracle-to-oracle-staged-merge` | load 100k via `MERGE` upsert by id | plaintext, ready |
| `oracle-to-jsonl-tls` | read over TCPS (wallet-verified) | **needs a wallet** (below) |

## The TLS wallet follow-up

ODPI-C verifies TLS against an Oracle **wallet** (`cwallet.sso`), not a PEM. The
`oracle-to-jsonl-tls` case points `tls.wallet_dir` at `/wallet`, which must hold
an auto-login wallet built from the lab CA
(`loadsmith-lab-canonical-images/images/lab-oracle/tls/ca.crt`). Building that
wallet needs `orapki`/`mkstore` (not in Instant Client Basic). Wiring it in —
bake into the delivery image, or generate + mount in the lab — is a follow-up;
the plaintext cases cover read/write/merge until then.

## The validation that matters

`oracle-to-jsonl` reading **all 100k rows of the full 34-column table** is the
case the pure-Rust `oracle-rs` driver failed (multi-packet truncation). ODPI-C
reads it natively; confirming this end-to-end is the headline proof the pivot
worked.
