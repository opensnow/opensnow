use axum::body::{to_bytes, Body};
use base64::{engine::general_purpose::STANDARD, Engine};
use http::{Method, Request, StatusCode};
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

fn sample_parquet_path() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/sample.parquet");
    root.to_string_lossy().to_string()
}

fn auth_header_value(user: &str, password: &str) -> String {
    let token = STANDARD.encode(format!("{user}:{password}"));
    format!("Basic {token}")
}

async fn submit_and_poll(
    app: axum::Router,
    auth: String,
    sql: String,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", &auth)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({ "statement": sql }).to_string()))
        .expect("request");

    let resp = app.clone().oneshot(req).await.expect("statement response");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let submitted: Value = serde_json::from_slice(&body).expect("json");
    let handle = submitted
        .get("statementHandle")
        .and_then(Value::as_str)
        .expect("statementHandle")
        .to_string();

    for _ in 0..50 {
        let poll_req = Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v2/statements/{handle}"))
            .header("authorization", &auth)
            .body(Body::empty())
            .expect("poll request");

        let poll_resp = app.clone().oneshot(poll_req).await.expect("poll response");
        assert_eq!(poll_resp.status(), StatusCode::OK);

        let poll_body = to_bytes(poll_resp.into_body(), usize::MAX)
            .await
            .expect("poll body");
        let polled: Value = serde_json::from_slice(&poll_body).expect("poll json");

        let status = polled
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if status == "succeeded" {
            return (StatusCode::OK, polled);
        }

        if status == "failed" {
            return (StatusCode::OK, polled);
        }

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    panic!("statement did not reach terminal status");
}

fn assert_statement_succeeded(polled: &Value) {
    assert_eq!(
        polled.get("status").and_then(Value::as_str),
        Some("succeeded"),
        "expected succeeded, got: {polled:?}"
    );
}

#[tokio::test]
async fn phase1_create_insert_select_and_copy_into() {
    let app = opensnow_server::app();
    let auth = auth_header_value("admin", "password");

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let db = format!("db_{suffix}");
    let schema = "s1";
    let t_insert = "t_insert";
    let t_copy = "t_copy";

    // CREATE DATABASE + SCHEMA
    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!("CREATE DATABASE {db}"),
    )
    .await;
    assert_statement_succeeded(&polled);

    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!("CREATE SCHEMA {db}.{schema}"),
    )
    .await;
    assert_statement_succeeded(&polled);

    // CREATE TABLE + INSERT + SELECT
    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!(
            "CREATE TABLE {db}.{schema}.{t_insert} (id INT, name VARCHAR)"
        ),
    )
    .await;
    assert_statement_succeeded(&polled);

    for (id, name) in [(1, "alice"), (2, "bob")] {
        let (_status, polled) = submit_and_poll(
            app.clone(),
            auth.clone(),
            format!(
                "INSERT INTO {db}.{schema}.{t_insert} VALUES ({id}, '{name}')"
            ),
        )
        .await;
        assert_statement_succeeded(&polled);
    }

    // INSERT ... SELECT (DataFusion-supported subset)
    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!(
            "INSERT INTO {db}.{schema}.{t_insert} SELECT id + 10, name FROM {db}.{schema}.{t_insert} WHERE id = 1"
        ),
    )
    .await;
    assert_statement_succeeded(&polled);

    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!(
            "SELECT id, name FROM {db}.{schema}.{t_insert} ORDER BY id"
        ),
    )
    .await;
    assert_statement_succeeded(&polled);

    let data = polled.get("data").and_then(Value::as_array).expect("data array");
    assert_eq!(data.len(), 3);
    assert_eq!(data[0].get("id").and_then(Value::as_str), Some("1"));
    assert_eq!(data[0].get("name").and_then(Value::as_str), Some("alice"));
    assert_eq!(data[1].get("id").and_then(Value::as_str), Some("2"));
    assert_eq!(data[1].get("name").and_then(Value::as_str), Some("bob"));
    assert_eq!(data[2].get("id").and_then(Value::as_str), Some("11"));
    assert_eq!(data[2].get("name").and_then(Value::as_str), Some("alice"));

    // COPY INTO parquet -> inferred all-VARCHAR MemTable
    let parquet_path = sample_parquet_path();
    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!(
            "COPY INTO {db}.{schema}.{t_copy} FROM @dummy FILES = ('{parquet_path}')"
        ),
    )
    .await;
    assert_statement_succeeded(&polled);

    // Smoke: ensure at least one row is visible
    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!("SELECT * FROM {db}.{schema}.{t_copy} LIMIT 1"),
    )
    .await;
    assert_statement_succeeded(&polled);

    let data = polled.get("data").and_then(Value::as_array).expect("data array");
    assert_eq!(data.len(), 1);
    let row_obj = data[0].as_object().expect("row object");
    assert!(!row_obj.is_empty());

    // DROP TABLE / SCHEMA / DATABASE (teardown path exercised in integration)
    for table in [t_copy, t_insert] {
        let (_status, polled) = submit_and_poll(
            app.clone(),
            auth.clone(),
            format!("DROP TABLE IF EXISTS {db}.{schema}.{table}"),
        )
        .await;
        assert_statement_succeeded(&polled);
    }

    let (_status, polled) = submit_and_poll(
        app.clone(),
        auth.clone(),
        format!("DROP SCHEMA IF EXISTS {db}.{schema}"),
    )
    .await;
    assert_statement_succeeded(&polled);

    let (_status, polled) = submit_and_poll(
        app,
        auth,
        format!("DROP DATABASE IF EXISTS {db}"),
    )
    .await;
    assert_statement_succeeded(&polled);
}

