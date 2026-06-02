use crate::app_dirs::{display_path, ensure_app_dirs};
use crate::storage::{connect, now_iso, write_event};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const WORKSPACE_CONFIG: &str = "codex-automation.toml";

fn toml_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn slugify_id(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            Some(ch)
        } else if ch == '-' || ch.is_ascii_whitespace() {
            Some('-')
        } else {
            Some('-')
        };
        if let Some(next) = mapped {
            if next == '-' {
                if !previous_dash {
                    slug.push(next);
                }
                previous_dash = true;
            } else {
                slug.push(next);
                previous_dash = false;
            }
        }
    }
    let trimmed = slug.trim_matches(&['-', '.', '_'][..]).to_owned();
    if trimmed.is_empty() {
        "workspace".to_owned()
    } else {
        trimmed
    }
}

pub fn validate_target_id(target_id: &str) -> Result<()> {
    let mut chars = target_id.chars();
    let Some(first) = chars.next() else {
        bail!("invalid target id: {target_id}");
    };
    if !first.is_ascii_alphanumeric() {
        bail!("invalid target id: {target_id}");
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-') {
        bail!("invalid target id: {target_id}");
    }
    Ok(())
}

fn workspace_config_path(workspace: &Path) -> PathBuf {
    workspace.join(WORKSPACE_CONFIG)
}

fn workspace_id_from_config(workspace: &Path) -> Result<String> {
    let path = workspace_config_path(workspace);
    let text = fs::read_to_string(&path).with_context(|| {
        format!(
            "not a codex-automation workspace: {}",
            display_path(workspace)
        )
    })?;
    let value: toml::Value = text
        .parse()
        .with_context(|| format!("workspace config is invalid: {}", display_path(&path)))?;
    let workspace_id = value
        .get("workspace")
        .and_then(|section| section.get("id"))
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("workspace id is missing in {}", display_path(&path)))?;
    Ok(workspace_id.to_owned())
}

fn workspace_readme(workspace_name: &str) -> String {
    [
        format!("# {workspace_name}"),
        String::new(),
        "This is a thin codex-automation control workspace for Codex App.".to_owned(),
        String::new(),
        "Human-facing config lives here. App-managed state, SQLite, worktrees, logs,".to_owned(),
        "runner data, and artifacts live in the OS application data directory.".to_owned(),
        String::new(),
        "Useful commands:".to_owned(),
        String::new(),
        "```bash".to_owned(),
        "codex-automation db doctor --json".to_owned(),
        "codex-automation target list --json".to_owned(),
        "codex-automation worker list <target-id> --json".to_owned(),
        "codex-automation target pack <target-id> --json".to_owned(),
        "codex-automation heartbeat run <target-id> --json".to_owned(),
        "codex-automation result list <target-id> --json".to_owned(),
        "```".to_owned(),
        String::new(),
    ]
    .join("\n")
}

fn workspace_agents() -> String {
    [
        "# AGENTS.md",
        "",
        "This directory is the human-facing codex-automation control workspace.",
        "",
        "- Keep heavy runtime state out of this directory.",
        "- Use `codex-automation target list --json` before adding targets.",
        "- Customize workers under `workers/` and load them with `codex-automation worker add`.",
        "- Use `codex-automation heartbeat run <target-id> --json` for one bounded control-plane step.",
        "- Use `codex-automation result submit` to record worker results.",
        "- Do not edit app-managed SQLite, worktrees, logs, or runner state by hand.",
        "",
    ]
    .join("\n")
}

fn repo_discovery_worker() -> String {
    [
        "version = 1",
        "",
        "[worker]",
        "id = \"repo-discovery\"",
        "name = \"Repo Discovery\"",
        "description = \"Read-only repository inspection worker.\"",
        "skills = [\"repo-discovery\"]",
        "allowed_workorder_kinds = [\"repo_discovery\", \"log_analysis\"]",
        "sandbox = \"read-only\"",
        "approval_policy = \"never\"",
        "autonomy_profile = \"observe\"",
        "max_concurrency = 2",
        "instructions = \"Inspect source, docs, tests, and configuration. Do not edit target files.\"",
        "",
        "[config]",
        "result_contract = \"codex-automation result submit\"",
        "",
    ]
    .join("\n")
}

