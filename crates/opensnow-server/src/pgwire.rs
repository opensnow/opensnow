use anyhow::Context;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

const PROTOCOL_VERSION_3: i32 = 196_608;
const SSL_REQUEST_CODE: i32 = 80_877_103;
const AUTH_OK: i32 = 0;
const AUTH_CLEAR_TEXT_PASSWORD: i32 = 3;

/// Prepared statement name → SQL from client [`Parse`].
/// Extended query state per PostgreSQL session.
#[derive(Default)]
struct Session {
    statements: HashMap<String, PreparedStatement>,
    portals: HashMap<String, Portal>,
    /// Result of last [`Describe`] for a **statement** (`S`); drives whether [`Execute`] may emit
    /// [`RowDescription`] (tokio-postgres does not accept a second `T` after `Describe`).
    statement_describe: HashMap<String, StmtDescribeState>,
    /// Rows left to stream after a [`PortalSuspended`] response for this portal name.
    portal_cursors: HashMap<String, Vec<serde_json::Value>>,
}

#[derive(Clone)]
struct PreparedStatement {
    query: String,
    /// Type OIDs from the client [`Parse`] message (`0` = unspecified → treat as text).
    param_type_oids: Vec<i32>,
}

/// Bound portal: concrete SQL plus the prepared statement it was created from.
struct Portal {
    sql: String,
    statement_name: String,
}

#[derive(Clone)]
enum StmtDescribeState {
    /// [`Describe`] returned `NoData`.
    NoData,
    /// [`Describe`] returned [`RowDescription`]; column names and PostgreSQL type OIDs.
    RowDesc(Vec<(String, i32)>),
}

pub async fn run_pgwire_listener<A: ToSocketAddrs>(
    addr: A,
    expected_user: &str,
    expected_password: &str,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("PostgreSQL wire listener started");
    loop {
        let (socket, peer) = listener.accept().await?;
        let expected_user = expected_user.to_string();
        let expected_password = expected_password.to_string();
        tokio::spawn(async move {
            if let Err(err) = handle_client(socket, &expected_user, &expected_password).await {
                tracing::debug!(%peer, error = %err, "pgwire session ended with error");
            }
        });
    }
}

async fn handle_client(
    mut socket: TcpStream,
    expected_user: &str,
    expected_password: &str,
) -> anyhow::Result<()> {
    let startup = read_startup_packet(&mut socket).await?;
    if startup.protocol_version == SSL_REQUEST_CODE {
        socket.write_all(b"N").await?;
        let next = read_startup_packet(&mut socket).await?;
        if next.protocol_version != PROTOCOL_VERSION_3 {
            send_error(&mut socket, "08P01", "unsupported protocol version").await?;
            return Ok(());
        }
        authenticate_and_run(&mut socket, next.params, expected_user, expected_password).await?;
        return Ok(());
    }

    if startup.protocol_version != PROTOCOL_VERSION_3 {
        send_error(&mut socket, "08P01", "unsupported protocol version").await?;
        return Ok(());
    }
    authenticate_and_run(&mut socket, startup.params, expected_user, expected_password).await?;
    Ok(())
}

