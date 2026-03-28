//! DataFusion-backed session used for Phase 1 SQL (DDL/DML/SELECT) plus legacy `parquet_scan`.

use datafusion::arrow::array::{ArrayRef, StringArray};
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::arrow::util::display::array_value_to_string;
use datafusion::catalog::memory::MemoryCatalogProviderList;
use datafusion::prelude::SessionContext;
use datafusion::datasource::MemTable;
use opensnow_common::{OpenSnowError, Result};
use opensnow_sql::{
    parse_one_statement, parse_select_parquet_scan, CopyIntoSnowflakeKind, ObjectType, Statement,
};
use serde_json::{Map, Value};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;

static SESSION: OnceLock<Mutex<SessionContext>> = OnceLock::new();

fn session() -> &'static Mutex<SessionContext> {
    SESSION.get_or_init(|| Mutex::new(SessionContext::new()))
}

fn batches_to_json(batches: &[RecordBatch]) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for batch in batches {
        for row in 0..batch.num_rows() {
            let mut map = Map::new();
            for (i, field) in batch.schema().fields().iter().enumerate() {
                let col = batch.column(i);
                let s = array_value_to_string(col, row)
                    .map_err(|e| OpenSnowError::Execution(e.to_string()))?;
                map.insert(field.name().clone(), Value::String(s));
            }
            rows.push(Value::Object(map));
        }
    }
    Ok(rows)
}

fn drop_database(ctx: &SessionContext, name: &str, if_exists: bool) -> Result<()> {
    let state = ctx.state_ref();
    let guard = state.read();
    let default_cat = guard.config_options().catalog.default_catalog.clone();
    if name == default_cat.as_str() {
        return Err(OpenSnowError::InvalidStatement(
            "cannot DROP DATABASE for the default catalog".to_string(),
        ));
    }
    let list = guard.catalog_list();
    let mem = list
        .as_any()
        .downcast_ref::<MemoryCatalogProviderList>()
        .ok_or_else(|| {
            OpenSnowError::Execution("DROP DATABASE requires an in-memory catalog list".to_string())
        })?;
    let removed = mem.catalogs.remove(name);
    match removed {
        Some(_) => Ok(()),
        None if if_exists => Ok(()),
        None => Err(OpenSnowError::InvalidStatement(format!(
            "database '{name}' does not exist"
        ))),
    }
}

async fn copy_into_table(
    ctx: &SessionContext,
    stmt: &Statement,
) -> Result<()> {
    let Statement::CopyIntoSnowflake {
        kind,
        into,
        files,
        ..
    } = stmt
    else {
        return Err(OpenSnowError::Execution(
            "internal: expected COPY INTO (Snowflake)".to_string(),
        ));
    };
    if *kind != CopyIntoSnowflakeKind::Table {
        return Err(OpenSnowError::InvalidStatement(
            "COPY INTO <location> is not supported yet".to_string(),
        ));
    }
    let paths = files.as_ref().ok_or_else(|| {
        OpenSnowError::InvalidStatement(
            "COPY INTO requires FILES = ('/path/to/file.parquet') for local loads".to_string(),
        )
    })?;
    if paths.is_empty() {
        return Err(OpenSnowError::InvalidStatement(
            "FILES list must not be empty".to_string(),
        ));
    }

    // Phase 1 approach:
    // - Load local parquet files via `opensnow-storage` (values become strings)
    // - Create an in-memory `MemTable` with all-`Utf8` columns if missing
    // - Append/update semantics are intentionally not implemented yet
    let dest_full_name = into.to_string();

    // TableReference parsing rules:
    // - `catalog.schema.table` (3 parts)
    // - `schema.table` (2 parts; catalog defaults)
    // - `table` (1 part; catalog + schema default)
    let table_ref = parse_table_reference(ctx, &dest_full_name)?;
    if ctx.table(table_ref.clone()).await.is_ok() {
        return Err(OpenSnowError::InvalidStatement(format!(
            "COPY INTO append is not implemented yet; table '{dest_full_name}' already exists"
        )));
    }

    let mut all_rows: Vec<Value> = Vec::new();
    for path in paths {
        let rows = opensnow_storage::read_local_parquet_rows(path, 1_000_000).map_err(|e| {
            match e {
                OpenSnowError::Storage(_) | OpenSnowError::Io(_) => e,
                other => OpenSnowError::Execution(other.to_string()),
            }
        })?;
        all_rows.extend(rows);
    }

    // Infer column set from the first row that has an object payload.
    let first_obj = all_rows.iter().find_map(|v| v.as_object());
    let Some(first_obj) = first_obj else {
        return Err(OpenSnowError::InvalidStatement(
            "COPY INTO parquet file produced no rows".to_string(),
        ));
    };
    let mut column_names: Vec<String> = first_obj.keys().cloned().collect();
    column_names.sort();

    let schema = Arc::new(Schema::new(
        column_names
            .iter()
            .map(|name| Field::new(name, DataType::Utf8, true))
            .collect::<Vec<_>>(),
    ));

    let mut arrays: Vec<ArrayRef> = Vec::new();
    for col in column_names.iter() {
        let mut values: Vec<Option<String>> = Vec::new();
        values.reserve(all_rows.len());
        for row in all_rows.iter() {
            match row {
                Value::Object(map) => {
                    let v = map.get(col);
                    if v.is_none() || matches!(v, Some(Value::Null)) {
                        values.push(None);
                    } else {
                        values.push(v.and_then(Value::as_str).map(|s| s.to_string()));
                    }
                }
                _ => values.push(None),
            }
        }

        let array = StringArray::from(values);
        arrays.push(Arc::new(array));
    }

    let record_batch = RecordBatch::try_new(schema.clone(), arrays).map_err(|e| {
        OpenSnowError::Execution(format!("failed to build Arrow batch for COPY INTO: {e}"))
    })?;

    let mem_table = Arc::new(
        MemTable::try_new(schema.clone() as SchemaRef, vec![vec![record_batch]])
            .map_err(|e| OpenSnowError::Execution(e.to_string()))?,
    );

    ctx.register_table(table_ref, mem_table)
        .map(|_| ())
        .map_err(|e| OpenSnowError::Execution(e.to_string()))
}

