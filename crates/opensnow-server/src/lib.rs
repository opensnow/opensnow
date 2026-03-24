use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use opensnow_auth::{
    fetch_jwks_json, hash_password, issue_session_token, verify_oidc_access_token_with_jwks_json,
    verify_password, verify_session_token_with_secrets, SessionContext,
};
use opensnow_common::{AuthMode, ServerConfig};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::RwLock};

#[derive(Clone)]
struct AppState {
    statements: Arc<RwLock<HashMap<String, StatementEntry>>>,
    next_statement_id: Arc<AtomicU64>,
    users: Arc<HashMap<String, String>>,
    session_secrets: Arc<Vec<String>>,
    revoked_jtis: Arc<RwLock<HashSet<String>>>,
    auth_mode: AuthMode,
    oidc_jwks_url: Option<String>,
    oidc_issuer: Option<String>,
    oidc_audience: Option<String>,
    oidc_jwks_cache_ttl_seconds: u64,
    oidc_jwks_cache: Arc<RwLock<Option<JwksCacheEntry>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::from_config(&ServerConfig::default())
            .expect("failed to build default app state")
    }
}

impl AppState {
    fn from_config(config: &ServerConfig) -> anyhow::Result<Self> {
        let mut users = HashMap::new();
        let bootstrap_hash = hash_password(&config.bootstrap_password)
            .map_err(|e| anyhow::anyhow!("failed to hash bootstrap password: {e}"))?;
        users.insert(config.bootstrap_user.clone(), bootstrap_hash);
        let mut session_secrets = vec![config.session_secret.clone()];
        if let Some(previous) = &config.previous_session_secret {
            if previous != &config.session_secret {
                session_secrets.push(previous.clone());
            }
        }
        Ok(Self {
            statements: Arc::new(RwLock::new(HashMap::new())),
            next_statement_id: Arc::new(AtomicU64::new(0)),
            users: Arc::new(users),
            session_secrets: Arc::new(session_secrets),
            revoked_jtis: Arc::new(RwLock::new(HashSet::new())),
            auth_mode: config.auth_mode,
            oidc_jwks_url: config.oidc_jwks_url.clone(),
            oidc_issuer: config.oidc_issuer.clone(),
            oidc_audience: config.oidc_audience.clone(),
            oidc_jwks_cache_ttl_seconds: config.oidc_jwks_cache_ttl_seconds,
            oidc_jwks_cache: Arc::new(RwLock::new(None)),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum StatementStatus {
    Running,
    Succeeded,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone)]
struct StatementEntry {
    status: StatementStatus,
    rows: Option<Vec<serde_json::Value>>,
    error: Option<String>,
    created_at_epoch_ms: u128,
    updated_at_epoch_ms: u128,
}

#[derive(Debug, Serialize)]
struct NotImplementedResponse<'a> {
    message: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostStatementRequest {
    statement: String,
    timeout: Option<u64>,
    warehouse: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    role: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PostStatementResponse {
    statement_handle: String,
    status: &'static str,
    message: &'static str,
    poll_url: String,
    created_at_epoch_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GetStatementResponse {
    statement_handle: String,
    status: &'static str,
    sql_state: &'static str,
    error_code: Option<&'static str>,
    message: String,
    data: Option<Vec<serde_json::Value>>,
    row_count: Option<usize>,
    created_at_epoch_ms: u128,
    updated_at_epoch_ms: u128,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionRequest {
    username: String,
    password: String,
    role: Option<String>,
    warehouse: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    session_ttl_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    session_token: String,
    status: &'static str,
    session: SessionContext,
    expires_at_epoch_ms: u128,
}

#[derive(Debug, Serialize)]
struct LogoutResponse<'a> {
    message: &'a str,
}

#[derive(Debug, Clone)]
struct JwksCacheEntry {
    jwks_json: String,
    fetched_at_epoch_secs: u64,
}

pub fn app() -> Router {
    build_app(AppState::default())
}

pub async fn run(config: ServerConfig) -> anyhow::Result<()> {
    let app_state = AppState::from_config(&config)?;
    let app = build_app(app_state);
    let listener = TcpListener::bind(("0.0.0.0", config.http_port)).await?;
    tracing::info!(port = config.http_port, "HTTP server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_app(app_state: AppState) -> Router {
    let api_v2 = Router::new()
        .route("/session", post(post_session))
        .route("/session/{token}", axum::routing::delete(delete_session))
        .route("/statements", post(post_statements))
        .route("/statements/{handle}", get(get_statement))
        .route("/statements/{handle}/cancel", post(cancel_statement))
        .with_state(app_state);

    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v2", api_v2)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn post_session(
    State(state): State<AppState>,
    Json(payload): Json<SessionRequest>,
) -> impl IntoResponse {
    if state.auth_mode == AuthMode::Oidc {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(ErrorResponse {
                message: "Session login endpoint is disabled in oidc auth mode".to_string(),
            }),
        )
            .into_response();
    }

    let Some(stored_hash) = state.users.get(&payload.username) else {
        return unauthorized_response();
    };
    let Ok(valid) = verify_password(&payload.password, stored_hash) else {
        return unauthorized_response();
    };
    if !valid {
        return unauthorized_response();
    }

    let session = SessionContext {
        account: Some("local".to_string()),
        user: Some(payload.username),
        role: payload.role.or_else(|| Some("PUBLIC".to_string())),
        warehouse: payload.warehouse,
        database: payload.database,
        schema: payload.schema,
    };
    let ttl_secs = payload.session_ttl_seconds.unwrap_or(3600);
    let now_secs = now_epoch_secs();
    let token = match issue_session_token(&state.session_secrets[0], &session, now_secs, ttl_secs) {
        Ok(token) => token,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    message: "Failed to create session token".to_string(),
                }),
            )
                .into_response();
        }
    };
    let ttl_ms = ttl_secs as u128 * 1000;
    let expires_at_epoch_ms = now_epoch_ms() + ttl_ms;

    let response = SessionResponse {
        session_token: token,
        status: "ok",
        session,
        expires_at_epoch_ms,
    };
    (StatusCode::OK, Json(response)).into_response()
}

async fn delete_session(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    if state.auth_mode == AuthMode::Oidc {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(ErrorResponse {
                message: "Session revoke endpoint is disabled in oidc auth mode".to_string(),
            }),
        )
            .into_response();
    }

