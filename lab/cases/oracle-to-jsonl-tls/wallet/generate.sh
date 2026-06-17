#!/usr/bin/env bash
# Regenerate the auto-login Oracle wallet (cwallet.sso) this case mounts at
# /wallet. ODPI-C verifies TLS against an Oracle wallet, not a PEM file, so the
# lab's stunnel TLS terminator (lab-oracle image, :2484) is trusted by putting
# the lab CA into an auto-login wallet.
#
# The wallet tooling (orapki/mkstore) ships in the full Oracle client only — the
# Instant Client and the slim "Free" DB images strip it (no JRE, no PKI jars).
# Oracle does publish the PKI jars to Maven Central, so we run orapki's main
# class with a stock JRE — no Oracle image required.
#
# The wallet holds ONLY the public lab CA (no private key), so the resulting
# cwallet.sso is safe to commit. Re-run this only if the lab CA changes.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ca="${1:-$here/../../../../../loadsmith-lab-canonical-images/images/lab-oracle/tls/ca.crt}"
[ -f "$ca" ] || { echo "lab CA not found: $ca" >&2; exit 1; }

ORACLEPKI=23.5.0.24.07
OSDT=21.18.0.0

docker run --rm \
  -v "$ca":/work/ca.crt:ro \
  -v "$here":/work/out \
  -e "OUT_UID=$(id -u)" -e "OUT_GID=$(id -g)" \
  -e "ORACLEPKI=$ORACLEPKI" -e "OSDT=$OSDT" \
  --entrypoint bash eclipse-temurin:17 -lc '
    set -e
    apt-get update -qq >/dev/null && apt-get install -y -qq curl >/dev/null
    cd /tmp && mkdir -p jars wallet
    M=https://repo1.maven.org/maven2/com/oracle/database/security
    curl -fsSL -o jars/oraclepki.jar "$M/oraclepki/$ORACLEPKI/oraclepki-$ORACLEPKI.jar"
    curl -fsSL -o jars/osdt_core.jar "$M/osdt_core/$OSDT/osdt_core-$OSDT.jar"
    curl -fsSL -o jars/osdt_cert.jar "$M/osdt_cert/$OSDT/osdt_cert-$OSDT.jar"
    CP=jars/oraclepki.jar:jars/osdt_core.jar:jars/osdt_cert.jar
    PKI="java -cp $CP oracle.security.pki.textui.OraclePKITextUI"
    $PKI wallet create -wallet wallet -auto_login_only >/dev/null
    $PKI wallet add -wallet wallet -trusted_cert -cert /work/ca.crt -auto_login_only >/dev/null
    install -m 0644 wallet/cwallet.sso /work/out/cwallet.sso
    chown "$OUT_UID:$OUT_GID" /work/out/cwallet.sso
    $PKI wallet display -wallet wallet | grep -A1 "Trusted Certificates"
  '
echo "wrote $here/cwallet.sso"