async fn authenticate_and_run(
    socket: &mut TcpStream,
    params: HashMap<String, String>,
    expected_user: &str,
    expected_password: &str,
) -> anyhow::Result<()> {
    write_auth_request_cleartext(socket).await?;
    let password = read_password_message(socket).await?;
    let user = params.get("user").cloned().unwrap_or_default();

    if user != expected_user {
        send_error(socket, "28P01", "invalid user or password").await?;
        return Ok(());
    }
    if password != expected_password {
        send_error(socket, "28P01", "invalid user or password").await?;
        return Ok(());
    }

    write_auth_ok(socket).await?;
    write_parameter_status(socket, "server_version", "16.0").await?;
    write_parameter_status(socket, "client_encoding", "UTF8").await?;
    write_parameter_status(socket, "DateStyle", "ISO, MDY").await?;
    write_backend_key_data(socket, 1, 1).await?;
    write_ready(socket).await?;

    let mut session = Session::default();
    let mut discard_until_sync = false;

    loop {
        let message = match read_frontend_message(socket).await {
            Ok(msg) => msg,
            Err(_) => return Ok(()),
        };

        if discard_until_sync {
            if message.tag == b'S' {
                discard_until_sync = false;
                write_ready(socket).await?;
            }
            continue;
        }

        match message.tag {
            b'Q' => {
                if let Err(errmsg) = handle_simple_query(socket, &message.payload).await {
                    send_error(socket, "XX000", &errmsg).await?;
                    write_ready(socket).await?;
                }
            }
            b'P' => {
                if let Err(e) = handle_parse(socket, &mut session, &message.payload).await {
                    send_error(socket, "XX000", &e.to_string()).await?;
                    discard_until_sync = true;
                }
            }
            b'B' => {
                if let Err(e) = handle_bind(socket, &mut session, &message.payload).await {
                    send_error(socket, "XX000", &e.to_string()).await?;
                    discard_until_sync = true;
                }
            }
            b'D' => {
                if let Err(e) = handle_describe(socket, &mut session, &message.payload).await {
                    send_error(socket, "XX000", &e.to_string()).await?;
                    discard_until_sync = true;
                }
            }
            b'E' => {
                if let Err(errmsg) = handle_execute(socket, &mut session, &message.payload).await {
                    send_error(socket, "XX000", &errmsg).await?;
                    discard_until_sync = true;
                }
            }
            b'C' => {
                if let Err(e) = handle_close(&mut session, &message.payload) {
                    send_error(socket, "XX000", &e.to_string()).await?;
                    discard_until_sync = true;
                } else {
                    write_close_complete(socket).await?;
                }
            }
            b'H' => {
                socket.flush().await?;
            }
            b'S' => {
                write_ready(socket).await?;
            }
            b'X' => return Ok(()),
            _ => {
                send_error(
                    socket,
                    "0A000",
                    &format!("unsupported PostgreSQL frontend message (tag {})", message.tag as char),
                )
                .await?;
                discard_until_sync = true;
            }
        }
    }
}

