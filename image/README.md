# `loadsmith-oracle-odpi` delivery image

The Loadsmith engine **slim** image, plus the Oracle **source + destination**
plugins (native ODPI-C) and the **Oracle Instant Client** baked in. It's the
turnkey way to run Oracle pipelines without assembling the native client
yourself.

```
ghcr.io/loadsmith-el/loadsmith-oracle-odpi:slim
```

## Why this image exists (and why it's temporary)

The native Oracle client needs `libclntsh.so` at runtime, which the plain slim
image doesn't carry. This image bundles it. **It is interim**: once the pure-Rust
`oracle-rs` driver can read multi-packet result sets (see the
`loadsmith-el/oracle-rs` fork and its `UPSTREAM-oracle-rs.md`), Oracle moves back
into the canonical plugin set, installs into the plain slim image via
`loadsmith install oracle`, and this image is retired.

## What's inside

- `FROM ghcr.io/loadsmith-el/loadsmith:slim` (Debian bookworm, glibc).
- Oracle Instant Client **Basic Lite** for the image's architecture
  (`linux/amd64` or `linux/arm64`), on the loader path via `ld.so.conf.d`.
- `loadsmith-source-oracle` / `loadsmith-destination-oracle` on `PATH`.

## Multi-arch

Published for `linux/amd64` and `linux/arm64`. The Instant Client is arch-specific
(arm64 IC ≥ 19.x connects to Oracle 12c–19c just fine); the plugin binaries are
built natively per arch and passed into the build context under `bin/<arch>/`.
See [`../ci/`](../ci) for the build/publish workflow.

## ⚠️ Instant Client licensing (your responsibility)

The Oracle Instant Client is distributed under the **Oracle Technology Network
(OTN)** license. Publishing or sharing an image with it embedded is **your
decision and your responsibility** — review the OTN terms. This is the main
reason Oracle ships out of canon and this image lives in a separate repo rather
than the public canonical index. See [`../docs/`](../docs) for the full rundown,
including how to build your **own** image instead.

## Build locally

```bash
# Build the plugin binaries for your arch first (see ci/), drop them in:
#   bin/amd64/loadsmith-{source,destination}-oracle
docker buildx build \
  --platform linux/amd64 \
  -t ghcr.io/loadsmith-el/loadsmith-oracle-odpi:dev \
  -f image/Dockerfile .
```
