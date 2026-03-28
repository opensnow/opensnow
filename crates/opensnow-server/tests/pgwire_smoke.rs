use opensnow_common::ServerConfig;
use postgres_types::Type;
use std::net::TcpListener;
use tokio_postgres::NoTls;

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

#[tokio::test]
async fn pgwire_allows_connect_and_select_one() {
    let pg_port = free_port();
    let http_port = free_port();
    let mut config = ServerConfig::default();
    config.pg_port = pg_port;
    config.http_port = http_port;
    config.bootstrap_user = "admin".to_string();
    config.bootstrap_password = "password".to_string();

    let server = tokio::spawn(opensnow_server::run(config));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let conn = format!(
        "host=127.0.0.1 port={pg_port} user=admin password=password dbname=postgres"
    );
    let (client, connection) = tokio_postgres::connect(&conn, NoTls)
        .await
        .expect("connect via pgwire");
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });

    let messages = client
        .simple_query("SELECT 1 AS n")
        .await
        .expect("simple query");
    let mut got_one = false;
    for message in messages {
        if let tokio_postgres::SimpleQueryMessage::Row(row) = message {
            if row.get("n") == Some("1") {
                got_one = true;
            }
        }
    }
    assert!(got_one, "expected row n=1 from pgwire simple query");

    connection_task.abort();
    server.abort();
}

#[tokio::test]
async fn pgwire_extended_protocol_query_one() {
    let pg_port = free_port();
    let http_port = free_port();
    let mut config = ServerConfig::default();
    config.pg_port = pg_port;
    config.http_port = http_port;
    config.bootstrap_user = "admin".to_string();
    config.bootstrap_password = "password".to_string();

    let server = tokio::spawn(opensnow_server::run(config));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let conn = format!(
        "host=127.0.0.1 port={pg_port} user=admin password=password dbname=postgres"
    );
    let (client, connection) = tokio_postgres::connect(&conn, NoTls)
        .await
        .expect("connect via pgwire");
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });

    let row = client
        .query_one("SELECT 1 AS n", &[])
        .await
        .expect("extended query query_one");
    let value: &str = row.get("n");
    assert_eq!(value, "1");

    connection_task.abort();
    server.abort();
}

#[tokio::test]
async fn pgwire_extended_protocol_parameter_text() {
    let pg_port = free_port();
    let http_port = free_port();
    let mut config = ServerConfig::default();
    config.pg_port = pg_port;
    config.http_port = http_port;
    config.bootstrap_user = "admin".to_string();
    config.bootstrap_password = "password".to_string();

    let server = tokio::spawn(opensnow_server::run(config));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let conn = format!(
        "host=127.0.0.1 port={pg_port} user=admin password=password dbname=postgres"
    );
    let (client, connection) = tokio_postgres::connect(&conn, NoTls)
        .await
        .expect("connect via pgwire");
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });

    let row = client
        .query_one("SELECT $1 AS n", &[&"42"])
        .await
        .expect("parameterized extended query");
    let value: &str = row.get("n");
    assert_eq!(value, "42");

    connection_task.abort();
    server.abort();
}

#[tokio::test]
async fn pgwire_extended_protocol_parameter_int4_binary() {
    let pg_port = free_port();
    let http_port = free_port();
    let mut config = ServerConfig::default();
    config.pg_port = pg_port;
    config.http_port = http_port;
    config.bootstrap_user = "admin".to_string();
    config.bootstrap_password = "password".to_string();

    let server = tokio::spawn(opensnow_server::run(config));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let conn = format!(
        "host=127.0.0.1 port={pg_port} user=admin password=password dbname=postgres"
    );
    let (client, connection) = tokio_postgres::connect(&conn, NoTls)
        .await
        .expect("connect via pgwire");
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });

    let stmt = client
        .prepare_typed("SELECT $1 AS n", &[Type::INT4])
        .await
        .expect("prepare_typed int4");
    let row = client
        .query_one(&stmt, &[&42_i32])
        .await
        .expect("parameterized int4 extended query");
    // Result cells are UTF-8 text on the wire; `prepare_typed` still covers INT4 *parameter* OIDs.
    let value: &str = row.get("n");
    assert_eq!(value, "42");

    connection_task.abort();
    server.abort();
}

#[tokio::test]
async fn pgwire_rejects_invalid_password() {
    let pg_port = free_port();
    let http_port = free_port();
    let mut config = ServerConfig::default();
    config.pg_port = pg_port;
    config.http_port = http_port;
    config.bootstrap_user = "admin".to_string();
    config.bootstrap_password = "password".to_string();

    let server = tokio::spawn(opensnow_server::run(config));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let conn = format!(
        "host=127.0.0.1 port={pg_port} user=admin password=wrong dbname=postgres"
    );
    assert!(
        tokio_postgres::connect(&conn, NoTls).await.is_err(),
        "connection with invalid password should fail"
    );

    server.abort();
}

#[tokio::test]
async fn pgwire_handles_session_lifecycle_commands() {
    let pg_port = free_port();
    let http_port = free_port();
    let mut config = ServerConfig::default();
    config.pg_port = pg_port;
    config.http_port = http_port;
    config.bootstrap_user = "admin".to_string();
    config.bootstrap_password = "password".to_string();

    let server = tokio::spawn(opensnow_server::run(config));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let conn = format!(
        "host=127.0.0.1 port={pg_port} user=admin password=password dbname=postgres"
    );
    let (client, connection) = tokio_postgres::connect(&conn, NoTls)
        .await
        .expect("connect via pgwire");
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });

    client
        .simple_query("BEGIN")
        .await
        .expect("begin should succeed");
    client
        .simple_query("SET application_name = 'opensnow-smoke'")
        .await
        .expect("set should succeed");
    client
        .simple_query("SHOW application_name")
        .await
        .expect("show should succeed");
    client
        .simple_query("COMMIT")
        .await
        .expect("commit should succeed");

    let row = client
        .query_one("SELECT 1 AS n", &[])
        .await
        .expect("query still works after session commands");
    let value: &str = row.get("n");
    assert_eq!(value, "1");

    client
        .simple_query("BEGIN")
        .await
        .expect("second begin should succeed");
    client
        .simple_query("ROLLBACK")
        .await
        .expect("rollback should succeed");

    connection_task.abort();
    server.abort();
}