fn workspace_config_text(
    workspace_id: &str,
    workspace_name: &str,
    workspace: &Path,
) -> Result<String> {
    let dirs = ensure_app_dirs()?;
    Ok([
        "version = 1".to_owned(),
        String::new(),
        "[workspace]".to_owned(),
        format!("id = {}", toml_quote(workspace_id)),
        format!("name = {}", toml_quote(workspace_name)),
        format!("path = {}", toml_quote(&display_path(workspace))),
        String::new(),
        "[app_state]".to_owned(),
        format!("root = {}", toml_quote(&display_path(&dirs.state_root))),
        format!("database = {}", toml_quote(&display_path(&dirs.database))),
        format!("worktrees = {}", toml_quote(&display_path(&dirs.worktrees))),
        String::new(),
    ]
    .join("\n"))
}

fn write_text_if_missing(path: &Path, text: &str) -> Result<()> {
    if !path.exists() {
        fs::write(path, text).with_context(|| format!("failed to write {}", display_path(path)))?;
    }
    Ok(())
}

pub fn initialize_workspace(path: &Path, name: Option<&str>, overwrite: bool) -> Result<Value> {
    let workspace = path
        .expand_home()
        .with_context(|| format!("failed to resolve workspace path {}", display_path(path)))?;
    let workspace_name = name.unwrap_or_else(|| {
        workspace
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("codex-automation")
    });
    let workspace_id = slugify_id(workspace_name);
    fs::create_dir_all(workspace.join("targets"))?;
    fs::create_dir_all(workspace.join("workers"))?;
    fs::create_dir_all(workspace.join("reports"))?;
    let config_path = workspace_config_path(&workspace);
    if config_path.exists() && !overwrite {
        bail!(
            "workspace config already exists: {}",
            display_path(&config_path)
        );
    }
    fs::write(
        &config_path,
        workspace_config_text(&workspace_id, workspace_name, &workspace)?,
    )?;
    write_text_if_missing(
        &workspace.join("README.md"),
        &workspace_readme(workspace_name),
    )?;
    write_text_if_missing(&workspace.join("AGENTS.md"), &workspace_agents())?;
    write_text_if_missing(
        &workspace.join("workers").join("repo-discovery.toml"),
        &repo_discovery_worker(),
    )?;
    let now = now_iso();
    let conn = connect()?;
    conn.execute(
        "INSERT INTO workspaces(id, path, name, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
             path = excluded.path,
             name = excluded.name,
             updated_at = excluded.updated_at",
        params![
            workspace_id,
            display_path(&workspace),
            workspace_name,
            now,
            now
        ],
    )?;
    write_event(
        &conn,
        "workspace_initialized",
        None,
        None,
        &json!({"workspace_id": workspace_id, "workspace": display_path(&workspace)}),
    )?;
    Ok(json!({
        "status": "initialized",
        "workspace_id": workspace_id,
        "workspace": display_path(&workspace),
        "config_path": display_path(&config_path),
        "app_state": ensure_app_dirs()?.as_json(),
    }))
}