fn parse_table_reference(ctx: &SessionContext, name: &str) -> Result<datafusion_common::TableReference> {
    use datafusion_common::TableReference;

    let state = ctx.state_ref().clone();
    let guard = state.read();
    let default_catalog = guard.config_options().catalog.default_catalog.clone();
    let default_schema = guard.config_options().catalog.default_schema.clone();

    let parts: Vec<&str> = name.split('.').filter(|p| !p.is_empty()).collect();
    let (catalog, schema, table) = match parts.len() {
        3 => (parts[0], parts[1], parts[2]),
        2 => (default_catalog.as_str(), parts[0], parts[1]),
        1 => (default_catalog.as_str(), default_schema.as_str(), parts[0]),
        _ => {
            return Err(OpenSnowError::InvalidStatement(format!(
                "invalid table reference '{name}'; expected 1-3 dot-separated parts"
            )))
        }
    };

    Ok(TableReference::full(catalog, schema, table))
}

/// Execute SQL: legacy `parquet_scan`, Snowflake `COPY INTO`, custom `DROP DATABASE`, or DataFusion
/// (DDL/DML/`DROP TABLE`/`DROP SCHEMA`/queries).
pub async fn execute_sql(sql: &str) -> Result<Vec<Value>> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err(OpenSnowError::InvalidStatement("empty statement".into()));
    }

    if let Ok(parsed) = parse_select_parquet_scan(sql) {
        return opensnow_storage::read_local_parquet_rows(parsed.path, parsed.limit).map_err(|e| {
            match e {
                OpenSnowError::Storage(_) | OpenSnowError::Io(_) => e,
                other => OpenSnowError::Execution(other.to_string()),
            }
        });
    }

    let stmt = parse_one_statement(sql)?;
    let ctx = session().lock().await;
    let sql_run = sql.trim().trim_end_matches(';').trim();

    match &stmt {
        Statement::Drop {
            object_type: ObjectType::Database,
            if_exists,
            names,
            ..
        } => {
            if names.len() != 1 {
                return Err(OpenSnowError::InvalidStatement(
                    "DROP DATABASE expects a single name".to_string(),
                ));
            }
            let name = names[0].to_string();
            drop_database(&ctx, &name, *if_exists)?;
            Ok(vec![])
        }
        Statement::CopyIntoSnowflake { .. } => {
            copy_into_table(&ctx, &stmt).await?;
            Ok(vec![])
        }
        _ => {
            let df = ctx
                .sql(sql_run)
                .await
                .map_err(|e| OpenSnowError::Execution(e.to_string()))?;
            let batches = df
                .collect()
                .await
                .map_err(|e| OpenSnowError::Execution(e.to_string()))?;
            batches_to_json(&batches)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn select_literal_returns_row() {
        // Fresh session for this test would require isolation; we only assert shape.
        let rows = execute_sql("SELECT 1 AS n").await.unwrap();
        assert_eq!(rows.len(), 1);
        let row = rows[0].as_object().unwrap();
        assert!(row.contains_key("n"));
    }
}
