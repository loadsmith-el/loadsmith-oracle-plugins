# Running the Loadsmith Oracle connector (ODPI-C)

The Oracle connector links Oracle's **native client** (ODPI-C / Instant Client)
instead of a pure-Rust driver. That has one practical consequence: **the Oracle
Instant Client `.so` must be present at runtime.** Build time needs only a C
compiler; the client is `dlopen`'d at run time.

You have two ways to get a working setup:

1. **Use the ready image** `ghcr.io/loadsmith-el/loadsmith-oracle-odpi` — the slim
   engine with the plugins and the Instant Client already baked in. Easiest path;
   see [`../image/`](../image). Skip the rest of this doc.
2. **Bring your own image** — install the plugin binaries into an image you
   control and add the Instant Client yourself. The rest of this doc is about
   that. It is genuinely your responsibility: the right `.so` for the right
   architecture, the loader path, and the OTN license.

---

## 1. Install the plugin (binaries only)

The plugin is published out of canon, so install it from this repo's index or a
direct manifest — no special flag, the normal install paths work:

```bash
# From this repo's custom index:
loadsmith plugin install oracle --index \
  https://raw.githubusercontent.com/loadsmith-el/loadsmith-oracle-plugins/main/index.json

# …or a direct manifest URL / local path:
loadsmith plugin install --manifest ./loadsmith-plugin.yaml
```

> **The install delivers only the plugin binaries.** It does **not** configure
> your image: it won't download the Instant Client, won't touch `ld.so`, won't
> set `LD_LIBRARY_PATH`. That part is on you (next section) — or use the ready
> image.

The binaries are `loadsmith-source-oracle` and `loadsmith-destination-oracle`.
Put them where the engine's plugin discovery looks (`~/.loadsmith/plugins/`) or
anywhere on `PATH`.

## 2. Add the Oracle Instant Client (the `.so`, per architecture)

ODPI-C loads `libclntsh.so` at runtime. Download **Instant Client Basic** (or
**Basic Lite**, smaller) for **your image's architecture** from Oracle:

| Image arch       | Instant Client package        | Notes |
|------------------|-------------------------------|-------|
| `linux/amd64`    | `linux.x64` Basic / Basic Lite | always available |
| `linux/arm64`    | `linux.arm64` Basic / Basic Lite | arm64 IC since ~19.x; connects to Oracle **12c–19c** servers fine |

Then make it discoverable by the loader:

```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends libaio1 \
 && mkdir -p /opt/oracle
# Unpack the matching Instant Client zip into /opt/oracle/instantclient_XX_Y
COPY instantclient_23_5 /opt/oracle/instantclient_23_5
RUN echo /opt/oracle/instantclient_23_5 > /etc/ld.so.conf.d/oracle-instantclient.conf \
 && ldconfig
```

or, without `ldconfig`:

```bash
export LD_LIBRARY_PATH=/opt/oracle/instantclient_23_5:$LD_LIBRARY_PATH
```

**Match the architecture to the image.** An amd64 Instant Client in an arm64
image (or vice versa) will fail to load at run time. If you build multi-arch,
select the package from the build's `TARGETARCH` (see
[`../image/Dockerfile`](../image/Dockerfile) for a worked example).

Verify it's wired up:

```bash
ldd "$(command -v loadsmith-source-oracle)" | grep -i clntsh   # should resolve
```

## 3. TLS (wallet-based)

The native client verifies TLS against an **Oracle wallet**, not a PEM. Point the
connector's `tls.wallet_dir` at a directory holding an auto-login wallet
(`cwallet.sso`) that trusts your server's CA, and set `tls.enabled: true`
(connector uses `PROTOCOL=TCPS`). Building a wallet from a PEM needs
`orapki`/`mkstore`, which are **not** in Instant Client Basic — use a full client
or the `orapki` from an Oracle install to create it once, then ship the wallet
dir into your image / mount it.

```yaml
tls:
  enabled: true
  wallet_dir: /wallet          # holds cwallet.sso with your CA
  server_dn_match: true        # set false to skip the DN/SAN check
  distinguished_name: "CN=..." # optional: pin the expected server cert DN
```

## 4. Licensing — read this

The Oracle Instant Client is under the **Oracle Technology Network (OTN)**
license. Downloading it for your own use is one thing; **redistributing** an
image with it embedded is another, and it's **your responsibility** to comply.
This is the core reason the Oracle connector is out of canon and not in the
public canonical index: the canonical index ships freely redistributable,
pure-Rust artifacts, and the Instant Client is neither.

---

## When does this go away?

When `oracle-rs` (pure Rust) can read multi-packet result sets. Track the work in
the `loadsmith-el/oracle-rs` fork (`UPSTREAM-oracle-rs.md`). At that point Oracle
rejoins the canonical plugin set, installs into the plain slim image with no
Instant Client and no wallet gymnastics, and this whole repo is retired.
