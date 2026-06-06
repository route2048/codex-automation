use crate::app_dirs::{display_path, ensure_app_dirs};
use crate::storage::{ensure_target_exists, now_iso, submit_result, write_event};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default)]
struct PromptCustomInstructions {
    control_plane: String,
    target: String,
    worker: String,
}

fn required(value: &str, name: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{name} is required");
    }
    Ok(trimmed.to_owned())
}

fn ensure_workorder_exists(conn: &Connection, target_id: &str, workorder_id: &str) -> Result<()> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM workorders WHERE target_id = ?1 AND id = ?2",
            params![target_id, workorder_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        bail!("workorder is not registered: {workorder_id}");
    }
    Ok(())
}

fn path_segment(value: &str, name: &str) -> Result<String> {
    let trimmed = required(value, name)?;
    if trimmed == "." || trimmed == ".." || trimmed.contains('/') || trimmed.contains('\\') {
        bail!("{name} must be a plain id without path separators");
    }
    Ok(trimmed)
}

fn required_field<'a>(payload: &'a Value, key: &str) -> Result<&'a Value> {
    payload
        .get(key)
        .with_context(|| format!("worker.{key} is required"))
}

fn required_string_field(payload: &Value, key: &str) -> Result<String> {
    let value = required_field(payload, key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("worker.{key} must be a non-empty string"))?;
    Ok(value.to_owned())
}

fn required_string_array(payload: &Value, key: &str) -> Result<Vec<String>> {
    let values = required_field(payload, key)?
        .as_array()
        .with_context(|| format!("worker.{key} must be an array"))?;
    let mut out = Vec::new();
    for value in values {
        let item = value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("worker.{key} items must be non-empty strings"))?;
        out.push(item.to_owned());
    }
    if out.is_empty() {
        bail!("worker.{key} must contain at least one item");
    }
    Ok(out)
}

fn optional_string_field(payload: &Value, key: &str) -> Result<String> {
    match payload.get(key) {
        Some(value) => {
            let text = value
                .as_str()
                .with_context(|| format!("worker.{key} must be a string"))?;
            Ok(text.trim().to_owned())
        }
        None => Ok(String::new()),
    }
}

fn worker_payload(payload: &Value) -> Result<&Value> {
    let worker = payload
        .get("worker")
        .context("worker TOML must contain a [worker] table")?;
    if !worker.is_object() {
        bail!("worker TOML [worker] must be an object");
    }
    Ok(worker)
}

fn custom_instructions_from_toml(path: &Path, section: &str) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", display_path(path)))?;
    let value: toml::Value = text
        .parse()
        .with_context(|| format!("TOML is invalid: {}", display_path(path)))?;
    let Some(section_value) = value.get(section) else {
        return Ok(String::new());
    };
    let Some(instructions_value) = section_value.get("custom_instructions") else {
        return Ok(String::new());
    };
    let instructions = instructions_value.as_str().with_context(|| {
        format!(
            "{section}.custom_instructions must be a string in {}",
            display_path(path)
        )
    })?;
    Ok(instructions.trim().to_owned())
}

fn control_workspace_path(conn: &Connection, target_id: &str) -> Result<PathBuf> {
    conn.query_row(
        "SELECT w.path
         FROM workspaces w
         INNER JOIN targets t ON t.workspace_id = w.id
         WHERE t.id = ?1",
        params![target_id],
        |row| row.get::<_, String>(0),
    )
    .map(PathBuf::from)
    .optional()?
    .with_context(|| format!("control workspace is not registered for target: {target_id}"))
}