    let claims = match verify_session_token_with_secrets(&state.session_secrets, &token) {
        Ok(claims) => claims,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    message: "Unknown session token".to_string(),
                }),
            )
                .into_response();
        }
    };
    let mut revoked = state.revoked_jtis.write().await;
    if !revoked.insert(claims.jti) {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                message: "Unknown session token".to_string(),
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(LogoutResponse {
            message: "Session revoked",
        }),
    )
        .into_response()
}

async fn post_statements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PostStatementRequest>,
) -> impl IntoResponse {
    if let Err(response) = require_auth(&state, &headers).await {
        return response;
    }

    let _ = (
        payload.timeout,
        payload.warehouse,
        payload.database,
        payload.schema,
        payload.role,
    );
    if let Err(err) = opensnow_sql::parse_select_parquet_scan(&payload.statement) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                message: err.to_string(),
            }),
        )
            .into_response();
    }

    let id = state.next_statement_id.fetch_add(1, Ordering::Relaxed) + 1;
    let handle = format!("stmt_mock_{id:04}");
    let now_ms = now_epoch_ms();
    state.statements.write().await.insert(
        handle.clone(),
        StatementEntry {
            status: StatementStatus::Running,
            rows: None,
            error: None,
            created_at_epoch_ms: now_ms,
            updated_at_epoch_ms: now_ms,
        },
    );

    let execution_state = state.clone();
    let execution_handle = handle.clone();
    let sql = payload.statement;
    tokio::spawn(async move {
        let result = opensnow_compute::execute_sql(&sql).await;
        let mut statements = execution_state.statements.write().await;
        if let Some(entry) = statements.get_mut(&execution_handle) {
            match result {
                Ok(rows) => {
                    entry.status = StatementStatus::Succeeded;
                    entry.rows = Some(rows);
                    entry.error = None;
                    entry.updated_at_epoch_ms = now_epoch_ms();
                }
                Err(err) => {
                    entry.status = StatementStatus::Failed;
                    entry.rows = None;
                    entry.error = Some(err.to_string());
                    entry.updated_at_epoch_ms = now_epoch_ms();
                }
            }
        }
    });

    let response = PostStatementResponse {
        statement_handle: handle.clone(),
        status: "accepted",
        message: "Statement accepted for execution",
        poll_url: format!("/api/v2/statements/{handle}"),
        created_at_epoch_ms: now_ms,
    };

    (StatusCode::ACCEPTED, Json(response)).into_response()
}

async fn get_statement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(handle): Path<String>,
) -> impl IntoResponse {
    if let Err(response) = require_auth(&state, &headers).await {
        return response;
    }

    let entry = {
        let statements = state.statements.read().await;
        statements.get(&handle).cloned()
    };

    let Some(entry) = entry else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                message: "Unknown statement handle".to_string(),
            }),
        )
            .into_response();
    };

    let (status, message, sql_state, error_code) = match entry.status {
        StatementStatus::Running => ("running", "Statement is still running".to_string(), "00000", None),
        StatementStatus::Succeeded => (
            "succeeded",
            "Statement completed successfully".to_string(),
            "00000",
            None,
        ),
        StatementStatus::Cancelled => ("canceled", "Statement was canceled".to_string(), "57014", Some("604")),
        StatementStatus::Failed => (
            "failed",
            entry
                .error
                .clone()
                .unwrap_or_else(|| "Statement failed during execution".to_string()),
            "XX000",
            Some("1000"),
        ),
    };

    let response = GetStatementResponse {
        statement_handle: handle,
        status,
        sql_state,
        error_code,
        message,
        row_count: entry.rows.as_ref().map(std::vec::Vec::len),
        data: entry.rows,
        created_at_epoch_ms: entry.created_at_epoch_ms,
        updated_at_epoch_ms: entry.updated_at_epoch_ms,
    };

    (StatusCode::OK, Json(response)).into_response()
}

