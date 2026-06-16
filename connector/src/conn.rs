//! Shared Oracle connection config + `connect()` for the native (ODPI-C) driver.
//!
//! Both the source and destination embed [`ConnectionConfig`] (via
//! `#[serde(flatten)]`) and call [`connect`], so the connection surface — the
//! TNS descriptor, connect timeout, and TLS — lives in one place.
//!
//! **TLS is wallet-based**, the way the Oracle network stack does it: unlike the
//! pure-Rust plugins (which take an inline PEM `root_cert` and a rustls
//! provider), ODPI-C verifies the server against an **Oracle wallet** containing
//! the trusted CA. You point [`TlsConfig::wallet_dir`] at a directory holding an
//! auto-login wallet (`cwallet.sso`); the connector puts it in the descriptor's
//! `MY_WALLET_DIRECTORY`. There is no PEM path — building a wallet from a PEM
//! needs `orapki`/`mkstore` (not in Instant Client Basic). See the repo docs.
//!
//! **Text↔value conversions are kept NLS-independent at the SQL level** (the
//! source renders temporal/CLOB columns with `TO_CHAR`, the destination wraps
//! temporal binds in `TO_DATE`/`TO_TIMESTAMP`/`TO_TIMESTAMP_TZ`). ODPI-C decodes
//! these types correctly, but doing the conversion in SQL keeps the exact
//! canonical text format regardless of session NLS — and matches the connector's
//! text-preserving contract with the rest of Loadsmith.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use oracle::Connection;
use serde::Deserialize;

// ── Canonical text formats (shared by source TO_CHAR and destination TO_*) ─────

/// `DATE` → text. Oracle DATE always carries a time component.
pub const FMT_DATE: &str = "YYYY-MM-DD HH24:MI:SS";
/// `TIMESTAMP` → text, microsecond precision (canonical data is µs).
pub const FMT_TIMESTAMP: &str = "YYYY-MM-DD HH24:MI:SS.FF6";
/// `TIMESTAMP WITH [LOCAL] TIME ZONE` → text, with explicit offset.
pub const FMT_TIMESTAMP_TZ: &str = "YYYY-MM-DD HH24:MI:SS.FF6 TZH:TZM";

// ── Config structs ─────────────────────────────────────────────────────────────

/// Connection-level config shared by both oracle plugins. Embed it with
/// `#[serde(flatten)]` alongside plugin-specific fields (query/target_table/…).
#[derive(Debug, Deserialize)]
pub struct ConnectionConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Oracle service name (e.g. `FREEPDB1`). Either this or [`Self::sid`].
    #[serde(default)]
    pub service_name: Option<String>,
    /// Legacy SID, as an alternative to `service_name`.
    #[serde(default)]
    pub sid: Option<String>,
    pub user: String,
    pub password: String,

    #[serde(default)]
    pub tls: Option<TlsConfig>,

    /// Connection establishment timeout ("10s", "500ms"); default 10s.
    #[serde(default)]
    pub connect_timeout: Option<String>,
}

/// Wallet-based TLS for the native client. When `enabled`, the connector uses
/// `PROTOCOL=TCPS`; the server is verified against the wallet at [`Self::wallet_dir`].
#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// Turn on TCPS. When false the block is a no-op (plaintext TCP).
    #[serde(default)]
    pub enabled: bool,
    /// Directory holding an auto-login Oracle wallet (`cwallet.sso`) with the
    /// trusted CA. Required when `enabled` and the server cert isn't chained to a
    /// CA already in a default wallet.
    #[serde(default)]
    pub wallet_dir: Option<String>,
    /// `SSL_SERVER_DN_MATCH`: verify the server certificate DN/SAN matches the
    /// service (full verification). Defaults to true.
    #[serde(default = "default_true")]
    pub server_dn_match: bool,
    /// Expected server distinguished name (`SSL_SERVER_CERT_DN`); use with
    /// `server_dn_match` when the cert's DN must be pinned explicitly.
    #[serde(default)]
    pub distinguished_name: Option<String>,
}

fn default_host() -> String {
    "localhost".to_string()
}
fn default_port() -> u16 {
    1521
}
fn default_true() -> bool {
    true
}

// ── Duration parsing ──────────────────────────────────────────────────────────

