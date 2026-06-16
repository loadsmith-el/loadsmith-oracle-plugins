use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use loadsmith_plugin_sdk::DestinationPlugin;
use oracle::sql_type::OracleType;
use oracle::Connection;
use serde::Deserialize;

use crate::conn::ConnectionConfig;
use crate::types::{cell_to_bind, quote_ident, typed_expr, Bind};

#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CommitMode {
    /// One transaction for the whole load; array-`INSERT` straight into the
    /// target, `COMMIT` at the end. All-or-nothing, at-least-once.
    #[default]
    Atomic,
    /// Array-`MERGE` each row into the target by `merge_key` (`WHEN MATCHED`
    /// update / `WHEN NOT MATCHED` insert), `COMMIT` at the end. Idempotent by
    /// the key ⇒ exactly-once effective. No staging table (Oracle DDL would
    /// auto-commit), so the whole load stays in one transaction.
    StagedMerge,
}

#[derive(Debug, Deserialize)]
struct OracleDestConfig {
    #[serde(flatten)]
    conn: ConnectionConfig,

    /// Target table.
    target_table: String,
    #[serde(default)]
    mode: CommitMode,
    /// Key columns for `staged_merge`'s `ON (...)` match. Must be a unique key
    /// on the target.
    #[serde(default)]
    merge_key: Vec<String>,
}

pub struct OracleDestPlugin {
    config: Option<OracleDestConfig>,
    conn: Option<Connection>,
    columns: Vec<String>,
    /// Target column → Oracle type (upper-cased names), from describing the
    /// target table in `prepare`. Drives the explicit `TO_*` bind wrapping.
    target_types: HashMap<String, OracleType>,
    /// Per-row bind SQL, built once the column set is known (first batch).
    sql: Option<String>,
    rows_written: u64,
}

impl Default for OracleDestPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OracleDestPlugin {
    pub fn new() -> Self {
        Self {
            config: None,
            conn: None,
            columns: Vec::new(),
            target_types: HashMap::new(),
            sql: None,
            rows_written: 0,
        }
    }

    fn cfg(&self) -> &OracleDestConfig {
        self.config.as_ref().expect("configured")
    }

    /// The explicit bind expression for one incoming column at placeholder `:idx`
    /// — `TO_DATE(:idx, …)` / `TO_TIMESTAMP*(:idx, …)` for temporal target
    /// columns, plain `:idx` otherwise (unknown columns pass through).
    fn bind_expr(&self, col: &str, idx: usize) -> String {
        let placeholder = format!(":{idx}");
        match self.target_types.get(&col.to_uppercase()) {
            Some(t) => typed_expr(&placeholder, t),
            None => placeholder,
        }
    }
}