fn prompt_custom_instructions(
    conn: &Connection,
    target_id: &str,
    worker: &Option<Value>,
) -> Result<PromptCustomInstructions> {
    let control_workspace = control_workspace_path(conn, target_id)?;
    let control_plane = custom_instructions_from_toml(
        &control_workspace.join("workers").join("control-plane.toml"),
        "worker",
    )?;
    let target = custom_instructions_from_toml(
        &control_workspace
            .join("targets")
            .join(format!("{target_id}.toml")),
        "target",
    )?;
    let worker = worker
        .as_ref()
        .and_then(|payload| payload.get("custom_instructions"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    Ok(PromptCustomInstructions {
        control_plane,
        target,
        worker,
    })
}

fn instruction_block(label: &str, text: &str) -> String {
    if text.trim().is_empty() {
        format!("### {label}\n\nNo custom instructions configured.\n")
    } else {
        format!("### {label}\n\n{}\n", text.trim())
    }
}

fn automation_binary_path() -> String {
    env::current_exe()
        .ok()
        .map(|path| display_path(&path))
        .unwrap_or_else(|| "codex-automation".to_owned())
}

fn result_submit_prefix(target_id: &str, workorder_id: &str) -> String {
    format!(
        "{} result submit {target_id} --workorder-id {workorder_id}",
        automation_binary_path()
    )
}

fn workorder_target_pack_path(workorder: &Value) -> Option<String> {
    workorder
        .pointer("/payload/target_pack_path")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn repository_state_block(workorder: &Value) -> String {
    let Some(pack_path) = workorder_target_pack_path(workorder) else {
        return "Target pack: not recorded on this workorder.\n".to_owned();
    };
    let mut lines = vec![format!("Target pack: {pack_path}")];
    let pack = fs::read_to_string(&pack_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    if let Some(git) = pack.as_ref().and_then(|value| value.pointer("/git")) {
        lines.push("Git state from target pack:".to_owned());
        lines.push("```json".to_owned());
        lines.push(serde_json::to_string_pretty(git).unwrap_or_else(|_| "{}".to_owned()));
        lines.push("```".to_owned());
    } else {
        lines.push(
            "Git state: unavailable; inspect the target checkout before assuming it is clean."
                .to_owned(),
        );
    }
    lines.join("\n")
}

fn ensure_worker_matches_workorder(
    conn: &Connection,
    target_id: &str,
    workorder_id: &str,
    worker_id: &str,
) -> Result<Value> {
    let workorder_kind: String = conn.query_row(
        "SELECT kind FROM workorders WHERE target_id = ?1 AND id = ?2",
        params![target_id, workorder_id],
        |row| row.get(0),
    )?;
    let worker = get_worker(conn, target_id, worker_id)?;
    let worker_object = worker
        .as_object()
        .context("worker payload is not an object")?;
    let allowed = worker_object
        .get("allowed_workorder_kinds")
        .and_then(Value::as_array)
        .context("worker allowed_workorder_kinds missing")?;
    let is_allowed = allowed
        .iter()
        .filter_map(Value::as_str)
        .any(|kind| kind == workorder_kind);
    if !is_allowed {
        bail!("worker {worker_id} does not allow workorder kind {workorder_kind}");
    }
    Ok(worker)
}

fn target_payload(conn: &Connection, target_id: &str) -> Result<Value> {
    conn.query_row(
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
    .optional()?
    .with_context(|| format!("target is not registered: {target_id}"))
}

fn first_ready_workorder(conn: &Connection, target_id: &str) -> Result<Option<Value>> {
    conn.query_row(
        "SELECT id, target_id, kind, status, title, payload_json, created_at, updated_at
         FROM workorders
         WHERE target_id = ?1 AND status = 'ready_for_worker'
         ORDER BY created_at, id LIMIT 1",
        params![target_id],
        |row| {
            let payload_text: String = row.get(5)?;
            let payload: Value = serde_json::from_str(&payload_text).unwrap_or_else(|_| json!({}));
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "target_id": row.get::<_, String>(1)?,
                "kind": row.get::<_, String>(2)?,
                "workorder_status": row.get::<_, String>(3)?,
                "title": row.get::<_, String>(4)?,
                "payload": payload,
                "created_at": row.get::<_, String>(6)?,
                "updated_at": row.get::<_, String>(7)?,
            }))
        },
    )
    .optional()
    .context("failed to query ready workorder")
}

fn worker_allows_kind(worker: &Value, kind: &str) -> bool {
    worker
        .get("allowed_workorder_kinds")
        .and_then(Value::as_array)
        .map(|allowed| {
            allowed
                .iter()
                .filter_map(Value::as_str)
                .any(|allowed_kind| allowed_kind == kind)
        })
        .unwrap_or(false)
}

fn select_worker_for_kind(conn: &Connection, target_id: &str, kind: &str) -> Result<Value> {
    let workers = list_workers(conn, target_id)?;
    let rows = workers
        .get("workers")
        .and_then(Value::as_array)
        .context("workers payload is missing")?;
    rows.iter()
        .find(|worker| {
            worker
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                == "active"
                && worker_allows_kind(worker, kind)
        })
        .cloned()
        .with_context(|| format!("no active worker allows workorder kind {kind}"))
}

fn render_runner_prompt(
    target_id: &str,
    workorder_id: &str,
    target: &Value,
    workorder: &Value,
    worker: &Option<Value>,
    custom_instructions: &PromptCustomInstructions,
) -> Result<String> {
    let repo_path = target
        .get("repo_path")
        .and_then(Value::as_str)
        .context("target.repo_path is missing")?;
    let workorder_kind = workorder
        .get("kind")
        .and_then(Value::as_str)
        .context("workorder.kind is missing")?;
    let workorder_title = workorder
        .get("title")
        .and_then(Value::as_str)
        .context("workorder.title is missing")?;
    let worker_name = worker
        .as_ref()
        .and_then(|payload| payload.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Unassigned worker");
    let worker_id = worker
        .as_ref()
        .and_then(|payload| payload.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("unassigned");
    let sandbox = worker
        .as_ref()
        .and_then(|payload| payload.get("sandbox"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let worker_block = serde_json::to_string_pretty(worker)?;
    let workorder_block = serde_json::to_string_pretty(workorder)?;
    let control_plane_instructions = instruction_block(
        "Control Plane (`workers/control-plane.toml`)",
        &custom_instructions.control_plane,
    );
    let target_instructions = instruction_block(
        "Target (`targets/<target-id>.toml`)",
        &custom_instructions.target,
    );
    let worker_instructions = instruction_block(
        "Worker (`workers/<worker-id>.toml`)",
        &custom_instructions.worker,
    );
    let result_command = result_submit_prefix(target_id, workorder_id);
    let repository_state = repository_state_block(workorder);
    let result_examples = if sandbox == "read-only" || workorder_kind.contains("discovery") {
        format!(
            r#"{result_command} --status discovery_no_findings --summary "..." --next-action no_action --json
{result_command} --status discovery_findings --summary "..." --next-action create_followup_workorder --json
{result_command} --status needs_more_investigation --summary "..." --next-action create_followup_workorder --json
{result_command} --status blocked --summary "..." --next-action needs_user --json"#
        )
    } else {
        format!(
            r#"{result_command} --status fixed --summary "..." --next-action no_action --json
{result_command} --status safe_fix_candidate --summary "..." --next-action needs_user --json
{result_command} --status tests_passed --summary "..." --next-action no_action --json
{result_command} --status blocked --summary "..." --next-action needs_user --json"#
        )
    };

    Ok(format!(
        r#"# Codex Automation Worker Handoff

You are acting as a Codex automation worker from a Codex App handoff.

## Assignment

- Target: {target_id}
- Repository path: {repo_path}
- Workorder: {workorder_id}
- Workorder kind: {workorder_kind}
- Workorder title: {workorder_title}
- Worker: {worker_name} ({worker_id})

## Repository State

{repository_state}

## Operating Boundaries

- Obey the target repository instructions, including AGENTS.md files.
- Do not specify model or model_reasoning_effort in Codex calls.
- Do not push, deploy, delete data, or start long-running services unless the workorder explicitly grants that authority.
- Respect the worker sandbox, approval policy, autonomy profile, and instructions below.
- Record completion through the codex-automation CLI when available. Do not edit SQLite or app-state files by hand.

## Custom Instructions

Apply these custom instructions in order after system/developer instructions and
target repository AGENTS.md files.

{control_plane_instructions}
{target_instructions}
{worker_instructions}

## Required Result Contract

When finished, prefer running one of these forms:

```bash
{result_examples}
```

If the CLI submission is unavailable, make your final response exactly one JSON
object with these fields: `workorder_id`, `status`, `summary`, and
`next_action`. The controller can save that object to the package `result.json`
and ingest it with the runner refresh command from `handoff.md`.

## Worker Definition

```json
{worker_block}
```

## Workorder

```json
{workorder_block}
```
"#
    ))
}

fn is_ignored_scan_dir(name: &str) -> bool {
    if name.starts_with('.') && name != ".github" {
        return true;
    }
    matches!(
        name,
        "__pycache__"
            | "build"
            | "coverage"
            | "dist"
            | "logs"
            | "node_modules"
            | "out"
            | "output"
            | "playwright-report"
            | "target"
            | "tmp"
            | "vendor"
    )
}

fn is_sensitive_scan_file(name: &str, relative_path: &str) -> bool {
    let lower_name = name.to_ascii_lowercase();
    let lower_path = relative_path.to_ascii_lowercase();
    lower_name.starts_with('.')
        || lower_name == ".env"
        || lower_name.starts_with(".env.")
        || lower_name.ends_with(".env")
        || lower_name.ends_with(".env.example")
        || lower_name.ends_with(".env.sample")
        || lower_name.ends_with(".pem")
        || lower_name.ends_with(".key")
        || lower_name.ends_with(".p12")
        || lower_name.ends_with(".pfx")
        || lower_name.ends_with(".pyc")
        || lower_name.ends_with(".sqlite")
        || lower_name.ends_with(".sqlite3")
        || lower_name.ends_with(".db")
        || lower_name == "token.json"
        || lower_name == "tokens.json"
        || lower_name == "credentials.json"
        || lower_name == "credentials.db"
        || lower_name == "access_tokens.db"
        || lower_path.contains("/.env.")
        || lower_path.contains("/.runtime/")
        || lower_path.contains("/.backup/")
}

fn language_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some("typescript-javascript"),
        "go" => Some("go"),
        "java" | "kt" => Some("jvm"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "cs" => Some("dotnet"),
        "swift" => Some("swift"),
        "sql" => Some("sql"),
        "md" | "mdx" => Some("docs"),
        _ => None,
    }
}

fn scan_repo_dir(
    root: &Path,
    dir: &Path,
    depth: usize,
    language_counts: &mut BTreeMap<String, usize>,
    markers: &mut Vec<String>,
    sample_files: &mut Vec<String>,
    max_files: &mut usize,
    has_tests: &mut bool,
) -> Result<()> {
    if depth > 5 || *max_files == 0 {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed to scan {}", display_path(dir)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if *max_files == 0 {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if is_ignored_scan_dir(&name) {
                continue;
            }
            scan_repo_dir(
                root,
                &path,
                depth + 1,
                language_counts,
                markers,
                sample_files,
                max_files,
                has_tests,
            )?;
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
        let rel_text = rel.into_owned();
        if is_sensitive_scan_file(&name, &rel_text) {
            continue;
        }
        *max_files -= 1;
        let lower_rel = rel_text.to_ascii_lowercase();
        if lower_rel.contains("test") || lower_rel.contains("spec") {
            *has_tests = true;
        }
        if sample_files.len() < 80 {
            sample_files.push(rel_text.clone());
        }
        if matches!(
            name.as_str(),
            "AGENTS.md"
                | "Cargo.toml"
                | "Dockerfile"
                | "Makefile"
                | "README.md"
                | "docker-compose.yml"
                | "package.json"
                | "pnpm-workspace.yaml"
                | "pyproject.toml"
                | "requirements.txt"
        ) || rel_text.starts_with(".github/workflows/")
            || rel_text.contains("/migrations/")
        {
            markers.push(rel_text.clone());
        }
        if let Some(language) = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(language_for_extension)
        {
            *language_counts.entry(language.to_owned()).or_insert(0) += 1;
        }
    }
    Ok(())
}

fn repo_scan(root: &Path) -> Result<Value> {
    let mut language_counts = BTreeMap::new();
    let mut markers = Vec::new();
    let mut sample_files = Vec::new();
    let mut remaining = 5000;
    let mut has_tests = false;
    scan_repo_dir(
        root,
        root,
        0,
        &mut language_counts,
        &mut markers,
        &mut sample_files,
        &mut remaining,
        &mut has_tests,
    )?;
    markers.sort();
    markers.dedup();
    let mut dominant = language_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(language, _)| language.clone())
        .unwrap_or_else(|| "unknown".to_owned());
    if language_counts.len() > 1 && dominant != "unknown" {
        dominant = "mixed".to_owned();
    }
    let suggested_workers = if has_tests {
        json!(["repo-maintainer", "ops-analyst", "release-manager"])
    } else {
        json!(["repo-maintainer", "ops-analyst"])
    };
    Ok(json!({
        "dominant_profile": dominant,
        "language_counts": language_counts,
        "markers": markers,
        "sample_files": sample_files,
        "scan_limit_reached": remaining == 0,
        "has_tests": has_tests,
        "suggested_workers": suggested_workers,
    }))
}

fn run_git(root: &Path, args: &[&str]) -> std::result::Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let message = if stderr.is_empty() { stdout } else { stderr };
        return Err(message);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn inspect_git_state(root: &Path) -> Value {
    let head = match run_git(root, &["rev-parse", "HEAD"]) {
        Ok(value) => value,
        Err(error) => {
            return json!({
                "status": "not_available",
                "reason": error,
                "dirty": null,
            })
        }
    };
    let branch = run_git(root, &["branch", "--show-current"]).unwrap_or_default();
    let status_text = run_git(root, &["status", "--porcelain=v1", "--branch"])
        .unwrap_or_else(|error| format!("## status unavailable: {error}"));
    let mut branch_status = String::new();
    let mut staged = 0;
    let mut unstaged = 0;
    let mut untracked = 0;
    let mut changed = 0;
    for line in status_text.lines() {
        if line.starts_with("## ") {
            branch_status = line.to_owned();
            continue;
        }
        if line.starts_with("??") {
            untracked += 1;
            changed += 1;
            continue;
        }
        let bytes = line.as_bytes();
        if bytes.first().copied().unwrap_or(b' ') != b' ' {
            staged += 1;
        }
        if bytes.get(1).copied().unwrap_or(b' ') != b' ' {
            unstaged += 1;
        }
        changed += 1;
    }
    json!({
        "status": "ok",
        "branch": branch,
        "head": head,
        "branch_status": branch_status,
        "dirty": changed > 0,
        "changed_count": changed,
        "staged_count": staged,
        "unstaged_count": unstaged,
        "untracked_count": untracked,
    })
}

pub fn generate_target_pack(conn: &Connection, target_id: &str) -> Result<Value> {
    let target = target_payload(conn, target_id)?;
    let repo_path = PathBuf::from(value_string(&target, "/repo_path", "target repo path")?);
    let scan = repo_scan(&repo_path)?;
    let git = inspect_git_state(&repo_path);
    let dirs = ensure_app_dirs()?;
    let target_segment = path_segment(target_id, "target id")?;
    let pack_root = dirs.artifacts.join("targets").join(target_segment);
    fs::create_dir_all(&pack_root)
        .with_context(|| format!("failed to create {}", display_path(&pack_root)))?;
    let pack_path = pack_root.join("target-pack.json");
    let instructions_path = pack_root.join("instructions.md");
    let generated_at = now_iso();
    let pack = json!({
        "version": 1,
        "target_id": target_id,
        "generated_at": generated_at,
        "target": target,
        "git": git,
        "scan": scan,
    });
    fs::write(&pack_path, serde_json::to_string_pretty(&pack)?)
        .with_context(|| format!("failed to write {}", display_path(&pack_path)))?;
    let instructions = format!(
        "# Target Pack: {target_id}\n\nGenerated at: {generated_at}\n\nUse this pack as repository context for workorder generation. Prefer read-only discovery before edits. Record results through `codex-automation result submit` or runner final JSON.\n"
    );
    fs::write(&instructions_path, instructions)
        .with_context(|| format!("failed to write {}", display_path(&instructions_path)))?;
    write_event(
        conn,
        "target_pack_generated",
        Some(target_id),
        None,
        &json!({
            "pack_path": display_path(&pack_path),
            "instructions_path": display_path(&instructions_path)
        }),
    )?;
    Ok(json!({
        "status": "generated",
        "target_id": target_id,
        "pack_path": display_path(&pack_path),
        "instructions_path": display_path(&instructions_path),
        "pack": pack,
    }))
}

fn value_string(value: &Value, pointer: &str, name: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("{name} is missing"))
}

fn set_runner_state(
    conn: &Connection,
    target_id: &str,
    runner_id: i64,
    status: &str,
    command: &Value,
) -> Result<()> {
    let updated_at = now_iso();
    conn.execute(
        "UPDATE runner_runs SET status = ?1, command_json = ?2, updated_at = ?3
         WHERE target_id = ?4 AND id = ?5",
        params![
            status,
            serde_json::to_string(command)?,
            updated_at,
            target_id,
            runner_id
        ],
    )?;
    if let Some(command_path) = command
        .pointer("/package/command_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
    {
        fs::write(&command_path, serde_json::to_string_pretty(command)?)
            .with_context(|| format!("failed to write {}", display_path(&command_path)))?;
    }
    Ok(())
}

fn result_count_for_workorder(
    conn: &Connection,
    target_id: &str,
    workorder_id: &str,
) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM results WHERE target_id = ?1 AND workorder_id = ?2",
        params![target_id, workorder_id],
        |row| row.get(0),
    )
    .context("failed to count workorder results")
}

pub fn add_worker(conn: &Connection, target_id: &str, payload: Value) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let worker = worker_payload(&payload)?;
    let worker_id = required_string_field(worker, "id")?;
    let name = required_string_field(worker, "name")?;
    let description = required_string_field(worker, "description")?;
    let skills = required_string_array(worker, "skills")?;
    let allowed_workorder_kinds = required_string_array(worker, "allowed_workorder_kinds")?;
    let sandbox = required_string_field(worker, "sandbox")?;
    let approval_policy = required_string_field(worker, "approval_policy")?;
    let autonomy_profile = required_string_field(worker, "autonomy_profile")?;
    let max_concurrency = required_field(worker, "max_concurrency")?
        .as_i64()
        .filter(|value| *value > 0)
        .context("worker.max_concurrency must be a positive integer")?;
    let custom_instructions = optional_string_field(worker, "custom_instructions")?;
    let config = payload.get("config").cloned().unwrap_or_else(|| json!({}));
    if !config.is_object() {
        bail!("worker config must be an object when present");
    }
    let now = now_iso();
    conn.execute(
        "INSERT INTO workers(
            id, target_id, name, description, status, skills_json,
            allowed_workorder_kinds_json, sandbox, approval_policy, autonomy_profile,
            max_concurrency, instructions, config_json, payload_json, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(id, target_id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            status = 'active',
            skills_json = excluded.skills_json,
            allowed_workorder_kinds_json = excluded.allowed_workorder_kinds_json,
            sandbox = excluded.sandbox,
            approval_policy = excluded.approval_policy,
            autonomy_profile = excluded.autonomy_profile,
            max_concurrency = excluded.max_concurrency,
            instructions = excluded.instructions,
            config_json = excluded.config_json,
            payload_json = excluded.payload_json,
            updated_at = excluded.updated_at",
        params![
            worker_id,
            target_id,
            name,
            description,
            serde_json::to_string(&skills)?,
            serde_json::to_string(&allowed_workorder_kinds)?,
            sandbox,
            approval_policy,
            autonomy_profile,
            max_concurrency,
            custom_instructions,
            serde_json::to_string(&config)?,
            serde_json::to_string(&payload)?,
            now,
            now,
        ],
    )?;
    write_event(
        conn,
        "worker_registered",
        Some(target_id),
        None,
        &json!({"worker_id": worker_id, "skills": skills, "allowed_workorder_kinds": allowed_workorder_kinds}),
    )?;
    Ok(json!({
        "status": "registered",
        "target_id": target_id,
        "worker_id": worker_id,
        "worker_status": "active",
        "name": name,
        "skills": skills,
        "allowed_workorder_kinds": allowed_workorder_kinds,
        "sandbox": sandbox,
        "approval_policy": approval_policy,
        "autonomy_profile": autonomy_profile,
        "max_concurrency": max_concurrency,
        "custom_instructions": custom_instructions,
    }))
}

pub fn list_workers(conn: &Connection, target_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let mut statement = conn.prepare(
        "SELECT id, target_id, name, description, status, skills_json, allowed_workorder_kinds_json,
                sandbox, approval_policy, autonomy_profile, max_concurrency, instructions, config_json,
                created_at, updated_at
         FROM workers WHERE target_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map(params![target_id], |row| {
        let skills_text: String = row.get(5)?;
        let allowed_text: String = row.get(6)?;
        let config_text: String = row.get(12)?;
        let skills: Value = serde_json::from_str(&skills_text).unwrap_or_else(|_| json!([]));
        let allowed: Value = serde_json::from_str(&allowed_text).unwrap_or_else(|_| json!([]));
        let config: Value = serde_json::from_str(&config_text).unwrap_or_else(|_| json!({}));
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "target_id": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "description": row.get::<_, String>(3)?,
            "status": row.get::<_, String>(4)?,
            "skills": skills,
            "allowed_workorder_kinds": allowed,
            "sandbox": row.get::<_, String>(7)?,
            "approval_policy": row.get::<_, String>(8)?,
            "autonomy_profile": row.get::<_, String>(9)?,
            "max_concurrency": row.get::<_, i64>(10)?,
            "custom_instructions": row.get::<_, String>(11)?,
            "config": config,
            "created_at": row.get::<_, String>(13)?,
            "updated_at": row.get::<_, String>(14)?,
        }))
    })?;
    let mut workers = Vec::new();
    for row in rows {
        workers.push(row?);
    }
    Ok(json!({"status": "ok", "target_id": target_id, "workers": workers}))
}

