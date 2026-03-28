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

fn missing_parquet_path() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/does-not-exist.parquet");
    root.to_string_lossy().to_string()
}

fn auth_header_value(user: &str, password: &str) -> String {
    let token = STANDARD.encode(format!("{user}:{password}"));
    format!("Basic {token}")
}

fn bearer_header_value(token: &str) -> String {
    format!("Bearer {token}")
}

#[tokio::test]
async fn rejects_invalid_sql_with_400() {
    let app = opensnow_server::app();
    let auth = auth_header_value("admin", "password");
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", auth)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"statement":";;;"}"#))
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn submit_then_poll_succeeds_with_fixture() {
    let app = opensnow_server::app();
    let auth = auth_header_value("admin", "password");
    let sql = format!(
        "SELECT * FROM parquet_scan('{}') LIMIT 5",
        sample_parquet_path()
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", &auth)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "statement": sql }).to_string(),
        ))
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
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

    for _ in 0..20 {
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
            let row_count = polled.get("rowCount").and_then(Value::as_u64).unwrap_or(0);
            assert!(row_count > 0);
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("statement did not reach succeeded status");
}

#[tokio::test]
async fn submit_then_poll_fails_for_missing_parquet() {
    let app = opensnow_server::app();
    let auth = auth_header_value("admin", "password");
    let sql = format!(
        "SELECT * FROM parquet_scan('{}') LIMIT 5",
        missing_parquet_path()
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", &auth)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "statement": sql }).to_string(),
        ))
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
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

    for _ in 0..20 {
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
        if status == "failed" {
            let sql_state = polled.get("sqlState").and_then(Value::as_str).unwrap_or("");
            let message = polled.get("message").and_then(Value::as_str).unwrap_or("");
            assert_eq!(sql_state, "XX000");
            assert!(!message.is_empty());
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("statement did not reach failed status");
}

#[tokio::test]
async fn rejects_missing_auth_with_401() {
    let app = opensnow_server::app();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"statement":"select 1"}"#))
        .expect("request");

    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn session_login_returns_token_and_bearer_can_query() {
    let app = opensnow_server::app();
    let login_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/session")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "admin",
                "password": "password",
                "role": "ACCOUNTADMIN"
            })
            .to_string(),
        ))
        .expect("login request");
    let login_resp = app.clone().oneshot(login_req).await.expect("login response");
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_body = to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .expect("login body");
    let login_json: Value = serde_json::from_slice(&login_body).expect("login json");
    let token = login_json
        .get("sessionToken")
        .and_then(Value::as_str)
        .expect("sessionToken");

    let auth = bearer_header_value(token);
    let stmt_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", auth)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"statement":"SELECT 1 AS n"}"#))
        .expect("statement request");
    let stmt_resp = app.oneshot(stmt_req).await.expect("statement response");
    assert_eq!(stmt_resp.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn revoked_session_token_is_rejected() {
    let app = opensnow_server::app();
    let login_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/session")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "admin",
                "password": "password",
            })
            .to_string(),
        ))
        .expect("login request");
    let login_resp = app.clone().oneshot(login_req).await.expect("login response");
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_body = to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .expect("login body");
    let login_json: Value = serde_json::from_slice(&login_body).expect("login json");
    let token = login_json
        .get("sessionToken")
        .and_then(Value::as_str)
        .expect("sessionToken")
        .to_string();

    let revoke_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v2/session/{token}"))
        .body(Body::empty())
        .expect("revoke request");
    let revoke_resp = app.clone().oneshot(revoke_req).await.expect("revoke response");
    assert_eq!(revoke_resp.status(), StatusCode::OK);

    let auth = bearer_header_value(&token);
    let stmt_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", auth)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"statement":"select 1"}"#))
        .expect("statement request");
    let stmt_resp = app.oneshot(stmt_req).await.expect("statement response");
    assert_eq!(stmt_resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_session_token_is_rejected() {
    let app = opensnow_server::app();
    let login_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/session")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "admin",
                "password": "password",
                "sessionTtlSeconds": 1
            })
            .to_string(),
        ))
        .expect("login request");
    let login_resp = app.clone().oneshot(login_req).await.expect("login response");
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_body = to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .expect("login body");
    let login_json: Value = serde_json::from_slice(&login_body).expect("login json");
    let token = login_json
        .get("sessionToken")
        .and_then(Value::as_str)
        .expect("sessionToken");

    tokio::time::sleep(std::time::Duration::from_millis(2200)).await;

    let auth = bearer_header_value(token);
    let stmt_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v2/statements")
        .header("authorization", auth)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"statement":"select 1"}"#))
        .expect("statement request");
    let stmt_resp = app.oneshot(stmt_req).await.expect("statement response");
    assert_eq!(stmt_resp.status(), StatusCode::UNAUTHORIZED);
}