#[async_trait]
impl DestinationPlugin for OracleDestPlugin {
    fn plugin_name(&self) -> &str {
        "loadsmith-destination-oracle"
    }
    fn plugin_version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["batch_write".into(), "staged_merge".into()]
    }

    async fn configure(&mut self, config: serde_json::Value) -> Result<()> {
        let cfg: OracleDestConfig =
            serde_json::from_value(config).context("invalid oracle destination config")?;
        if cfg.mode == CommitMode::StagedMerge && cfg.merge_key.is_empty() {
            bail!("mode 'staged_merge' requires a non-empty merge_key");
        }
        self.config = Some(cfg);
        Ok(())
    }

    async fn prepare(&mut self) -> Result<()> {
        // Nothing is durable until finalize() commits (Oracle holds the
        // transaction open implicitly).
        let describe = format!("SELECT * FROM {} WHERE 1 = 0", quote_ident(&self.cfg().target_table));

        let (conn, target_types) = tokio::task::block_in_place(|| -> Result<_> {
            let conn = crate::conn::connect(&self.cfg().conn)?;
            let rs = conn
                .query(&describe, &[])
                .map_err(|e| anyhow::anyhow!("describing target table failed: {e}"))?;
            let target_types: HashMap<String, OracleType> = rs
                .column_info()
                .iter()
                .map(|c| (c.name().to_uppercase(), c.oracle_type().clone()))
                .collect();
            Ok((conn, target_types))
        })?;

        self.target_types = target_types;
        self.conn = Some(conn);
        Ok(())
    }

    async fn write_batch(&mut self, batch: RecordBatch) -> Result<()> {
        if self.columns.is_empty() {
            self.columns = batch.schema().fields().iter().map(|f| f.name().clone()).collect();
            // Per-column bind expressions (temporal columns wrapped in TO_*).
            let binds: Vec<String> =
                self.columns.iter().enumerate().map(|(i, c)| self.bind_expr(c, i + 1)).collect();
            self.sql = Some(match self.cfg().mode {
                CommitMode::Atomic => build_insert_sql(&self.cfg().target_table, &self.columns, &binds),
                CommitMode::StagedMerge => build_merge_sql(
                    &self.cfg().target_table,
                    &self.columns,
                    &binds,
                    &self.cfg().merge_key,
                )?,
            });
        }
        let nrows = batch.num_rows();
        if nrows == 0 {
            return Ok(());
        }

        // Materialise all binds (owned) up front, then run the array DML.
        let mut rows: Vec<Vec<Bind>> = Vec::with_capacity(nrows);
        for row in 0..nrows {
            let mut cells: Vec<Bind> = Vec::with_capacity(batch.num_columns());
            for col in 0..batch.num_columns() {
                cells.push(cell_to_bind(batch.column(col), row)?);
            }
            rows.push(cells);
        }

        let sql = self.sql.as_ref().unwrap().clone();
        let conn = self.conn.as_ref().context("not prepared")?;
        tokio::task::block_in_place(|| -> Result<()> {
            let mut dml =
                conn.batch(&sql, nrows).build().map_err(|e| anyhow::anyhow!("batch build failed: {e}"))?;
            for row in &rows {
                let params: Vec<&dyn oracle::sql_type::ToSql> =
                    row.iter().map(|b| b.as_sql()).collect();
                dml.append_row(&params).map_err(|e| anyhow::anyhow!("append_row failed: {e}"))?;
            }
            dml.execute().map_err(|e| anyhow::anyhow!("array DML failed: {e}"))?;
            Ok(())
        })?;

        self.rows_written += nrows as u64;
        Ok(())
    }

    async fn finalize(&mut self) -> Result<u64> {
        let conn = self.conn.as_ref().context("not prepared")?;
        tokio::task::block_in_place(|| conn.commit().map_err(|e| anyhow::anyhow!("COMMIT failed: {e}")))?;
        Ok(self.rows_written)
    }

    async fn cancel(&mut self) {
        if let Some(conn) = self.conn.as_ref() {
            let _ = tokio::task::block_in_place(|| conn.rollback());
        }
    }
}

/// `INSERT INTO "target" ("c1", …) VALUES (<bind1>, …)`, where each `<bind>` is
/// the column's (possibly `TO_*`-wrapped) bind expression.
fn build_insert_sql(target: &str, columns: &[String], binds: &[String]) -> String {
    let col_list = columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let values = binds.join(", ");
    format!("INSERT INTO {} ({col_list}) VALUES ({values})", quote_ident(target))
}