pub fn get_worker(conn: &Connection, target_id: &str, worker_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let row: Option<Value> = conn
        .query_row(
            "SELECT id, target_id, name, description, status, skills_json, allowed_workorder_kinds_json,
                    sandbox, approval_policy, autonomy_profile, max_concurrency, instructions, config_json,
                    created_at, updated_at
             FROM workers WHERE target_id = ?1 AND id = ?2",
            params![target_id, worker_id],
            |row| {
                let skills_text: String = row.get(5)?;
                let allowed_text: String = row.get(6)?;
                let config_text: String = row.get(12)?;
                let skills: Value = serde_json::from_str(&skills_text).unwrap_or_else(|_| json!([]));
                let allowed: Value = serde_json::from_str(&allowed_text).unwrap_or_else(|_| json!([]));
                let config: Value = serde_json::from_str(&config_text).unwrap_or_else(|_| json!({}));
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "target_id": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "description": row.get::<_, String>(3)?,
                    "worker_status": row.get::<_, String>(4)?,
                    "skills": skills,
                    "allowed_workorder_kinds": allowed,
                    "sandbox": row.get::<_, String>(7)?,
                    "approval_policy": row.get::<_, String>(8)?,
                    "autonomy_profile": row.get::<_, String>(9)?,
                    "max_concurrency": row.get::<_, i64>(10)?,
                    "custom_instructions": row.get::<_, String>(11)?,
                    "config": config,
                    "created_at": row.get::<_, String>(13)?,
                    "updated_at": row.get::<_, String>(14)?,
                }))
            },
        )
        .optional()?;
    let Some(mut payload) = row else {
        bail!("worker is not registered: {worker_id}");
    };
    payload
        .as_object_mut()
        .context("worker payload is not an object")?
        .insert("status".to_owned(), json!("ok"));
    Ok(payload)
}

