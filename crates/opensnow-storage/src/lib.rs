//! # opensnow-storage
//!
//! Storage layer for OpenSnow. Responsible for:
//!
//! - Reading and writing Apache Parquet files to/from AWS S3 (and
//!   S3-compatible stores such as MinIO for local development).
//! - Implementing the Apache Iceberg table format on top of Parquet:
//!   micro-partition management, snapshot commits, manifest files, and
//!   metadata JSON.
//! - Exposing a `TableIO` abstraction consumed by `opensnow-compute` so
//!   the query engine can scan and write data without knowing about S3 paths.
//! - Providing column statistics (min/max per micro-partition) for predicate
//!   pushdown and partition pruning in the query planner.
//!
//! ## Storage model
//!
//! Data is organized as follows on S3:
//!
//! ```text
//! s3://<bucket>/
//!   <database>/
//!     <schema>/
//!       <table>/
//!         metadata/
//!           v1.metadata.json       ← Iceberg table metadata
//!           snap-<id>.avro         ← Iceberg snapshot manifest list
//!         data/
//!           <partition>/
//!             <uuid>.parquet       ← Micro-partition data files
//! ```
//!
//! ## Planned modules
//!
//! - `s3`        — `object_store` wrapper for AWS S3 and MinIO
//! - `parquet`   — Parquet reader/writer with Arrow schema integration
//! - `iceberg`   — Iceberg table format: snapshots, manifests, metadata
//! - `stats`     — Column-level statistics extraction for pruning

use opensnow_common::{OpenSnowError, Result};
use parquet::{
    file::reader::{FileReader, SerializedFileReader},
    record::Field,
};
use serde_json::{Map, Value};
use std::{fs::File, path::Path};

/// Read rows from a local parquet file and return them as JSON objects.
pub fn read_local_parquet_rows(path: impl AsRef<Path>, limit: usize) -> Result<Vec<Value>> {
    let file = File::open(path.as_ref())?;
    let reader = SerializedFileReader::new(file).map_err(|e| OpenSnowError::Storage(e.to_string()))?;
    let mut iter = reader
        .get_row_iter(None)
        .map_err(|e| OpenSnowError::Storage(e.to_string()))?;

    let mut rows = Vec::new();
    for _ in 0..limit {
        let Some(row) = iter.next() else {
            break;
        };
        let row = row.map_err(|e| OpenSnowError::Storage(e.to_string()))?;
        let mut obj = Map::new();
        for (name, field) in row.get_column_iter() {
            obj.insert(name.to_string(), parquet_field_to_json(field));
        }
        rows.push(Value::Object(obj));
    }

    Ok(rows)
}

fn parquet_field_to_json(field: &Field) -> Value {
    if matches!(field, Field::Null) {
        Value::Null
    } else {
        Value::from(field.to_string())
    }
}
