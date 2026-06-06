use crate::app_dirs::{display_path, ensure_app_dirs};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub const SCHEMA_VERSION: i64 = 3;

const RESULT_STATUSES: &[&str] = &[
    "approval_required",
    "blocked",
    "discovery_findings",
    "discovery_no_findings",
    "failed",
    "fixed",
    "needs_more_investigation",
    "safe_fix_candidate",
    "staging_deploy_blocked",
    "staging_deployed",
    "stale_or_invalid",
    "tests_failed",
    "tests_passed",
];

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn connect() -> Result<Connection> {
    let dirs = ensure_app_dirs()?;
    if let Some(parent) = dirs.database.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create database parent {}", display_path(parent))
        })?;
    }
    let conn = Connection::open(&dirs.database)
        .with_context(|| format!("failed to open database {}", display_path(&dirs.database)))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    initialize_database(&conn)?;
    Ok(conn)
}

pub fn initialize_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS targets (
            id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            repo_path TEXT NOT NULL,
            worktree_path TEXT NOT NULL,
            profile TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
        );

        CREATE TABLE IF NOT EXISTS workorders (
            id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            title TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(id, target_id),
            FOREIGN KEY(target_id) REFERENCES targets(id)
        );

        CREATE TABLE IF NOT EXISTS workers (
            id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL,
            skills_json TEXT NOT NULL,
            allowed_workorder_kinds_json TEXT NOT NULL,
            sandbox TEXT NOT NULL,
            approval_policy TEXT NOT NULL,
            autonomy_profile TEXT NOT NULL,
            max_concurrency INTEGER NOT NULL,
            instructions TEXT NOT NULL,
            config_json TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(id, target_id),
            FOREIGN KEY(target_id) REFERENCES targets(id)
        );

        CREATE TABLE IF NOT EXISTS results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target_id TEXT NOT NULL,
            workorder_id TEXT NOT NULL,
            status TEXT NOT NULL,
            summary TEXT NOT NULL,
            next_action TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(target_id) REFERENCES targets(id)
        );

        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            target_id TEXT,
            workorder_id TEXT,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS approvals (
            id TEXT PRIMARY KEY,
            target_id TEXT NOT NULL,
            workorder_id TEXT NOT NULL,
            status TEXT NOT NULL,
            reason TEXT NOT NULL,
            decision TEXT,
            decided_by TEXT,
            message TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(target_id) REFERENCES targets(id)
        );

        CREATE TABLE IF NOT EXISTS loop_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target_id TEXT NOT NULL,
            status TEXT NOT NULL,
            summary TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(target_id) REFERENCES targets(id)
        );

        CREATE TABLE IF NOT EXISTS runner_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target_id TEXT NOT NULL,
            workorder_id TEXT NOT NULL,
            status TEXT NOT NULL,
            command_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(target_id) REFERENCES targets(id)
        );
        ",
    )?;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

pub fn db_doctor() -> Result<Value> {
    let dirs = ensure_app_dirs()?;
    let conn = connect()?;
    let schema_version: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let target_count: i64 = conn.query_row("SELECT COUNT(*) FROM targets", [], |row| row.get(0))?;
    let result_count: i64 = conn.query_row("SELECT COUNT(*) FROM results", [], |row| row.get(0))?;
    let workorder_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM workorders", [], |row| row.get(0))?;
    let worker_count: i64 = conn.query_row("SELECT COUNT(*) FROM workers", [], |row| row.get(0))?;
    let approval_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM approvals", [], |row| row.get(0))?;
    let loop_run_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM loop_runs", [], |row| row.get(0))?;
    let runner_run_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM runner_runs", [], |row| row.get(0))?;
    Ok(json!({
        "status": "ok",
        "schema_version": schema_version.and_then(|value| value.parse::<i64>().ok()),
        "database": display_path(&dirs.database),
        "state_root": display_path(&dirs.state_root),
        "worktrees": display_path(&dirs.worktrees),
        "target_count": target_count,
        "result_count": result_count,
        "workorder_count": workorder_count,
        "worker_count": worker_count,
        "approval_count": approval_count,
        "loop_run_count": loop_run_count,
        "runner_run_count": runner_run_count,
    }))
}