pub fn create_workorder(
    conn: &Connection,
    target_id: &str,
    workorder_id: &str,
    kind: &str,
    title: &str,
    payload: Value,
) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let workorder_id = required(workorder_id, "workorder id")?;
    let kind = required(kind, "workorder kind")?;
    let title = required(title, "workorder title")?;
    if !payload.is_object() {
        bail!("workorder payload must be a JSON object");
    }
    let now = now_iso();
    conn.execute(
        "INSERT INTO workorders(id, target_id, kind, status, title, payload_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?7)",
        params![
            workorder_id,
            target_id,
            kind,
            title,
            serde_json::to_string(&payload)?,
            now,
            now,
        ],
    )?;
    write_event(
        conn,
        "workorder_created",
        Some(target_id),
        Some(&workorder_id),
        &json!({"kind": kind, "title": title}),
    )?;
    Ok(json!({
        "status": "created",
        "target_id": target_id,
        "workorder_id": workorder_id,
        "workorder_status": "queued",
        "kind": kind,
        "title": title,
    }))
}

pub fn list_workorders(conn: &Connection, target_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let mut statement = conn.prepare(
        "SELECT id, target_id, kind, status, title, payload_json, created_at, updated_at
         FROM workorders WHERE target_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = statement.query_map(params![target_id], |row| {
        let payload_text: String = row.get(5)?;
        let payload: Value = serde_json::from_str(&payload_text).unwrap_or_else(|_| json!({}));
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "target_id": row.get::<_, String>(1)?,
            "kind": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "title": row.get::<_, String>(4)?,
            "payload": payload,
            "created_at": row.get::<_, String>(6)?,
            "updated_at": row.get::<_, String>(7)?,
        }))
    })?;
    let mut workorders = Vec::new();
    for row in rows {
        workorders.push(row?);
    }
    Ok(json!({"status": "ok", "target_id": target_id, "workorders": workorders}))
}