async fn cancel_statement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(handle): Path<String>,
) -> impl IntoResponse {
    if let Err(response) = require_auth(&state, &headers).await {
        return response;
    }

    let mut statements = state.statements.write().await;
    let Some(current) = statements.get_mut(&handle) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                message: "Unknown statement handle".to_string(),
            }),
        )
            .into_response();
    };

    if !matches!(current.status, StatementStatus::Running) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                message: "Statement is already in a terminal state".to_string(),
            }),
        )
            .into_response();
    }
    current.status = StatementStatus::Cancelled;
    current.error = Some("Canceled by client".to_string());
    current.updated_at_epoch_ms = now_epoch_ms();

    (
        StatusCode::OK,
        Json(NotImplementedResponse {
            message: "Cancel request accepted (mock response)",
        }),
    )
        .into_response()
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn require_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), axum::response::Response> {
    let Some(auth_header) = headers.get(header::AUTHORIZATION) else {
        return Err(unauthorized_response());
    };

    let Ok(auth_header) = auth_header.to_str() else {
        return Err(unauthorized_response());
    };

    if state.auth_mode == AuthMode::Oidc {
        let Some(token) = auth_header.strip_prefix("Bearer ") else {
            return Err(unauthorized_response());
        };
        let Some(jwks_url) = state.oidc_jwks_url.as_deref() else {
            return Err(unauthorized_response());
        };
        let now_secs = now_epoch_secs();
        let jwks_json = get_cached_or_fresh_jwks(state, jwks_url, now_secs)
            .await
            .map_err(|_| unauthorized_response())?;
        let claims = verify_oidc_access_token_with_jwks_json(
            token,
            &jwks_json,
            state.oidc_issuer.as_deref(),
            state.oidc_audience.as_deref(),
        )
        .map_err(|_| unauthorized_response())?;
        if claims.exp > now_epoch_secs() {
            return Ok(());
        }
        return Err(unauthorized_response());
    }

    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        let claims = match verify_session_token_with_secrets(&state.session_secrets, token) {
            Ok(claims) => claims,
            Err(_) => return Err(unauthorized_response()),
        };
        let revoked = state.revoked_jtis.read().await;
        if !revoked.contains(&claims.jti) {
            return Ok(());
        }
        return Err(unauthorized_response());
    }

    let Some(encoded) = auth_header.strip_prefix("Basic ") else {
        return Err(unauthorized_response());
    };

    let Ok(raw) = STANDARD.decode(encoded) else {
        return Err(unauthorized_response());
    };
    let Ok(raw) = String::from_utf8(raw) else {
        return Err(unauthorized_response());
    };

    let Some((user, password)) = raw.split_once(':') else {
        return Err(unauthorized_response());
    };
    let Some(stored_hash) = state.users.get(user) else {
        return Err(unauthorized_response());
    };

    let Ok(valid) = verify_password(password, stored_hash) else {
        return Err(unauthorized_response());
    };
    if !valid {
        return Err(unauthorized_response());
    }

    Ok(())
}

async fn get_cached_or_fresh_jwks(
    state: &AppState,
    jwks_url: &str,
    now_secs: u64,
) -> anyhow::Result<String> {
    {
        let cache = state.oidc_jwks_cache.read().await;
        if let Some(entry) = cache.as_ref() {
            let age = now_secs.saturating_sub(entry.fetched_at_epoch_secs);
            if age < state.oidc_jwks_cache_ttl_seconds {
                return Ok(entry.jwks_json.clone());
            }
        }
    }

    match fetch_jwks_json(jwks_url).await {
        Ok(jwks_json) => {
            let mut cache = state.oidc_jwks_cache.write().await;
            *cache = Some(JwksCacheEntry {
                jwks_json: jwks_json.clone(),
                fetched_at_epoch_secs: now_secs,
            });
            Ok(jwks_json)
        }
        Err(err) => {
            let cache = state.oidc_jwks_cache.read().await;
            if let Some(entry) = cache.as_ref() {
                tracing::warn!("oidc jwks refresh failed; using stale cache: {}", err);
                return Ok(entry.jwks_json.clone());
            }
            Err(anyhow::anyhow!("failed to fetch oidc jwks: {err}"))
        }
    }
}

fn unauthorized_response() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, r#"Basic realm="opensnow""#)],
        Json(ErrorResponse {
            message: "Authentication required".to_string(),
        }),
    )
        .into_response()
}