pub fn db_migrate() -> Result<Value> {
    let conn = connect()?;
    let schema_version: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(json!({
        "status": "migrated",
        "schema_version": schema_version.and_then(|value| value.parse::<i64>().ok()),
    }))
}

pub fn write_event(
    conn: &Connection,
    kind: &str,
    target_id: Option<&str>,
    workorder_id: Option<&str>,
    payload: &Value,
) -> Result<()> {
    conn.execute(
        "INSERT INTO events(kind, target_id, workorder_id, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            kind,
            target_id,
            workorder_id,
            serde_json::to_string(payload)?,
            now_iso()
        ],
    )?;
    Ok(())
}

pub fn ensure_target_exists(conn: &Connection, target_id: &str) -> Result<()> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM targets WHERE id = ?1",
            params![target_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        bail!("target is not registered: {target_id}");
    }
    Ok(())
}

fn required_string(payload: &Value, key: &str) -> Result<String> {
    let value = payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("result.{key} is required"))?;
    Ok(value.to_owned())
}

fn workorder_status_for_result(status: &str) -> &'static str {
    match status {
        "discovery_no_findings" | "fixed" | "staging_deployed" | "tests_passed" => "completed",
        "approval_required" | "safe_fix_candidate" => "needs_user",
        "failed" | "blocked" | "stale_or_invalid" | "staging_deploy_blocked" => "failed",
        _ => "result_submitted",
    }
}

pub fn submit_result(conn: &mut Connection, target_id: &str, payload: Value) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let workorder_id = required_string(&payload, "workorder_id")?;
    let status = required_string(&payload, "status")?;
    let summary = required_string(&payload, "summary")?;
    let next_action = required_string(&payload, "next_action")?;
    if !RESULT_STATUSES.contains(&status.as_str()) {
        bail!("unsupported result status: {status}");
    }
    let now = now_iso();
    let workorder_status = workorder_status_for_result(&status);
    let encoded = serde_json::to_string(&payload)?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO workorders(id, target_id, kind, status, title, payload_json, created_at, updated_at)
         VALUES (?1, ?2, 'external_result', ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id, target_id) DO UPDATE SET
             status = excluded.status,
             payload_json = excluded.payload_json,
             updated_at = excluded.updated_at",
        params![
            workorder_id,
            target_id,
            workorder_status,
            format!("Result for {workorder_id}"),
            encoded,
            now,
            now,
        ],
    )?;
    tx.execute(
        "INSERT INTO results(target_id, workorder_id, status, summary, next_action, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![target_id, workorder_id, status, summary, next_action, encoded, now],
    )?;
    let result_id = tx.last_insert_rowid();
    write_event(
        &tx,
        "result_submitted",
        Some(target_id),
        Some(&workorder_id),
        &json!({"result_id": result_id, "status": status, "next_action": next_action}),
    )?;
    tx.commit()?;
    Ok(json!({
        "status": "recorded",
        "target_id": target_id,
        "workorder_id": workorder_id,
        "result_id": result_id,
        "result_status": status,
        "workorder_status": workorder_status,
    }))
}

pub fn list_results(conn: &Connection, target_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let mut statement = conn.prepare(
        "SELECT id, target_id, workorder_id, status, summary, next_action, created_at
         FROM results
         WHERE target_id = ?1
         ORDER BY id",
    )?;
    let rows = statement.query_map(params![target_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "target_id": row.get::<_, String>(1)?,
            "workorder_id": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "summary": row.get::<_, String>(4)?,
            "next_action": row.get::<_, String>(5)?,
            "created_at": row.get::<_, String>(6)?,
        }))
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(json!({"status": "ok", "target_id": target_id, "results": results}))
}