pub fn get_workorder(conn: &Connection, target_id: &str, workorder_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let row: Option<Value> = conn
        .query_row(
            "SELECT id, target_id, kind, status, title, payload_json, created_at, updated_at
             FROM workorders WHERE target_id = ?1 AND id = ?2",
            params![target_id, workorder_id],
            |row| {
                let payload_text: String = row.get(5)?;
                let payload: Value =
                    serde_json::from_str(&payload_text).unwrap_or_else(|_| json!({}));
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "target_id": row.get::<_, String>(1)?,
                    "kind": row.get::<_, String>(2)?,
                    "workorder_status": row.get::<_, String>(3)?,
                    "title": row.get::<_, String>(4)?,
                    "payload": payload,
                    "created_at": row.get::<_, String>(6)?,
                    "updated_at": row.get::<_, String>(7)?,
                }))
            },
        )
        .optional()?;
    let Some(mut payload) = row else {
        bail!("workorder is not registered: {workorder_id}");
    };
    payload
        .as_object_mut()
        .context("workorder payload is not an object")?
        .insert("status".to_owned(), json!("ok"));
    Ok(payload)
}

pub fn run_loop_once(conn: &Connection, target_id: &str, dry_run: bool) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let queued: Option<(String, String, String)> = conn
        .query_row(
            "SELECT id, kind, title FROM workorders
             WHERE target_id = ?1 AND status = 'queued'
             ORDER BY created_at, id LIMIT 1",
            params![target_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((workorder_id, kind, title)) = queued {
        if dry_run {
            return Ok(json!({
                "status": "planned",
                "target_id": target_id,
                "action": "mark_ready_for_worker",
                "workorder_id": workorder_id,
                "kind": kind,
                "title": title,
            }));
        }
        let now = now_iso();
        conn.execute(
            "UPDATE workorders SET status = 'ready_for_worker', updated_at = ?1
             WHERE target_id = ?2 AND id = ?3",
            params![now, target_id, workorder_id],
        )?;
        conn.execute(
            "INSERT INTO loop_runs(target_id, status, summary, payload_json, created_at)
             VALUES (?1, 'ready_for_worker', ?2, ?3, ?4)",
            params![
                target_id,
                format!("Marked workorder {workorder_id} ready for worker"),
                serde_json::to_string(&json!({"workorder_id": workorder_id, "kind": kind}))?,
                now,
            ],
        )?;
        write_event(
            conn,
            "loop_ready_for_worker",
            Some(target_id),
            Some(&workorder_id),
            &json!({"kind": kind}),
        )?;
        return Ok(json!({
            "status": "ready_for_worker",
            "target_id": target_id,
            "workorder_id": workorder_id,
            "kind": kind,
            "title": title,
        }));
    }
    let active: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT id, kind, title, status FROM workorders
             WHERE target_id = ?1
               AND status IN ('ready_for_worker', 'handoff_ready', 'needs_user')
             ORDER BY created_at, id LIMIT 1",
            params![target_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    if let Some((workorder_id, kind, title, status)) = active {
        return Ok(json!({
            "status": "active_workorder_exists",
            "target_id": target_id,
            "workorder_id": workorder_id,
            "workorder_status": status,
            "kind": kind,
            "title": title,
        }));
    }
    let workorder_id = format!("repo-maintainer-{}", Utc::now().timestamp_millis());
    if dry_run {
        return Ok(json!({
            "status": "planned",
            "target_id": target_id,
            "action": "create_repo_discovery_workorder",
            "workorder_id": workorder_id,
        }));
    }
    let pack = generate_target_pack(conn, target_id)?;
    let created = create_workorder(
        conn,
        target_id,
        &workorder_id,
        "repo_discovery",
        "Inspect target repository",
        json!({
            "created_by": "loop_run",
            "mode": "observe",
            "target_pack_path": pack.get("pack_path").and_then(Value::as_str),
        }),
    )?;
    conn.execute(
        "INSERT INTO loop_runs(target_id, status, summary, payload_json, created_at)
         VALUES (?1, 'created_workorder', ?2, ?3, ?4)",
        params![
            target_id,
            format!("Created discovery workorder {workorder_id}"),
            serde_json::to_string(&json!({"workorder_id": workorder_id}))?,
            now_iso(),
        ],
    )?;
    Ok(json!({
        "status": "created_workorder",
        "target_id": target_id,
        "workorder": created,
    }))
}

pub fn run_heartbeat(
    conn: &mut Connection,
    target_id: &str,
    dry_run: bool,
    max_dispatches: usize,
) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let refresh = if dry_run {
        json!({"status": "planned", "target_id": target_id, "action": "refresh_runner_runs"})
    } else {
        refresh_runner_runs(conn, target_id)?
    };
    let pack = if dry_run {
        json!({"status": "planned", "target_id": target_id, "action": "generate_target_pack"})
    } else {
        generate_target_pack(conn, target_id)?
    };
    let mut actions = Vec::new();
    let first_loop = run_loop_once(conn, target_id, dry_run)?;
    actions.push(json!({"phase": "loop", "result": first_loop}));
    if !dry_run
        && actions
            .last()
            .and_then(|action| action.pointer("/result/status"))
            .and_then(Value::as_str)
            == Some("created_workorder")
    {
        let second_loop = run_loop_once(conn, target_id, false)?;
        actions.push(json!({"phase": "loop", "result": second_loop}));
    }
    let mut dispatched = Vec::new();
    for _ in 0..max_dispatches {
        let Some(workorder) = first_ready_workorder(conn, target_id)? else {
            break;
        };
        let workorder_id = workorder
            .get("id")
            .and_then(Value::as_str)
            .context("ready workorder id is missing")?;
        let kind = workorder
            .get("kind")
            .and_then(Value::as_str)
            .context("ready workorder kind is missing")?;
        let worker = select_worker_for_kind(conn, target_id, kind)?;
        let worker_id = worker
            .get("id")
            .and_then(Value::as_str)
            .context("selected worker id is missing")?;
        if dry_run {
            dispatched.push(json!({
                "status": "planned",
                "workorder_id": workorder_id,
                "kind": kind,
                "worker_id": worker_id,
                "handoff": "planned",
            }));
            break;
        }
        let package = dispatch_runner_plan(conn, target_id, workorder_id, Some(worker_id))?;
        dispatched.push(package);
    }
    let last_loop_result = actions
        .last()
        .and_then(|action| action.get("result"))
        .cloned()
        .unwrap_or(Value::Null);
    let dispatch_summary = if dispatched.is_empty() {
        let loop_status = last_loop_result
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let reason = match loop_status {
            "active_workorder_exists" => "active_workorder_exists",
            "planned" => "dry_run_no_ready_workorders",
            _ => "no_ready_workorders",
        };
        json!({
            "status": "idle",
            "reason": reason,
            "dispatched_count": 0,
            "loop_status": loop_status,
            "active_workorder": last_loop_result,
        })
    } else {
        json!({
            "status": "dispatched",
            "dispatched_count": dispatched.len(),
        })
    };
    Ok(json!({
        "status": "ok",
        "target_id": target_id,
        "dry_run": dry_run,
        "execution": "codex_app_handoff",
        "runner_execution": "package_only",
        "refresh": refresh,
        "target_pack": pack,
        "actions": actions,
        "dispatch_summary": dispatch_summary,
        "dispatched": dispatched,
    }))
}

pub fn dispatch_runner_plan(
    conn: &Connection,
    target_id: &str,
    workorder_id: &str,
    worker_id: Option<&str>,
) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    ensure_workorder_exists(conn, target_id, workorder_id)?;
    let worker = worker_id
        .map(|id| ensure_worker_matches_workorder(conn, target_id, workorder_id, id))
        .transpose()?;
    let now = now_iso();
    let placeholder = json!({
        "runner": "codex-app",
        "mode": "handoff_pending",
        "target_id": target_id,
        "workorder_id": workorder_id,
        "worker": worker.clone(),
    });
    conn.execute(
        "INSERT INTO runner_runs(target_id, workorder_id, status, command_json, created_at, updated_at)
         VALUES (?1, ?2, 'handoff_pending', ?3, ?4, ?5)",
        params![
            target_id,
            workorder_id,
            serde_json::to_string(&placeholder)?,
            now,
            now
        ],
    )?;
    let runner_id = conn.last_insert_rowid();
    let target = target_payload(conn, target_id)?;
    let workorder = get_workorder(conn, target_id, workorder_id)?;
    let custom_instructions = prompt_custom_instructions(conn, target_id, &worker)?;
    let dirs = ensure_app_dirs()?;
    let target_segment = path_segment(target_id, "target id")?;
    let package_dir = dirs
        .artifacts
        .join("runners")
        .join(target_segment)
        .join(runner_id.to_string());
    fs::create_dir_all(&package_dir)
        .with_context(|| format!("failed to create {}", display_path(&package_dir)))?;
    let prompt_path = package_dir.join("prompt.md");
    let handoff_path = package_dir.join("handoff.md");
    let command_path = package_dir.join("command.json");
    let result_path = package_dir.join("result.json");
    let binary_path = automation_binary_path();
    let result_command = result_submit_prefix(target_id, workorder_id);
    let refresh_command = format!("{binary_path} runner refresh {target_id} --json");
    let prompt = render_runner_prompt(
        target_id,
        workorder_id,
        &target,
        &workorder,
        &worker,
        &custom_instructions,
    )?;
    fs::write(&prompt_path, prompt)
        .with_context(|| format!("failed to write {}", display_path(&prompt_path)))?;
    let handoff = format!(
        r#"# Codex Automation Handoff

Open the target repository in Codex App and give the worker the contents of:

`{}`

When the worker finishes, record the result with:

```bash
{result_command} --status <status> --summary "..." --next-action <action> --json
```

If the worker returns a final JSON object instead, save that object to:

`{}`

Then run:

```bash
{refresh_command}
```
"#,
        display_path(&prompt_path),
        display_path(&result_path),
    );
    fs::write(&handoff_path, handoff)
        .with_context(|| format!("failed to write {}", display_path(&handoff_path)))?;
    let command = json!({
        "runner": "codex-app",
        "mode": "handoff",
        "execution": "package_only",
        "binary_path": binary_path,
        "target_id": target_id,
        "workorder_id": workorder_id,
        "worker": worker,
        "target": target,
        "workorder": workorder,
        "handoff": {
            "status": "ready",
            "medium": "codex_app",
            "instructions": "Open prompt_path in Codex App or paste its contents into a worker thread. codex-automation does not launch Codex processes.",
            "result_path": display_path(&result_path)
        },
        "package": {
            "root": display_path(&package_dir),
            "prompt_path": display_path(&prompt_path),
            "handoff_path": display_path(&handoff_path),
            "command_path": display_path(&command_path),
            "result_path": display_path(&result_path)
        },
        "result_contract": {
            "command": result_command,
            "refresh_command": refresh_command,
            "target_id": target_id,
            "workorder_id": workorder_id,
            "final_json_fields": ["workorder_id", "status", "summary", "next_action"]
        }
    });
    fs::write(&command_path, serde_json::to_string_pretty(&command)?)
        .with_context(|| format!("failed to write {}", display_path(&command_path)))?;
    let updated_at = now_iso();
    conn.execute(
        "UPDATE runner_runs SET status = 'handoff_ready', command_json = ?1, updated_at = ?2
         WHERE target_id = ?3 AND id = ?4",
        params![
            serde_json::to_string(&command)?,
            updated_at,
            target_id,
            runner_id
        ],
    )?;
    conn.execute(
        "UPDATE workorders SET status = 'handoff_ready', updated_at = ?1
         WHERE target_id = ?2 AND id = ?3",
        params![updated_at, target_id, workorder_id],
    )?;
    write_event(
        conn,
        "runner_handoff_ready",
        Some(target_id),
        Some(workorder_id),
        &json!({
            "runner_id": runner_id,
            "prompt_path": display_path(&prompt_path),
            "handoff_path": display_path(&handoff_path),
            "result_path": display_path(&result_path),
            "command_path": display_path(&command_path)
        }),
    )?;
    Ok(json!({
        "status": "handoff_ready",
        "target_id": target_id,
        "workorder_id": workorder_id,
        "runner_id": runner_id,
        "command": command,
    }))
}

