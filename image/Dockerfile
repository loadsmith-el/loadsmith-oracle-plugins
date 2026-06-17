# loadsmith-oracle-odpi — the Loadsmith engine slim image with the Oracle
# (ODPI-C) source + destination plugins and the Oracle Instant Client baked in.
#
# This image is INTERIM. It exists only because the native Oracle client needs
# its `.so` (libclntsh) at runtime. When the pure-Rust `oracle-rs` driver can
# read multi-packet result sets (see the loadsmith-el/oracle-rs fork), Oracle
# returns to the canonical plugin set, installs via `loadsmith install` like any
# other plugin into the plain slim image, and THIS image is retired.
#
# Multi-arch: built for linux/amd64 and linux/arm64. The Instant Client is
# arch-specific, selected from TARGETARCH. The plugin binaries are built natively
# per-arch and passed in via the build context (see ci/, mirroring the engine
# release). Build with buildx and a per-arch binary in ./bin/<arch>/.
#
# LICENSING: the Oracle Instant Client is under the Oracle Technology Network
# (OTN) license. Redistributing an image with it embedded is YOUR decision and
# YOUR responsibility — see the repo docs. This Dockerfile downloads Basic Lite
# from Oracle's public endpoint at build time.

ARG BASE=ghcr.io/loadsmith-el/loadsmith:slim
FROM ${BASE}

# Instant Client Basic Lite version (smallest variant with libclntsh). This pins
# the x86-64 download; the ARM64 build uses Oracle's versionless "latest" arm64
# package (Oracle ships no versioned ARM64 URL — see the RUN block below).
ARG IC_VERSION=23.5.0.24.07
# Docker's TARGETARCH: "amd64" | "arm64".
ARG TARGETARCH

USER root

# Instant Client needs libaio at runtime; curl/unzip only to fetch + unpack it.
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends libaio1 curl ca-certificates unzip; \
    base="https://download.oracle.com/otn_software/linux/instantclient"; \
    case "${TARGETARCH}" in \
      # x86-64: a pinned, versioned Basic Lite (IC_VERSION). \
      amd64) url="${base}/${IC_VERSION%%.*}50000/instantclient-basiclite-linux.x64-${IC_VERSION}.zip" ;; \
      # aarch64: Oracle publishes the ARM64 Instant Client ONLY at the versionless \
      # "latest" path, and with a HYPHEN ("linux-arm64", not "linux.arm64") — there \
      # is no versioned ARM64 URL like x64 has. It currently resolves to IC 23.x \
      # (compatible with 12c–19c servers, same as the x64 pin). \
      arm64) url="${base}/instantclient-basiclite-linux-arm64.zip" ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/ic.zip "${url}"; \
    mkdir -p /opt/oracle; \
    unzip -q /tmp/ic.zip -d /opt/oracle; \
    icdir="$(find /opt/oracle -maxdepth 1 -type d -name 'instantclient*' | head -n1)"; \
    echo "${icdir}" > /etc/ld.so.conf.d/oracle-instantclient.conf; \
    ldconfig; \
    rm -f /tmp/ic.zip; \
    apt-get purge -y curl unzip; \
    apt-get autoremove -y; \
    rm -rf /var/lib/apt/lists/*

# The Oracle plugin binaries, built natively for this arch (see ci/). Placed on
# PATH so the core's plugin discovery finds them (it falls back to PATH).
COPY bin/${TARGETARCH}/loadsmith-source-oracle      /usr/local/bin/loadsmith-source-oracle
COPY bin/${TARGETARCH}/loadsmith-destination-oracle /usr/local/bin/loadsmith-destination-oracle
RUN chmod 0755 /usr/local/bin/loadsmith-source-oracle /usr/local/bin/loadsmith-destination-oracle

# Inherit the base image's ENTRYPOINT (`loadsmith`).