async fn handle_simple_query(socket: &mut TcpStream, payload: &[u8]) -> anyhow::Result<(), String> {
    let sql = parse_cstring(payload).map_err(|e| e.to_string())?.trim().to_string();
    if sql.is_empty() {
        write_command_complete(socket, "EMPTY QUERY")
            .await
            .map_err(|e| e.to_string())?;
        write_ready(socket).await.map_err(|e| e.to_string())?;
        return Ok(());
    }
    if is_session_sql(&sql) {
        let tag = command_tag_for_session_sql(&sql);
        write_command_complete(socket, tag)
            .await
            .map_err(|e| e.to_string())?;
        write_ready(socket).await.map_err(|e| e.to_string())?;
        return Ok(());
    }
    match opensnow_compute::execute_sql(&sql).await {
        Ok(rows) => {
            write_query_result(socket, rows)
                .await
                .map_err(|e| e.to_string())?;
            write_ready(socket).await.map_err(|e| e.to_string())?;
        }
        Err(err) => {
            send_error(socket, "XX000", &err.to_string())
                .await
                .map_err(|e| e.to_string())?;
            write_ready(socket).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn handle_parse(
    socket: &mut TcpStream,
    session: &mut Session,
    payload: &[u8],
) -> anyhow::Result<()> {
    let mut buf = MsgBuf::new(payload);
    let stmt_name = buf.read_cstring()?;
    let query = buf.read_cstring()?;
    let num_param_types = buf.read_i16()? as i32;
    let mut param_type_oids = Vec::with_capacity(num_param_types.max(0) as usize);
    for _ in 0..num_param_types {
        param_type_oids.push(buf.read_i32()?);
    }
    session.statement_describe.remove(&stmt_name);
    session.statements.insert(
        stmt_name,
        PreparedStatement {
            query,
            param_type_oids,
        },
    );
    write_parse_complete(socket).await?;
    Ok(())
}

async fn handle_bind(
    socket: &mut TcpStream,
    session: &mut Session,
    payload: &[u8],
) -> anyhow::Result<()> {
    let mut buf = MsgBuf::new(payload);
    let portal_name = buf.read_cstring()?;
    let stmt_name = buf.read_cstring()?;

    let num_format_codes = buf.read_i16()? as usize;
    let mut format_codes = Vec::new();
    match num_format_codes {
        0 => {}
        1 => format_codes.push(buf.read_i16()?),
        n => {
            for _ in 0..n {
                format_codes.push(buf.read_i16()?);
            }
        }
    }

    let num_params = buf.read_i16()? as usize;
    let mut params: Vec<Option<String>> = Vec::with_capacity(num_params);
    for p in 0..num_params {
        let len = buf.read_i32()?;
        let fmt = param_format_code(&format_codes, p)?;
        if len < 0 {
            params.push(None);
            continue;
        }
        let len = len as usize;
        let bytes = buf.read_bytes(len)?;
        let s = decode_param(p, fmt, &bytes)?;
        params.push(Some(s));
    }

    // Result-column format codes (same 0 / 1 / N convention as parameter format codes).
    let num_result_formats = buf.read_i16()? as usize;
    match num_result_formats {
        0 => {}
        1 => {
            let _ = buf.read_i16()?;
        }
        n => {
            for _ in 0..n {
                let _ = buf.read_i16()?;
            }
        }
    }

    let prep = session
        .statements
        .get(&stmt_name)
        .with_context(|| format!("unknown prepared statement `{}`", stmt_name))?
        .clone();

    let sql = substitute_params(&prep.query, &params)?;
    session.portal_cursors.remove(&portal_name);
    session.portals.insert(
        portal_name,
        Portal {
            sql,
            statement_name: stmt_name,
        },
    );
    write_bind_complete(socket).await?;
    Ok(())
}

fn param_format_code(codes: &[i16], param_idx: usize) -> anyhow::Result<i16> {
    if codes.is_empty() {
        Ok(0)
    } else if codes.len() == 1 {
        Ok(codes[0])
    } else {
        codes
            .get(param_idx)
            .copied()
            .context("missing parameter format code")
    }
}

/// `0` = text, `1` = binary (`int4` or UTF-8 text as sent by some clients).
fn decode_param(_param_idx: usize, fmt: i16, bytes: &[u8]) -> anyhow::Result<String> {
    match fmt {
        0 => Ok(std::str::from_utf8(bytes)?.to_string()),
        1 => {
            if bytes.len() == 4 {
                let v = i32::from_be_bytes(bytes.try_into().unwrap());
                Ok(v.to_string())
            } else {
                Ok(std::str::from_utf8(bytes)?.to_string())
            }
        }
        other => anyhow::bail!("unsupported parameter format code {other}"),
    }
}

async fn handle_describe(
    socket: &mut TcpStream,
    session: &mut Session,
    payload: &[u8],
) -> anyhow::Result<()> {
    if payload.is_empty() {
        anyhow::bail!("empty Describe payload");
    }
    let kind = payload[0];
    let mut buf = MsgBuf::new(&payload[1..]);
    let name = buf.read_cstring()?;

    match kind {
        b'S' => {
            let prep = session
                .statements
                .get(&name)
                .with_context(|| format!("unknown prepared statement `{}`", name))?;
            let sql = prep.query.as_str();
            let n_params = max_param_index(sql);
            let param_oids = resolved_statement_param_oids(prep, n_params);
            write_parameter_description_oids(socket, &param_oids).await?;

            if is_session_sql(sql) || !sql.trim().to_ascii_uppercase().starts_with("SELECT") {
                write_no_data(socket).await?;
                session
                    .statement_describe
                    .insert(name.clone(), StmtDescribeState::NoData);
                return Ok(());
            }

            let col_shape: Vec<(String, i32)> = if n_params == 0 {
                infer_result_shape(sql).await?
            } else {
                // Placeholder SQL is described without executing; cells are always UTF-8 text on the wire.
                placeholder_select_aliases(sql)
                    .into_iter()
                    .map(|n| (n, postgres_types::TEXT_OID))
                    .collect()
            };

            if col_shape.is_empty() {
                write_no_data(socket).await?;
                session
                    .statement_describe
                    .insert(name.clone(), StmtDescribeState::NoData);
            } else {
                write_row_description(socket, &col_shape).await?;
                session
                    .statement_describe
                    .insert(name.clone(), StmtDescribeState::RowDesc(col_shape));
            }
        }
        b'P' => {
            let portal = session
                .portals
                .get(&name)
                .with_context(|| format!("unknown portal `{}`", name))?;
            describe_portal_adhoc(socket, &portal.sql).await?;
        }
        _ => anyhow::bail!("invalid Describe kind {}", kind as char),
    }
    Ok(())
}

/// Cheap column names for `SELECT $1 AS n`-style statements (no execute at describe time).
fn placeholder_select_aliases(sql: &str) -> Vec<String> {
    if max_param_index(sql) == 0 {
        return Vec::new();
    }
    let t = sql.trim();
    let low = t.to_ascii_lowercase();
    if !low.starts_with("select") {
        return Vec::new();
    }
    if let Some(pos) = low.rfind(" as ") {
        let after = t[pos + 4..].trim();
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return vec![name];
        }
    }
    Vec::new()
}

async fn describe_portal_adhoc(socket: &mut TcpStream, sql: &str) -> anyhow::Result<()> {
    if is_session_sql(sql) {
        write_no_data(socket).await?;
        return Ok(());
    }
    let shape = infer_result_shape(sql).await?;
    write_row_description(socket, &shape).await?;
    Ok(())
}

/// PostgreSQL OIDs we advertise for parameters/results (subset).
mod postgres_types {
    pub const UNSPECIFIED_OID: i32 = 0;
    pub const INT4_OID: i32 = 23;
    pub const TEXT_OID: i32 = 25;
    #[allow(dead_code)]
    pub const FLOAT8_OID: i32 = 701;
}

fn resolved_statement_param_oids(prep: &PreparedStatement, n_params: i16) -> Vec<i32> {
    let n = n_params as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let oid = if prep.param_type_oids.is_empty() {
            postgres_types::TEXT_OID
        } else {
            let o = prep
                .param_type_oids
                .get(i)
                .copied()
                .unwrap_or(postgres_types::UNSPECIFIED_OID);
            if o == postgres_types::UNSPECIFIED_OID {
                postgres_types::TEXT_OID
            } else {
                o
            }
        };
        out.push(oid);
    }
    out
}

async fn write_parameter_description_oids(
    socket: &mut TcpStream,
    oids: &[i32],
) -> anyhow::Result<()> {
    let n = oids.len() as i16;
    let mut body = Vec::new();
    body.extend_from_slice(&n.to_be_bytes());
    for oid in oids {
        body.extend_from_slice(&oid.to_be_bytes());
    }
    write_message(socket, b't', &body).await
}

/// Column names (sorted) and best-effort type OIDs from a sample row.
async fn infer_result_shape(sql: &str) -> anyhow::Result<Vec<(String, i32)>> {
    let rows = opensnow_compute::execute_sql(sql).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let first = rows[0]
        .as_object()
        .context("Describe: row must be object")?;
    let mut names: Vec<String> = first.keys().cloned().collect();
    names.sort();
    Ok(names
        .into_iter()
        .map(|name| {
            let oid = first
                .get(&name)
                .map(json_value_type_oid)
                .unwrap_or(postgres_types::TEXT_OID);
            (name, oid)
        })
        .collect())
}

fn json_value_type_oid(v: &serde_json::Value) -> i32 {
    match v {
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                postgres_types::INT4_OID
            } else {
                postgres_types::FLOAT8_OID
            }
        }
        serde_json::Value::String(s) => cell_string_type_oid(s),
        _ => postgres_types::TEXT_OID,
    }
}