/// Parses a human-readable duration string into a `Duration`.
/// Accepted suffixes: `ms`, `s`, `m`, `h`, `d`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    // Check "ms" before "s" to avoid stripping just the trailing 's' from "10ms".
    if let Some(n) = s.strip_suffix("ms") {
        return Ok(Duration::from_millis(n.trim().parse::<u64>().map_err(|_| dur_err(s))?));
    }
    if let Some(n) = s.strip_suffix('s') {
        return Ok(Duration::from_secs(n.trim().parse::<u64>().map_err(|_| dur_err(s))?));
    }
    if let Some(n) = s.strip_suffix('m') {
        return Ok(Duration::from_secs(n.trim().parse::<u64>().map_err(|_| dur_err(s))? * 60));
    }
    if let Some(n) = s.strip_suffix('h') {
        return Ok(Duration::from_secs(n.trim().parse::<u64>().map_err(|_| dur_err(s))? * 3600));
    }
    if let Some(n) = s.strip_suffix('d') {
        return Ok(Duration::from_secs(n.trim().parse::<u64>().map_err(|_| dur_err(s))? * 86400));
    }
    Err(dur_err(s))
}

fn dur_err(s: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "invalid duration '{s}': expected a number followed by ms/s/m/h/d (e.g. '10s', '500ms', '2h')"
    )
}

// ── Connect descriptor ─────────────────────────────────────────────────────────

/// Builds the TNS connect descriptor string passed to `oracle::Connection::connect`.
/// Plaintext uses `PROTOCOL=TCP`; TLS uses `PROTOCOL=TCPS` plus a `SECURITY`
/// section (wallet directory + DN matching).
pub fn build_descriptor(cfg: &ConnectionConfig) -> Result<String> {
    let connect_data = match (&cfg.service_name, &cfg.sid) {
        (Some(svc), _) => format!("(SERVICE_NAME={svc})"),
        (None, Some(sid)) => format!("(SID={sid})"),
        (None, None) => bail!("oracle config requires either service_name or sid"),
    };

    let tls_on = cfg.tls.as_ref().map(|t| t.enabled).unwrap_or(false);
    let protocol = if tls_on { "TCPS" } else { "TCP" };

    let timeout = match &cfg.connect_timeout {
        Some(s) => parse_duration(s).context("invalid connect_timeout")?.as_secs().max(1),
        None => 10,
    };

    let security = if tls_on {
        let tls = cfg.tls.as_ref().unwrap();
        let mut parts = String::new();
        parts.push_str(if tls.server_dn_match {
            "(SSL_SERVER_DN_MATCH=yes)"
        } else {
            "(SSL_SERVER_DN_MATCH=no)"
        });
        if let Some(dn) = &tls.distinguished_name {
            parts.push_str(&format!("(SSL_SERVER_CERT_DN=\"{dn}\")"));
        }
        if let Some(dir) = &tls.wallet_dir {
            parts.push_str(&format!("(MY_WALLET_DIRECTORY={dir})"));
        }
        format!("(SECURITY={parts})")
    } else {
        String::new()
    };

    Ok(format!(
        "(DESCRIPTION=(CONNECT_TIMEOUT={timeout})\
         (ADDRESS=(PROTOCOL={protocol})(HOST={host})(PORT={port}))\
         (CONNECT_DATA={connect_data}){security})",
        host = cfg.host,
        port = cfg.port,
    ))
}

/// Connects to Oracle through the native client using the built descriptor.
///
/// Synchronous (ODPI-C); call inside [`tokio::task::block_in_place`].
pub fn connect(cfg: &ConnectionConfig) -> Result<Connection> {
    let descriptor = build_descriptor(cfg)?;
    Connection::connect(&cfg.user, &cfg.password, &descriptor)
        .map_err(|e| anyhow::anyhow!("oracle connect failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(json: serde_json::Value) -> ConnectionConfig {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn parse_duration_units() {
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert!(parse_duration("100").is_err());
        assert!(parse_duration("10y").is_err());
    }

    #[test]
    fn descriptor_plaintext_service() {
        let c = cfg(serde_json::json!({
            "service_name": "FREEPDB1", "user": "lab", "password": "lab"
        }));
        let d = build_descriptor(&c).unwrap();
        assert!(d.contains("(PROTOCOL=TCP)"));
        assert!(d.contains("(SERVICE_NAME=FREEPDB1)"));
        assert!(d.contains("(PORT=1521)"));
        assert!(!d.contains("SECURITY"));
        assert!(d.contains("(CONNECT_TIMEOUT=10)"));
    }

    #[test]
    fn descriptor_requires_service_or_sid() {
        let c = cfg(serde_json::json!({ "user": "lab", "password": "lab" }));
        assert!(build_descriptor(&c).is_err());
    }

    #[test]
    fn descriptor_tls_uses_tcps_and_wallet() {
        let c = cfg(serde_json::json!({
            "service_name": "FREEPDB1", "user": "lab", "password": "lab",
            "tls": { "enabled": true, "wallet_dir": "/wallet", "server_dn_match": false }
        }));
        let d = build_descriptor(&c).unwrap();
        assert!(d.contains("(PROTOCOL=TCPS)"));
        assert!(d.contains("(MY_WALLET_DIRECTORY=/wallet)"));
        assert!(d.contains("(SSL_SERVER_DN_MATCH=no)"));
    }
}