pub fn render_prompt_for_workorder(
    conn: &Connection,
    target_id: &str,
    workorder_id: &str,
    worker_id: Option<&str>,
) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    ensure_workorder_exists(conn, target_id, workorder_id)?;
    let worker = worker_id
        .map(|id| ensure_worker_matches_workorder(conn, target_id, workorder_id, id))
        .transpose()?;
    let target = target_payload(conn, target_id)?;
    let workorder = get_workorder(conn, target_id, workorder_id)?;
    let custom_instructions = prompt_custom_instructions(conn, target_id, &worker)?;
    let prompt = render_runner_prompt(
        target_id,
        workorder_id,
        &target,
        &workorder,
        &worker,
        &custom_instructions,
    )?;
    Ok(json!({
        "status": "rendered",
        "target_id": target_id,
        "workorder_id": workorder_id,
        "worker_id": worker.as_ref().and_then(|value| value.get("id")).and_then(Value::as_str),
        "custom_instructions": {
            "control_plane": custom_instructions.control_plane,
            "target": custom_instructions.target,
            "worker": custom_instructions.worker,
        },
        "prompt": prompt,
    }))
}

fn refresh_one_runner(conn: &mut Connection, runner: &Value) -> Result<Value> {
    let target_id = runner
        .get("target_id")
        .and_then(Value::as_str)
        .context("runner target_id is missing")?;
    let runner_id = runner
        .get("id")
        .and_then(Value::as_i64)
        .context("runner id is missing")?;
    let workorder_id = runner
        .get("workorder_id")
        .and_then(Value::as_str)
        .context("runner workorder_id is missing")?;
    let runner_status = runner
        .get("runner_status")
        .and_then(Value::as_str)
        .context("runner_status is missing")?;
    let mut command = runner
        .get("command")
        .cloned()
        .context("runner command is missing")?;
    if result_count_for_workorder(conn, target_id, workorder_id)? > 0 {
        if runner_status != "completed_from_result" {
            command["handoff"]["status"] = json!("completed_from_result");
            command["handoff"]["completed_at"] = json!(now_iso());
            set_runner_state(
                conn,
                target_id,
                runner_id,
                "completed_from_result",
                &command,
            )?;
        }
        return Ok(json!({"runner_id": runner_id, "status": "completed_from_result"}));
    }
    if let Some(result_path) = command
        .pointer("/package/result_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
    {
        if result_path.is_file() {
            let text = fs::read_to_string(&result_path)
                .with_context(|| format!("failed to read {}", display_path(&result_path)))?;
            if let Ok(result_payload) = serde_json::from_str::<Value>(&text) {
                if result_payload.get("workorder_id").and_then(Value::as_str) == Some(workorder_id)
                {
                    let recorded = submit_result(conn, target_id, result_payload)?;
                    command["handoff"]["status"] = json!("completed_from_result");
                    command["handoff"]["completed_at"] = json!(now_iso());
                    command["handoff"]["ingested_result"] = recorded;
                    set_runner_state(
                        conn,
                        target_id,
                        runner_id,
                        "completed_from_result",
                        &command,
                    )?;
                    return Ok(json!({"runner_id": runner_id, "status": "completed_from_result"}));
                }
            }
        }
    }
    Ok(json!({"runner_id": runner_id, "status": runner_status}))
}

pub fn refresh_runner_runs(conn: &mut Connection, target_id: &str) -> Result<Value> {
    let listed = list_runner_runs(conn, target_id)?;
    let runners = listed
        .get("runner_runs")
        .and_then(Value::as_array)
        .context("runner_runs is missing")?
        .clone();
    let mut refreshed = Vec::new();
    for runner in runners {
        refreshed.push(refresh_one_runner(conn, &runner)?);
    }
    Ok(json!({
        "status": "ok",
        "target_id": target_id,
        "runner_refresh": {
            "checked": refreshed.len(),
            "runners": refreshed
        }
    }))
}

pub fn list_runner_runs(conn: &Connection, target_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let mut statement = conn.prepare(
        "SELECT id, target_id, workorder_id, status, command_json, created_at, updated_at
         FROM runner_runs WHERE target_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map(params![target_id], |row| {
        let command_text: String = row.get(4)?;
        let command: Value = serde_json::from_str(&command_text).unwrap_or_else(|_| json!({}));
        let worker_id = command
            .pointer("/worker/id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let worker_name = command
            .pointer("/worker/name")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "target_id": row.get::<_, String>(1)?,
            "workorder_id": row.get::<_, String>(2)?,
            "worker_id": worker_id,
            "worker_name": worker_name,
            "runner_status": row.get::<_, String>(3)?,
            "command": command,
            "created_at": row.get::<_, String>(5)?,
            "updated_at": row.get::<_, String>(6)?,
        }))
    })?;
    let mut runner_runs = Vec::new();
    for row in rows {
        runner_runs.push(row?);
    }
    Ok(json!({"status": "ok", "target_id": target_id, "runner_runs": runner_runs}))
}