fn cell_string_type_oid(_s: &str) -> i32 {
    // Compute layer exposes most scalars as JSON strings; keep `TEXT` unless we add a typed IR.
    postgres_types::TEXT_OID
}

async fn handle_execute(
    socket: &mut TcpStream,
    session: &mut Session,
    payload: &[u8],
) -> anyhow::Result<(), String> {
    let mut buf = MsgBuf::new(payload);
    let portal_name = buf.read_cstring().map_err(|e| e.to_string())?;
    let max_rows_req = buf.read_i32().map_err(|e| e.to_string())?;
    if max_rows_req < 0 {
        return Err("Execute max_rows must be non-negative".to_string());
    }

    let portal = session
        .portals
        .get(&portal_name)
        .with_context(|| format!("unknown portal `{}`", portal_name))
        .map_err(|e| e.to_string())?;

    let sql_text = portal.sql.clone();
    let stmt_name = portal.statement_name.clone();
    let describe_state = session.statement_describe.get(&stmt_name).cloned();

    if sql_text.trim().is_empty() {
        session.portal_cursors.remove(&portal_name);
        write_command_complete(socket, "EMPTY QUERY")
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if is_session_sql(&sql_text) {
        session.portal_cursors.remove(&portal_name);
        let tag = command_tag_for_session_sql(&sql_text);
        write_command_complete(socket, tag)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut rows: Vec<serde_json::Value> = if let Some(pending) = session.portal_cursors.remove(&portal_name)
    {
        pending
    } else {
        opensnow_compute::execute_sql(&sql_text)
            .await
            .map_err(|e| e.to_string())?
    };

    let max_rows = if describe_state.is_some() {
        max_rows_req as usize
    } else {
        0
    };

    let mut rest = Vec::new();
    if max_rows > 0 && rows.len() > max_rows {
        rest = rows.split_off(max_rows);
    }
    let suspended = !rest.is_empty();
    if suspended {
        session.portal_cursors.insert(portal_name.clone(), rest);
    }

    match &describe_state {
        Some(StmtDescribeState::RowDesc(shape)) => {
            let names: Vec<String> = shape.iter().map(|(n, _)| n.clone()).collect();
            write_query_result_skip_row_description(socket, rows, &names, suspended)
                .await
                .map_err(|e| e.to_string())
        }
        Some(StmtDescribeState::NoData) => write_query_result_skip_row_description(
            socket,
            rows,
            &[],
            suspended,
        )
        .await
        .map_err(|e| e.to_string()),
        None => {
            session.portal_cursors.remove(&portal_name);
            write_query_result(socket, rows)
                .await
                .map_err(|e| e.to_string())
        }
    }
}

fn handle_close(session: &mut Session, payload: &[u8]) -> anyhow::Result<()> {
    if payload.is_empty() {
        anyhow::bail!("empty Close payload");
    }
    let kind = payload[0];
    let mut buf = MsgBuf::new(&payload[1..]);
    let name = buf.read_cstring()?;
    match kind {
        b'S' => {
            session.statements.remove(&name);
            session.statement_describe.remove(&name);
        }
        b'P' => {
            session.portals.remove(&name);
            session.portal_cursors.remove(&name);
        }
        _ => anyhow::bail!("invalid Close kind {}", kind as char),
    }
    Ok(())
}

fn is_session_sql(sql: &str) -> bool {
    let s = sql.trim().to_ascii_lowercase();
    s.starts_with("set ")
        || s == "begin"
        || s == "commit"
        || s == "rollback"
        || s.starts_with("show ")
}

fn command_tag_for_session_sql(sql: &str) -> &'static str {
    let s = sql.trim().to_ascii_lowercase();
    if s.starts_with("set ") {
        "SET"
    } else if s == "begin" {
        "BEGIN"
    } else if s == "commit" {
        "COMMIT"
    } else if s == "rollback" {
        "ROLLBACK"
    } else {
        "SHOW"
    }
}

fn max_param_index(sql: &str) -> i16 {
    find_placeholders(sql)
        .iter()
        .map(|ph| ph.n as i16)
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
struct Placeholder {
    start: usize,
    end: usize,
    /// 1-based index (`$1` → 1).
    n: usize,
}

#[derive(Debug, Clone, Copy)]
enum ScanState {
    Normal,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

/// Find `$n` placeholders outside string/comment literals (PostgreSQL-style `$` + digits).
fn find_placeholders(sql: &str) -> Vec<Placeholder> {
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    let mut state = ScanState::Normal;
    while i < bytes.len() {
        match state {
            ScanState::Normal => {
                if bytes[i] == b'\'' {
                    state = ScanState::SingleQuote;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'"' {
                    state = ScanState::DoubleQuote;
                    i += 1;
                    continue;
                }
                if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
                    state = ScanState::LineComment;
                    i += 2;
                    continue;
                }
                if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    state = ScanState::BlockComment;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    let start = i;
                    let mut j = i + 1;
                    let mut n: usize = 0;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        n = n * 10 + (bytes[j] - b'0') as usize;
                        j += 1;
                    }
                    if n > 0 {
                        out.push(Placeholder {
                            start,
                            end: j,
                            n,
                        });
                    }
                    i = j;
                    continue;
                }
                i += 1;
            }
            ScanState::SingleQuote => {
                if bytes[i] == b'\'' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    state = ScanState::Normal;
                }
                i += 1;
            }
            ScanState::DoubleQuote => {
                if bytes[i] == b'"' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                        i += 2;
                        continue;
                    }
                    state = ScanState::Normal;
                }
                i += 1;
            }
            ScanState::LineComment => {
                if bytes[i] == b'\n' {
                    state = ScanState::Normal;
                }
                i += 1;
            }
            ScanState::BlockComment => {
                if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    state = ScanState::Normal;
                    i += 2;
                    continue;
                }
                i += 1;
            }
        }
    }
    out
}

