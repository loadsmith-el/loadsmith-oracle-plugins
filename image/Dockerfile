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

# Instant Client Basic Lite version (smallest variant with libclntsh).
ARG IC_VERSION=23.5.0.24.07
# Docker's TARGETARCH: "amd64" | "arm64".
ARG TARGETARCH

USER root

# Instant Client needs libaio at runtime; curl/unzip only to fetch + unpack it.
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends libaio1 curl ca-certificates unzip; \
    case "${TARGETARCH}" in \
      amd64) ic_arch="linux.x64";   ic_ver="${IC_VERSION}" ;; \
      arm64) ic_arch="linux.arm64"; ic_ver="${IC_VERSION}" ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    url="https://download.oracle.com/otn_software/linux/instantclient/${ic_ver%%.*}50000/instantclient-basiclite-${ic_arch}-${ic_ver}.zip"; \
    # Fall back to the versionless "latest" path if the pinned URL 404s.
    curl -fsSL -o /tmp/ic.zip "${url}" || \
      curl -fsSL -o /tmp/ic.zip "https://download.oracle.com/otn_software/linux/instantclient/instantclient-basiclite-${ic_arch}.zip"; \
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