pub fn get_runner_run(conn: &Connection, target_id: &str, runner_id: i64) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let row: Option<Value> = conn
        .query_row(
            "SELECT id, target_id, workorder_id, status, command_json, created_at, updated_at
             FROM runner_runs WHERE target_id = ?1 AND id = ?2",
            params![target_id, runner_id],
            |row| {
                let command_text: String = row.get(4)?;
                let command: Value =
                    serde_json::from_str(&command_text).unwrap_or_else(|_| json!({}));
                let worker_id = command
                    .pointer("/worker/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let worker_name = command
                    .pointer("/worker/name")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "target_id": row.get::<_, String>(1)?,
                    "workorder_id": row.get::<_, String>(2)?,
                    "worker_id": worker_id,
                    "worker_name": worker_name,
                    "runner_status": row.get::<_, String>(3)?,
                    "command": command,
                    "created_at": row.get::<_, String>(5)?,
                    "updated_at": row.get::<_, String>(6)?,
                }))
            },
        )
        .optional()?;
    row.with_context(|| format!("runner is not registered: {runner_id}"))
}

pub fn request_approval(
    conn: &Connection,
    target_id: &str,
    workorder_id: &str,
    approval_id: Option<&str>,
    reason: &str,
) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    ensure_workorder_exists(conn, target_id, workorder_id)?;
    let reason = required(reason, "approval reason")?;
    let approval_id = approval_id
        .map(|value| required(value, "approval id"))
        .transpose()?
        .unwrap_or_else(|| format!("approval-{workorder_id}"));
    let now = now_iso();
    conn.execute(
        "INSERT INTO approvals(id, target_id, workorder_id, status, reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6)",
        params![approval_id, target_id, workorder_id, reason, now, now],
    )?;
    conn.execute(
        "UPDATE workorders SET status = 'needs_user', updated_at = ?1
         WHERE target_id = ?2 AND id = ?3",
        params![now, target_id, workorder_id],
    )?;
    write_event(
        conn,
        "approval_requested",
        Some(target_id),
        Some(workorder_id),
        &json!({"approval_id": approval_id, "reason": reason}),
    )?;
    Ok(json!({
        "status": "pending",
        "target_id": target_id,
        "workorder_id": workorder_id,
        "approval_id": approval_id,
        "reason": reason,
    }))
}