fn ensure_workspace_registered(conn: &Connection, workspace: &Path) -> Result<String> {
    let workspace_id = workspace_id_from_config(workspace)?;
    let path = workspace_config_path(workspace);
    let text = fs::read_to_string(&path)?;
    let value: toml::Value = text.parse()?;
    let name = value
        .get("workspace")
        .and_then(|section| section.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or(&workspace_id);
    let now = now_iso();
    conn.execute(
        "INSERT INTO workspaces(id, path, name, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
             path = excluded.path,
             name = excluded.name,
             updated_at = excluded.updated_at",
        params![workspace_id, display_path(workspace), name, now, now],
    )?;
    Ok(workspace_id)
}

pub fn workspace_status(path: &Path) -> Result<Value> {
    let workspace = path.expand_home()?;
    let workspace_id = workspace_id_from_config(&workspace)?;
    let conn = connect()?;
    let registered: Option<String> = conn
        .query_row(
            "SELECT id FROM workspaces WHERE id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .optional()?;
    let mut statement = conn.prepare(
        "SELECT id, workspace_id, repo_path, worktree_path, profile, status, created_at, updated_at
         FROM targets WHERE workspace_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map(params![workspace_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "workspace_id": row.get::<_, String>(1)?,
            "repo_path": row.get::<_, String>(2)?,
            "worktree_path": row.get::<_, String>(3)?,
            "profile": row.get::<_, String>(4)?,
            "status": row.get::<_, String>(5)?,
            "created_at": row.get::<_, String>(6)?,
            "updated_at": row.get::<_, String>(7)?,
        }))
    })?;
    let mut targets = Vec::new();
    for row in rows {
        targets.push(row?);
    }
    Ok(json!({
        "status": "ok",
        "workspace_id": workspace_id,
        "workspace": display_path(&workspace),
        "registered": registered.is_some(),
        "targets": targets,
    }))
}

fn target_config_text(
    target_id: &str,
    repo_path: &Path,
    workspace_id: &str,
    profile: &str,
    worktree_path: &Path,
) -> Result<String> {
    let dirs = ensure_app_dirs()?;
    Ok([
        "version = 1".to_owned(),
        String::new(),
        "[target]".to_owned(),
        format!("id = {}", toml_quote(target_id)),
        format!("workspace_id = {}", toml_quote(workspace_id)),
        format!("repo_path = {}", toml_quote(&display_path(repo_path))),
        format!("profile = {}", toml_quote(profile)),
        String::new(),
        "[app_state]".to_owned(),
        format!("database = {}", toml_quote(&display_path(&dirs.database))),
        format!(
            "worktree_path = {}",
            toml_quote(&display_path(worktree_path))
        ),
        String::new(),
    ]
    .join("\n"))
}

pub fn add_target(
    workspace: &Path,
    target_id: &str,
    repo_path: &Path,
    profile: &str,
) -> Result<Value> {
    validate_target_id(target_id)?;
    let repo = repo_path.expand_home()?;
    if !repo.is_dir() {
        bail!("target repo does not exist: {}", display_path(repo_path));
    }
    let workspace = workspace.expand_home()?;
    let dirs = ensure_app_dirs()?;
    let worktree_path = dirs.worktrees.join(target_id);
    let target_config_path = workspace.join("targets").join(format!("{target_id}.toml"));
    let now = now_iso();
    let conn = connect()?;
    let workspace_id = ensure_workspace_registered(&conn, &workspace)?;
    conn.execute(
        "INSERT INTO targets(id, workspace_id, repo_path, worktree_path, profile, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'registered', ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
             workspace_id = excluded.workspace_id,
             repo_path = excluded.repo_path,
             worktree_path = excluded.worktree_path,
             profile = excluded.profile,
             status = 'registered',
             updated_at = excluded.updated_at",
        params![
            target_id,
            workspace_id,
            display_path(&repo),
            display_path(&worktree_path),
            profile,
            now,
            now,
        ],
    )?;
    write_event(
        &conn,
        "target_registered",
        Some(target_id),
        None,
        &json!({"workspace_id": workspace_id, "repo_path": display_path(&repo), "profile": profile}),
    )?;
    fs::create_dir_all(
        target_config_path
            .parent()
            .context("target config path has no parent")?,
    )?;
    fs::write(
        &target_config_path,
        target_config_text(target_id, &repo, &workspace_id, profile, &worktree_path)?,
    )?;
    Ok(json!({
        "status": "registered",
        "target_id": target_id,
        "workspace": display_path(&workspace),
        "repo_path": display_path(&repo),
        "target_config_path": display_path(&target_config_path),
        "worktree_path": display_path(&worktree_path),
        "runtime_state_location": "app_state",
    }))
}