/// `MERGE INTO "target" t USING (SELECT <bind1> AS "c1", … FROM dual) s ON (key
/// match) WHEN MATCHED THEN UPDATE SET <non-key> WHEN NOT MATCHED THEN INSERT
/// (...) VALUES (...)`. Oracle's upsert; one row per bind set under array DML.
fn build_merge_sql(
    target: &str,
    columns: &[String],
    binds: &[String],
    merge_key: &[String],
) -> Result<String> {
    for k in merge_key {
        if !columns.contains(k) {
            bail!("merge_key column '{k}' is not present in the incoming data");
        }
    }
    let src = columns
        .iter()
        .zip(binds)
        .map(|(c, bind)| format!("{bind} AS {}", quote_ident(c)))
        .collect::<Vec<_>>()
        .join(", ");
    let on = merge_key
        .iter()
        .map(|k| format!("t.{0} = s.{0}", quote_ident(k)))
        .collect::<Vec<_>>()
        .join(" AND ");
    let col_list = columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let src_list = columns.iter().map(|c| format!("s.{}", quote_ident(c))).collect::<Vec<_>>().join(", ");

    let non_key: Vec<&String> = columns.iter().filter(|c| !merge_key.contains(c)).collect();
    let matched = if non_key.is_empty() {
        // All columns are keys → nothing to update; only insert new rows.
        String::new()
    } else {
        let set = non_key
            .iter()
            .map(|c| format!("t.{0} = s.{0}", quote_ident(c)))
            .collect::<Vec<_>>()
            .join(", ");
        format!(" WHEN MATCHED THEN UPDATE SET {set}")
    };

    Ok(format!(
        "MERGE INTO {target} t USING (SELECT {src} FROM dual) s ON ({on}){matched} \
         WHEN NOT MATCHED THEN INSERT ({col_list}) VALUES ({src_list})",
        target = quote_ident(target),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn config_requires_merge_key_for_staged_merge() {
        let mut p = OracleDestPlugin::new();
        assert!(p
            .configure(serde_json::json!({
                "service_name": "S", "user": "u", "password": "p",
                "target_table": "events", "mode": "staged_merge"
            }))
            .await
            .is_err());
        assert!(p
            .configure(serde_json::json!({
                "service_name": "S", "user": "u", "password": "p",
                "target_table": "events", "mode": "staged_merge", "merge_key": ["id"]
            }))
            .await
            .is_ok());
    }

    #[test]
    fn default_mode_is_atomic() {
        let json = serde_json::json!({
            "service_name": "S", "user": "u", "password": "p", "target_table": "t"
        });
        let cfg: OracleDestConfig = serde_json::from_value(json).unwrap();
        assert_eq!(cfg.mode, CommitMode::Atomic);
    }

    // Plain positional binds (no temporal columns): bind == ":idx".
    fn plain_binds(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!(":{i}")).collect()
    }

    #[test]
    fn insert_sql_positional_binds() {
        let cols = vec!["ID".to_string(), "NAME".to_string()];
        assert_eq!(
            build_insert_sql("EVENTS", &cols, &plain_binds(2)),
            "INSERT INTO \"EVENTS\" (\"ID\", \"NAME\") VALUES (:1, :2)"
        );
    }

    #[test]
    fn insert_sql_wraps_temporal_binds() {
        let cols = vec!["ID".to_string(), "TS".to_string()];
        let binds = vec![":1".to_string(), "TO_TIMESTAMP(:2, 'YYYY-MM-DD HH24:MI:SS.FF6')".to_string()];
        assert_eq!(
            build_insert_sql("EVENTS", &cols, &binds),
            "INSERT INTO \"EVENTS\" (\"ID\", \"TS\") VALUES (:1, TO_TIMESTAMP(:2, 'YYYY-MM-DD HH24:MI:SS.FF6'))"
        );
    }

    #[test]
    fn merge_sql_updates_non_key() {
        let cols = vec!["ID".to_string(), "NAME".to_string(), "TS".to_string()];
        let keys = vec!["ID".to_string()];
        let sql = build_merge_sql("EVENTS", &cols, &plain_binds(3), &keys).unwrap();
        assert!(sql.contains("USING (SELECT :1 AS \"ID\", :2 AS \"NAME\", :3 AS \"TS\" FROM dual) s"));
        assert!(sql.contains("ON (t.\"ID\" = s.\"ID\")"));
        assert!(sql.contains("WHEN MATCHED THEN UPDATE SET t.\"NAME\" = s.\"NAME\", t.\"TS\" = s.\"TS\""));
        assert!(sql.contains("WHEN NOT MATCHED THEN INSERT (\"ID\", \"NAME\", \"TS\") VALUES (s.\"ID\", s.\"NAME\", s.\"TS\")"));
    }

    #[test]
    fn merge_sql_all_keys_inserts_only() {
        let cols = vec!["A".to_string(), "B".to_string()];
        let keys = vec!["A".to_string(), "B".to_string()];
        let sql = build_merge_sql("T", &cols, &plain_binds(2), &keys).unwrap();
        assert!(!sql.contains("WHEN MATCHED"));
        assert!(sql.contains("WHEN NOT MATCHED THEN INSERT"));
    }

    #[test]
    fn merge_sql_rejects_unknown_key() {
        let cols = vec!["A".to_string()];
        assert!(build_merge_sql("T", &cols, &plain_binds(1), &["B".to_string()]).is_err());
    }
}
