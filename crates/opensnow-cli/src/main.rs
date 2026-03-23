//! # opensnow CLI
//!
//! SnowSQL-compatible command-line client for OpenSnow.
//!
//! ## Usage
//!
//! ```text
//! # Interactive REPL
//! opensnow --account mycluster.example.com --user alice --database MYDB
//!
//! # Execute a file
//! opensnow -f path/to/queries.sql
//!
//! # Execute an inline query
//! opensnow -q "SELECT CURRENT_TIMESTAMP()"
//!
//! # Pipe SQL from stdin
//! cat queries.sql | opensnow --stdin
//! ```
//!
//! ## SnowSQL compatibility
//!
//! The CLI mirrors SnowSQL's interface so that teams can replace `snowsql`
//! with `opensnow` in their scripts with no changes:
//!
//! - `--account / -a`    — account identifier (maps to server hostname)
//! - `--username / -u`   — user name
//! - `--dbname / -d`     — default database
//! - `--schemaname / -s` — default schema
//! - `--rolename / -r`   — active role
//! - `--warehouse / -w`  — active virtual warehouse
//! - `--query / -q`      — inline SQL query
//! - `--filename / -f`   — SQL file(s) to execute
//! - `--stdin / -i`      — read SQL from stdin
//! - `--variable / -D`   — key=value variable for template substitution
//! - `--format`          — output format: TABLE (default), JSON, CSV
//!
//! ## Interactive mode commands
//!
//! - `!source <file>` — execute SQL from a local file
//! - `!queries`       — list queries executed in this session
//! - `!result <n>`    — display result of query n
//! - `!abort`         — cancel the running query
//! - `!edit`          — open $EDITOR to compose a query
//! - `!exit` / `!quit` — end the session

fn main() {
    // TODO: parse CLI arguments with clap
    // TODO: load connection config (~/.opensnow/config or env vars)
    // TODO: establish connection (PostgreSQL wire protocol or Snowflake REST API v2)
    // TODO: dispatch to interactive REPL or batch execution mode
    println!("opensnow CLI — not yet implemented");
}