pub fn list_targets(workspace: Option<&Path>) -> Result<Value> {
    let conn = connect()?;
    let (workspace_id, query, params_value): (Option<String>, &str, Option<String>) = if let Some(
        workspace,
    ) =
        workspace
    {
        let resolved = workspace.expand_home()?;
        let workspace_id = workspace_id_from_config(&resolved)?;
        (
            Some(workspace_id.clone()),
            "SELECT id, workspace_id, repo_path, worktree_path, profile, status, created_at, updated_at
             FROM targets WHERE workspace_id = ?1 ORDER BY id",
            Some(workspace_id),
        )
    } else {
        (
            None,
            "SELECT id, workspace_id, repo_path, worktree_path, profile, status, created_at, updated_at
             FROM targets ORDER BY id",
            None,
        )
    };
    let mut statement = conn.prepare(query)?;
    let map_row = |row: &rusqlite::Row<'_>| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "workspace_id": row.get::<_, String>(1)?,
            "repo_path": row.get::<_, String>(2)?,
            "worktree_path": row.get::<_, String>(3)?,
            "profile": row.get::<_, String>(4)?,
            "status": row.get::<_, String>(5)?,
            "created_at": row.get::<_, String>(6)?,
            "updated_at": row.get::<_, String>(7)?,
        }))
    };
    let mut targets = Vec::new();
    if let Some(value) = params_value {
        for row in statement.query_map(params![value], map_row)? {
            targets.push(row?);
        }
    } else {
        for row in statement.query_map([], map_row)? {
            targets.push(row?);
        }
    }
    Ok(json!({"status": "ok", "workspace_id": workspace_id, "targets": targets}))
}

pub fn target_status(target_id: &str) -> Result<Value> {
    let conn = connect()?;
    let target: Option<Value> = conn
        .query_row(
            "SELECT id, workspace_id, repo_path, worktree_path, profile, status, created_at, updated_at
             FROM targets WHERE id = ?1",
            params![target_id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "workspace_id": row.get::<_, String>(1)?,
                    "repo_path": row.get::<_, String>(2)?,
                    "worktree_path": row.get::<_, String>(3)?,
                    "profile": row.get::<_, String>(4)?,
                    "target_status": row.get::<_, String>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                    "updated_at": row.get::<_, String>(7)?,
                }))
            },
        )
        .optional()?;
    let Some(mut payload) = target else {
        bail!("target is not registered: {target_id}");
    };
    let result_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM results WHERE target_id = ?1",
        params![target_id],
        |row| row.get(0),
    )?;
    let workorder_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM workorders WHERE target_id = ?1",
        params![target_id],
        |row| row.get(0),
    )?;
    let object = payload
        .as_object_mut()
        .context("target payload is not an object")?;
    object.insert("status".to_owned(), json!("ok"));
    object.insert("target_id".to_owned(), json!(target_id));
    object.insert("result_count".to_owned(), json!(result_count));
    object.insert("workorder_count".to_owned(), json!(workorder_count));
    Ok(payload)
}

trait ExpandHome {
    fn expand_home(&self) -> Result<PathBuf>;
}

impl ExpandHome for Path {
    fn expand_home(&self) -> Result<PathBuf> {
        let raw = self.to_string_lossy();
        if raw == "~" || raw.starts_with("~/") {
            let home = std::env::var_os("HOME").context("HOME is required for ~ expansion")?;
            let suffix = raw.strip_prefix("~/").unwrap_or("");
            Ok(PathBuf::from(home).join(suffix))
        } else {
            Ok(self.to_path_buf())
        }
        .and_then(|path| path.canonicalize().or_else(|_| Ok(path)))
    }
}