fn substitute_params(sql: &str, params: &[Option<String>]) -> anyhow::Result<String> {
    let placeholders = find_placeholders(sql);
    let max_n = placeholders.iter().map(|p| p.n).max().unwrap_or(0);
    if max_n != params.len() {
        anyhow::bail!(
            "bind parameter count {} does not match statement placeholders (expect {} for max index)",
            params.len(),
            max_n
        );
    }
    let mut out = sql.to_string();
    let mut sorted = placeholders;
    sorted.sort_by_key(|p| p.start);
    for ph in sorted.into_iter().rev() {
        let replacement = match params.get(ph.n - 1) {
            Some(Some(s)) => format!("'{}'", s.replace('\'', "''")),
            _ => "NULL".to_string(),
        };
        out.replace_range(ph.start..ph.end, &replacement);
    }
    Ok(out)
}

struct StartupPacket {
    protocol_version: i32,
    params: HashMap<String, String>,
}

struct FrontendMessage {
    tag: u8,
    payload: Vec<u8>,
}

struct MsgBuf<'a> {
    buf: &'a [u8],
    i: usize,
}

impl<'a> MsgBuf<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, i: 0 }
    }

    fn read_i16(&mut self) -> anyhow::Result<i16> {
        let end = self.i + 2;
        let slice = self.buf.get(self.i..end).context("parse underflow")?;
        self.i = end;
        Ok(i16::from_be_bytes(slice.try_into().unwrap()))
    }

    fn read_i32(&mut self) -> anyhow::Result<i32> {
        let end = self.i + 4;
        let slice = self.buf.get(self.i..end).context("parse underflow")?;
        self.i = end;
        Ok(i32::from_be_bytes(slice.try_into().unwrap()))
    }

    fn read_cstring(&mut self) -> anyhow::Result<String> {
        let start = self.i;
        let rest = self.buf.get(start..).context("parse underflow")?;
        let nul = rest
            .iter()
            .position(|&b| b == 0)
            .context("cstring terminator missing")?;
        let s = std::str::from_utf8(&rest[..nul])?.to_string();
        self.i = start + nul + 1;
        Ok(s)
    }

    fn read_bytes(&mut self, len: usize) -> anyhow::Result<&'a [u8]> {
        let end = self.i + len;
        let slice = self.buf.get(self.i..end).context("parse underflow")?;
        self.i = end;
        Ok(slice)
    }
}