pub fn list_approvals(conn: &Connection, target_id: &str) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let mut statement = conn.prepare(
        "SELECT id, target_id, workorder_id, status, reason, decision, decided_by, message, created_at, updated_at
         FROM approvals WHERE target_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = statement.query_map(params![target_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "target_id": row.get::<_, String>(1)?,
            "workorder_id": row.get::<_, String>(2)?,
            "status": row.get::<_, String>(3)?,
            "reason": row.get::<_, String>(4)?,
            "decision": row.get::<_, Option<String>>(5)?,
            "decided_by": row.get::<_, Option<String>>(6)?,
            "message": row.get::<_, Option<String>>(7)?,
            "created_at": row.get::<_, String>(8)?,
            "updated_at": row.get::<_, String>(9)?,
        }))
    })?;
    let mut approvals = Vec::new();
    for row in rows {
        approvals.push(row?);
    }
    Ok(json!({"status": "ok", "target_id": target_id, "approvals": approvals}))
}

pub fn record_approval(
    conn: &Connection,
    target_id: &str,
    approval_id: &str,
    decision: &str,
    decided_by: &str,
    message: &str,
) -> Result<Value> {
    ensure_target_exists(conn, target_id)?;
    let decision = required(decision, "approval decision")?;
    if decision != "approved" && decision != "rejected" {
        bail!("approval decision must be approved or rejected");
    }
    let decided_by = required(decided_by, "decided_by")?;
    let message = required(message, "approval message")?;
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM approvals WHERE target_id = ?1 AND id = ?2",
            params![target_id, approval_id],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_none() {
        bail!("approval is not registered: {approval_id}");
    }
    let now = now_iso();
    conn.execute(
        "UPDATE approvals
         SET status = 'decided', decision = ?1, decided_by = ?2, message = ?3, updated_at = ?4
         WHERE target_id = ?5 AND id = ?6",
        params![decision, decided_by, message, now, target_id, approval_id],
    )?;
    write_event(
        conn,
        "approval_decided",
        Some(target_id),
        None,
        &json!({"approval_id": approval_id, "decision": decision, "decided_by": decided_by}),
    )?;
    Ok(json!({
        "status": "recorded",
        "target_id": target_id,
        "approval_id": approval_id,
        "decision": decision,
        "decided_by": decided_by,
    }))
}