async fn read_startup_packet(socket: &mut TcpStream) -> anyhow::Result<StartupPacket> {
    let len = socket.read_i32().await?;
    if len < 8 {
        anyhow::bail!("invalid startup packet length");
    }
    let mut buf = vec![0_u8; (len - 4) as usize];
    socket.read_exact(&mut buf).await?;
    let protocol_version = i32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let params = if protocol_version == PROTOCOL_VERSION_3 {
        parse_params(&buf[4..])?
    } else {
        HashMap::new()
    };
    Ok(StartupPacket {
        protocol_version,
        params,
    })
}

fn parse_params(raw: &[u8]) -> anyhow::Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == 0 {
            break;
        }
        let key_end = raw[i..]
            .iter()
            .position(|b| *b == 0)
            .map(|p| i + p)
            .context("startup key missing terminator")?;
        let key = std::str::from_utf8(&raw[i..key_end])?.to_string();
        i = key_end + 1;
        let val_end = raw[i..]
            .iter()
            .position(|b| *b == 0)
            .map(|p| i + p)
            .context("startup value missing terminator")?;
        let val = std::str::from_utf8(&raw[i..val_end])?.to_string();
        i = val_end + 1;
        out.insert(key, val);
    }
    Ok(out)
}

async fn read_frontend_message(socket: &mut TcpStream) -> anyhow::Result<FrontendMessage> {
    let tag = socket.read_u8().await?;
    let len = socket.read_i32().await?;
    if len < 4 {
        anyhow::bail!("invalid frontend message length");
    }
    let mut payload = vec![0_u8; (len - 4) as usize];
    socket.read_exact(&mut payload).await?;
    Ok(FrontendMessage { tag, payload })
}

async fn read_password_message(socket: &mut TcpStream) -> anyhow::Result<String> {
    let msg = read_frontend_message(socket).await?;
    if msg.tag != b'p' {
        anyhow::bail!("expected password message");
    }
    parse_cstring(&msg.payload)
}

fn parse_cstring(payload: &[u8]) -> anyhow::Result<String> {
    let nul = payload
        .iter()
        .position(|b| *b == 0)
        .context("cstring payload missing terminator")?;
    Ok(std::str::from_utf8(&payload[..nul])?.to_string())
}

async fn write_parse_complete(socket: &mut TcpStream) -> anyhow::Result<()> {
    write_message(socket, b'1', &[]).await
}

async fn write_bind_complete(socket: &mut TcpStream) -> anyhow::Result<()> {
    write_message(socket, b'2', &[]).await
}

async fn write_close_complete(socket: &mut TcpStream) -> anyhow::Result<()> {
    write_message(socket, b'3', &[]).await
}

async fn write_no_data(socket: &mut TcpStream) -> anyhow::Result<()> {
    write_message(socket, b'n', &[]).await
}

async fn write_auth_request_cleartext(socket: &mut TcpStream) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(&AUTH_CLEAR_TEXT_PASSWORD.to_be_bytes());
    write_message(socket, b'R', &body).await
}

async fn write_auth_ok(socket: &mut TcpStream) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(&AUTH_OK.to_be_bytes());
    write_message(socket, b'R', &body).await
}

async fn write_backend_key_data(
    socket: &mut TcpStream,
    pid: i32,
    secret: i32,
) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(&pid.to_be_bytes());
    body.extend_from_slice(&secret.to_be_bytes());
    write_message(socket, b'K', &body).await
}

async fn write_parameter_status(
    socket: &mut TcpStream,
    key: &str,
    value: &str,
) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(key.as_bytes());
    body.push(0);
    body.extend_from_slice(value.as_bytes());
    body.push(0);
    write_message(socket, b'S', &body).await
}

async fn write_ready(socket: &mut TcpStream) -> anyhow::Result<()> {
    write_message(socket, b'Z', b"I").await
}

async fn write_row_description(
    socket: &mut TcpStream,
    cols: &[(String, i32)],
) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(&(cols.len() as i16).to_be_bytes());
    for (name, type_oid) in cols {
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(&0_i32.to_be_bytes());
        body.extend_from_slice(&0_i16.to_be_bytes());
        body.extend_from_slice(&type_oid.to_be_bytes());
        body.extend_from_slice(&(-1_i16).to_be_bytes());
        body.extend_from_slice(&(-1_i32).to_be_bytes());
        body.extend_from_slice(&0_i16.to_be_bytes());
    }
    write_message(socket, b'T', &body).await
}

async fn write_data_row(socket: &mut TcpStream, values: &[String]) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(&(values.len() as i16).to_be_bytes());
    for value in values {
        body.extend_from_slice(&(value.len() as i32).to_be_bytes());
        body.extend_from_slice(value.as_bytes());
    }
    write_message(socket, b'D', &body).await
}

async fn write_command_complete(socket: &mut TcpStream, tag: &str) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.extend_from_slice(tag.as_bytes());
    body.push(0);
    write_message(socket, b'C', &body).await
}

async fn write_portal_suspended(socket: &mut TcpStream) -> anyhow::Result<()> {
    write_message(socket, b's', &[]).await
}

async fn send_error(socket: &mut TcpStream, sql_state: &str, message: &str) -> anyhow::Result<()> {
    let mut body = Vec::new();
    body.push(b'S');
    body.extend_from_slice(b"ERROR");
    body.push(0);
    body.push(b'C');
    body.extend_from_slice(sql_state.as_bytes());
    body.push(0);
    body.push(b'M');
    body.extend_from_slice(message.as_bytes());
    body.push(0);
    body.push(0);
    write_message(socket, b'E', &body).await
}

/// After the client received [`RowDescription`] from [`Describe`] on this portal, send only data +
/// completion.
async fn write_query_result_skip_row_description(
    socket: &mut TcpStream,
    rows: Vec<serde_json::Value>,
    cols: &[String],
    suspended: bool,
) -> anyhow::Result<()> {
    if rows.is_empty() && !suspended {
        write_command_complete(socket, "SELECT 0").await?;
        return Ok(());
    }
    let cols: Vec<String> = if cols.is_empty() {
        if rows.is_empty() {
            Vec::new()
        } else {
            let first = rows[0]
                .as_object()
                .context("query row must be a JSON object")?;
            let mut inferred: Vec<String> = first.keys().cloned().collect();
            inferred.sort();
            inferred
        }
    } else {
        cols.to_vec()
    };
    let row_count = rows.len();
    for row in rows {
        let obj = row
            .as_object()
            .context("query row must be a JSON object")?;
        let values: Vec<String> = cols
            .iter()
            .map(|c| {
                obj.get(c)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        write_data_row(socket, &values).await?;
    }
    if suspended {
        write_portal_suspended(socket).await?;
    } else {
        write_command_complete(socket, &format!("SELECT {}", row_count)).await?;
    }
    Ok(())
}

async fn write_query_result(socket: &mut TcpStream, rows: Vec<serde_json::Value>) -> anyhow::Result<()> {
    if rows.is_empty() {
        write_row_description(socket, &[]).await?;
        write_command_complete(socket, "SELECT 0").await?;
        return Ok(());
    }
    let row_count = rows.len();

    let first = rows[0]
        .as_object()
        .context("query row must be a JSON object")?;
    let mut names: Vec<String> = first.keys().cloned().collect();
    names.sort();
    let col_shape: Vec<(String, i32)> = names
        .iter()
        .map(|n| {
            let oid = first
                .get(n)
                .map(json_value_type_oid)
                .unwrap_or(postgres_types::TEXT_OID);
            (n.clone(), oid)
        })
        .collect();
    write_row_description(socket, &col_shape).await?;

    for row in rows {
        let obj = row
            .as_object()
            .context("query row must be a JSON object")?;
        let values: Vec<String> = names
            .iter()
            .map(|c| {
                obj.get(c)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        write_data_row(socket, &values).await?;
    }
    write_command_complete(socket, &format!("SELECT {}", row_count)).await?;
    Ok(())
}

async fn write_message(socket: &mut TcpStream, tag: u8, body: &[u8]) -> anyhow::Result<()> {
    socket.write_u8(tag).await?;
    socket.write_i32((body.len() as i32) + 4).await?;
    socket.write_all(body).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_placeholders_skips_inside_single_quotes() {
        let sql = "SELECT '$1', $2";
        let p = find_placeholders(sql);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].n, 2);
    }

    #[test]
    fn find_placeholders_skips_line_comment() {
        let sql = "SELECT $1 -- $2\n, $3";
        let p = find_placeholders(sql);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].n, 1);
        assert_eq!(p[1].n, 3);
    }

    #[test]
    fn substitute_leaves_dollar_inside_literal() {
        let sql = "SELECT '$1'::text, $1";
        let out = substitute_params(sql, &[Some("x".into())]).unwrap();
        assert!(out.contains("'$1'::text"));
        assert!(out.ends_with(", 'x'") || out.contains(", 'x'"));
    }

    #[test]
    fn dollar_ten_before_dollar_one_substitution_order() {
        let sql = "SELECT $10, $1";
        let out = substitute_params(
            sql,
            &[
                Some("a".into()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("b".into()),
            ],
        )
        .unwrap();
        assert!(out.contains("'b'"));
        assert!(out.contains("'a'"));
        assert!(!out.contains("$10"));
    }

    #[test]
    fn portal_row_batch_split_for_max_rows() {
        let mut rows = vec![
            serde_json::json!({"k":"1"}),
            serde_json::json!({"k":"2"}),
            serde_json::json!({"k":"3"}),
        ];
        let max_rows = 2usize;
        let rest = if max_rows > 0 && rows.len() > max_rows {
            rows.split_off(max_rows)
        } else {
            vec![]
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rest.len(), 1);
    }
}
