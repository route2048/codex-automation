use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use codex_automation_core::app_dirs::{app_dirs, display_path, ensure_app_dirs, paths_summary};
use codex_automation_core::control;
use codex_automation_core::storage;
use codex_automation_core::workspace;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

const DEFAULT_RUNNER_WORKERS: &[&str] = &[
    "repo-maintainer.toml",
    "ops-analyst.toml",
    "release-manager.toml",
];
const SETUP_SKILL_NAME: &str = "codex-automation-setup";

#[derive(Debug, Parser)]
#[command(name = "codex-automation")]
#[command(about = "Local-first Codex automation control app")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init(InitArgs),
    Uninstall(UninstallArgs),
    Update(UpdateArgs),
    Doctor,
    Paths(PathsArgs),
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Target {
        #[command(subcommand)]
        command: TargetCommand,
    },
    Heartbeat {
        #[command(subcommand)]
        command: HeartbeatCommand,
    },
    Result {
        #[command(subcommand)]
        command: ResultCommand,
    },
    Workorder {
        #[command(subcommand)]
        command: WorkorderCommand,
    },
    Worker {
        #[command(subcommand)]
        command: WorkerCommand,
    },
    Prompt {
        #[command(subcommand)]
        command: PromptCommand,
    },
    Loop {
        #[command(subcommand)]
        command: LoopCommand,
    },
    Runner {
        #[command(subcommand)]
        command: RunnerCommand,
    },
    Approval {
        #[command(subcommand)]
        command: ApprovalCommand,
    },
}

#[derive(Debug, Args)]
struct InitArgs {
    target: String,
    #[arg(long, default_value = "codex-automation")]
    workspace: PathBuf,
    #[arg(long, default_value = "targets")]
    clone_dir: PathBuf,
    #[arg(long)]
    target_id: Option<String>,
    #[arg(long, default_value = "balanced")]
    profile: String,
    #[arg(long)]
    overwrite_workspace: bool,
}

#[derive(Debug, Args)]
struct UninstallArgs {
    #[arg(long)]
    remove_app_state: bool,
    #[arg(long)]
    remove_skills: bool,
    #[arg(long)]
    remove_control_workspace: bool,
    #[arg(long)]
    workspace: Option<PathBuf>,
    #[arg(long)]
    codex_home: Option<PathBuf>,
    #[arg(long)]
    yes: bool,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    #[arg(long)]
    workspace: Option<PathBuf>,
    #[arg(long)]
    target_id: Option<String>,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct PathsArgs {
    #[arg(long)]
    workspace: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum DbCommand {
    Doctor,
    Migrate,
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    Init(WorkspaceInitArgs),
    Status(WorkspaceStatusArgs),
}

#[derive(Debug, Args)]
struct WorkspaceInitArgs {
    path: PathBuf,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    overwrite: bool,
}

#[derive(Debug, Args)]
struct WorkspaceStatusArgs {
    path: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum TargetCommand {
    Add(TargetAddArgs),
    List(TargetListArgs),
    Pack(TargetPackArgs),
    Status(TargetStatusArgs),
}

#[derive(Debug, Args)]
struct TargetAddArgs {
    id: String,
    #[arg(long)]
    repo: PathBuf,
    #[arg(long)]
    workspace: Option<PathBuf>,
    #[arg(long, default_value = "balanced")]
    profile: String,
}

#[derive(Debug, Args)]
struct TargetListArgs {
    #[arg(long)]
    workspace: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TargetPackArgs {
    id: String,
}

#[derive(Debug, Args)]
struct TargetStatusArgs {
    id: String,
}

#[derive(Debug, Subcommand)]
enum HeartbeatCommand {
    Run(HeartbeatRunArgs),
}

#[derive(Debug, Args)]
struct HeartbeatRunArgs {
    target_id: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    execute: bool,
    #[arg(long, default_value_t = 1)]
    max_dispatches: usize,
}

#[derive(Debug, Subcommand)]
enum ResultCommand {
    Submit(ResultSubmitArgs),
    List(ResultListArgs),
}

#[derive(Debug, Args)]
struct ResultSubmitArgs {
    target_id: String,
    #[arg(long)]
    from_file: Option<PathBuf>,
    #[arg(long)]
    workorder_id: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    summary: Option<String>,
    #[arg(long)]
    next_action: Option<String>,
}

#[derive(Debug, Args)]
struct ResultListArgs {
    target_id: String,
}

#[derive(Debug, Subcommand)]
enum WorkorderCommand {
    Create(WorkorderCreateArgs),
    List(WorkorderListArgs),
    Status(WorkorderStatusArgs),
}

#[derive(Debug, Args)]
struct WorkorderCreateArgs {
    target_id: String,
    #[arg(long)]
    id: String,
    #[arg(long)]
    kind: String,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "{}")]
    payload_json: String,
}

#[derive(Debug, Args)]
struct WorkorderListArgs {
    target_id: String,
}

#[derive(Debug, Args)]
struct WorkorderStatusArgs {
    target_id: String,
    workorder_id: String,
}

#[derive(Debug, Subcommand)]
enum WorkerCommand {
    Add(WorkerAddArgs),
    List(WorkerListArgs),
    Status(WorkerStatusArgs),
}

#[derive(Debug, Args)]
struct WorkerAddArgs {
    target_id: String,
    #[arg(long)]
    from_file: PathBuf,
}

#[derive(Debug, Args)]
struct WorkerListArgs {
    target_id: String,
}

#[derive(Debug, Args)]
struct WorkerStatusArgs {
    target_id: String,
    worker_id: String,
}

#[derive(Debug, Subcommand)]
enum PromptCommand {
    Render(PromptRenderArgs),
}

#[derive(Debug, Args)]
struct PromptRenderArgs {
    target_id: String,
    #[arg(long)]
    workorder_id: String,
    #[arg(long)]
    worker: Option<String>,
}

#[derive(Debug, Subcommand)]
enum LoopCommand {
    Run(LoopRunArgs),
}

#[derive(Debug, Args)]
struct LoopRunArgs {
    target_id: String,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Subcommand)]
enum RunnerCommand {
    Dispatch(RunnerDispatchArgs),
    List(RunnerListArgs),
    Status(RunnerStatusArgs),
    Refresh(RunnerRefreshArgs),
}

#[derive(Debug, Args)]
struct RunnerDispatchArgs {
    target_id: String,
    #[arg(long)]
    workorder_id: String,
    #[arg(long)]
    worker: Option<String>,
    #[arg(long)]
    execute: bool,
}

#[derive(Debug, Args)]
struct RunnerListArgs {
    target_id: String,
}

#[derive(Debug, Args)]
struct RunnerStatusArgs {
    target_id: String,
    runner_id: i64,
}

#[derive(Debug, Args)]
struct RunnerRefreshArgs {
    target_id: String,
}

#[derive(Debug, Subcommand)]
enum ApprovalCommand {
    Request(ApprovalRequestArgs),
    List(ApprovalListArgs),
    Record(ApprovalRecordArgs),
}

#[derive(Debug, Args)]
struct ApprovalRequestArgs {
    target_id: String,
    #[arg(long)]
    workorder_id: String,
    #[arg(long)]
    approval_id: Option<String>,
    #[arg(long)]
    reason: String,
}

#[derive(Debug, Args)]
struct ApprovalListArgs {
    target_id: String,
}

#[derive(Debug, Args)]
struct ApprovalRecordArgs {
    target_id: String,
    approval_id: String,
    #[arg(long)]
    decision: String,
    #[arg(long, default_value = "operator")]
    decided_by: String,
    #[arg(long)]
    message: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let payload = run(cli.command)?;
    print_payload(&payload, cli.json)
}

fn run(command: Command) -> Result<Value> {
    match command {
        Command::Init(args) => run_init(args),
        Command::Uninstall(args) => run_uninstall(args),
        Command::Update(args) => run_update(args),
        Command::Doctor => {
            let dirs = ensure_app_dirs()?;
            Ok(json!({"status": "ok", "app_state": dirs.as_json()}))
        }
        Command::Paths(args) => {
            let workspace = args.workspace.as_deref();
            paths_summary(workspace)
        }
        Command::Db { command } => match command {
            DbCommand::Doctor => storage::db_doctor(),
            DbCommand::Migrate => storage::db_migrate(),
        },
        Command::Workspace { command } => match command {
            WorkspaceCommand::Init(args) => {
                workspace::initialize_workspace(&args.path, args.name.as_deref(), args.overwrite)
            }
            WorkspaceCommand::Status(args) => {
                let path = args.path.unwrap_or(std::env::current_dir()?);
                workspace::workspace_status(&path)
            }
        },
        Command::Target { command } => match command {
            TargetCommand::Add(args) => {
                let workspace_path = args.workspace.unwrap_or(std::env::current_dir()?);
                workspace::add_target(&workspace_path, &args.id, &args.repo, &args.profile)
            }
            TargetCommand::List(args) => {
                let workspace_path = args.workspace.as_deref();
                workspace::list_targets(workspace_path)
            }
            TargetCommand::Pack(args) => {
                let conn = storage::connect()?;
                control::generate_target_pack(&conn, &args.id)
            }
            TargetCommand::Status(args) => workspace::target_status(&args.id),
        },
        Command::Heartbeat { command } => match command {
            HeartbeatCommand::Run(args) => {
                if args.execute {
                    ensure_runner_execution_enabled()?;
                }
                let mut conn = storage::connect()?;
                control::run_heartbeat(
                    &mut conn,
                    &args.target_id,
                    args.dry_run,
                    args.execute,
                    args.max_dispatches,
                )
            }
        },
        Command::Result { command } => match command {
            ResultCommand::Submit(args) => {
                let payload = result_payload(args)?;
                let mut conn = storage::connect()?;
                let target_id = payload
                    .get("_target_id")
                    .and_then(Value::as_str)
                    .context("internal target id is missing")?
                    .to_owned();
                let mut object = payload
                    .as_object()
                    .context("result payload is not an object")?
                    .clone();
                object.remove("_target_id");
                storage::submit_result(&mut conn, &target_id, Value::Object(object))
            }
            ResultCommand::List(args) => {
                let conn = storage::connect()?;
                storage::list_results(&conn, &args.target_id)
            }
        },
        Command::Workorder { command } => match command {
            WorkorderCommand::Create(args) => {
                let conn = storage::connect()?;
                let payload: Value = serde_json::from_str(&args.payload_json)
                    .context("--payload-json must be a JSON object")?;
                control::create_workorder(
                    &conn,
                    &args.target_id,
                    &args.id,
                    &args.kind,
                    &args.title,
                    payload,
                )
            }
            WorkorderCommand::List(args) => {
                let conn = storage::connect()?;
                control::list_workorders(&conn, &args.target_id)
            }
            WorkorderCommand::Status(args) => {
                let conn = storage::connect()?;
                control::get_workorder(&conn, &args.target_id, &args.workorder_id)
            }
        },
        Command::Worker { command } => match command {
            WorkerCommand::Add(args) => {
                let conn = storage::connect()?;
                let payload = worker_payload_from_file(&args.from_file)?;
                control::add_worker(&conn, &args.target_id, payload)
            }
            WorkerCommand::List(args) => {
                let conn = storage::connect()?;
                control::list_workers(&conn, &args.target_id)
            }
            WorkerCommand::Status(args) => {
                let conn = storage::connect()?;
                control::get_worker(&conn, &args.target_id, &args.worker_id)
            }
        },
        Command::Prompt { command } => match command {
            PromptCommand::Render(args) => {
                let conn = storage::connect()?;
                control::render_prompt_for_workorder(
                    &conn,
                    &args.target_id,
                    &args.workorder_id,
                    args.worker.as_deref(),
                )
            }
        },
        Command::Loop { command } => match command {
            LoopCommand::Run(args) => {
                let conn = storage::connect()?;
                control::run_loop_once(&conn, &args.target_id, args.dry_run)
            }
        },
        Command::Runner { command } => match command {
            RunnerCommand::Dispatch(args) => {
                if args.execute {
                    ensure_runner_execution_enabled()?;
                }
                let conn = storage::connect()?;
                let package = control::dispatch_runner_plan(
                    &conn,
                    &args.target_id,
                    &args.workorder_id,
                    args.worker.as_deref(),
                )?;
                if args.execute {
                    let runner_id = package
                        .get("runner_id")
                        .and_then(Value::as_i64)
                        .context("runner_id is missing from runner package")?;
                    control::start_runner_package(&conn, &args.target_id, runner_id)
                } else {
                    Ok(package)
                }
            }
            RunnerCommand::List(args) => {
                let conn = storage::connect()?;
                control::list_runner_runs(&conn, &args.target_id)
            }
            RunnerCommand::Status(args) => {
                let conn = storage::connect()?;
                control::get_runner_run(&conn, &args.target_id, args.runner_id)
            }
            RunnerCommand::Refresh(args) => {
                let mut conn = storage::connect()?;
                control::refresh_runner_runs(&mut conn, &args.target_id)
            }
        },
        Command::Approval { command } => match command {
            ApprovalCommand::Request(args) => {
                let conn = storage::connect()?;
                control::request_approval(
                    &conn,
                    &args.target_id,
                    &args.workorder_id,
                    args.approval_id.as_deref(),
                    &args.reason,
                )
            }
            ApprovalCommand::List(args) => {
                let conn = storage::connect()?;
                control::list_approvals(&conn, &args.target_id)
            }
            ApprovalCommand::Record(args) => {
                let conn = storage::connect()?;
                control::record_approval(
                    &conn,
                    &args.target_id,
                    &args.approval_id,
                    &args.decision,
                    &args.decided_by,
                    &args.message,
                )
            }
        },
    }
}

fn run_update(args: UpdateArgs) -> Result<Value> {
    let workspace_path = args.workspace.as_deref();
    let paths = paths_summary(workspace_path)?;
    let database = if args.check {
        storage::db_doctor()?
    } else {
        storage::db_migrate()?
    };
    let database_check = storage::db_doctor()?;
    let workspace_status = if let Some(path) = workspace_path {
        Some(workspace::workspace_status(path)?)
    } else {
        None
    };
    let targets = workspace::list_targets(workspace_path)?;
    let target_status = if let Some(target_id) = args.target_id.as_deref() {
        Some(workspace::target_status(target_id)?)
    } else {
        None
    };
    let target_pack = if !args.check {
        if let Some(target_id) = args.target_id.as_deref() {
            let conn = storage::connect()?;
            Some(control::generate_target_pack(&conn, target_id)?)
        } else {
            None
        }
    } else {
        None
    };
    let heartbeat = if let Some(target_id) = args.target_id.as_deref() {
        let mut conn = storage::connect()?;
        Some(control::run_heartbeat(
            &mut conn, target_id, true, false, 1,
        )?)
    } else {
        None
    };
    let binary = std::env::current_exe()
        .ok()
        .map(|path| display_path(&path))
        .unwrap_or_else(|| "unknown".to_owned());
    Ok(json!({
        "status": if args.check { "checked" } else { "updated" },
        "mode": if args.check { "check" } else { "apply" },
        "version": env!("CARGO_PKG_VERSION"),
        "binary": binary,
        "database": database,
        "database_check": database_check,
        "paths": paths,
        "workspace": workspace_status,
        "targets": targets,
        "target": target_status,
        "target_pack": target_pack,
        "heartbeat": heartbeat,
        "runner_execution": "not_started",
    }))
}

fn run_uninstall(args: UninstallArgs) -> Result<Value> {
    if args.yes && args.dry_run {
        bail!("--yes and --dry-run cannot be used together");
    }
    let requested = args.remove_app_state || args.remove_skills || args.remove_control_workspace;
    if args.yes && !requested {
        bail!("refusing to uninstall without explicit removal flags");
    }
    if args.remove_control_workspace && args.workspace.is_none() {
        bail!("--workspace is required with --remove-control-workspace");
    }
    let dry_run = !args.yes || args.dry_run;
    let include_app_state = args.remove_app_state || !requested;
    let include_skills = args.remove_skills || !requested;
    let include_workspace =
        args.remove_control_workspace || (!requested && args.workspace.is_some());
    let mut actions = Vec::new();
    if include_app_state {
        let dirs = app_dirs()?;
        actions.push(remove_app_state_action(&dirs.state_root, dry_run)?);
    }
    if include_skills {
        actions.push(json!({
            "label": "setup_skill",
            "action": remove_setup_skill_action(args.codex_home.as_deref(), dry_run)?,
        }));
    }
    if include_workspace {
        let workspace = args
            .workspace
            .as_ref()
            .context("internal workspace path is missing")?;
        actions.push(remove_path_action(
            "control_workspace",
            workspace,
            dry_run,
            Some("codex-automation.toml"),
        )?);
    }
    Ok(json!({
        "status": if dry_run { "planned" } else { "ok" },
        "dry_run": dry_run,
        "actions": actions,
        "binary": {
            "removed": false,
            "reason": "binary removal depends on the installer or package manager; use brew uninstall, remove the install.sh destination, or cargo uninstall as appropriate"
        },
        "target_repositories": {
            "removed": false,
            "reason": "target repositories are never removed by codex-automation uninstall"
        }
    }))
}

fn remove_path_action(
    label: &str,
    path: &Path,
    dry_run: bool,
    required_marker: Option<&str>,
) -> Result<Value> {
    let path = expand_path(path)?;
    reject_dangerous_delete_path(&path)?;
    if path.exists() {
        if let Some(marker) = required_marker {
            let marker_path = path.join(marker);
            if !marker_path.is_file() {
                bail!(
                    "refusing to remove {label}; marker is missing: {}",
                    display_path(&marker_path)
                );
            }
        }
    }
    let existed = path.exists();
    let path_kind = path_kind(&path);
    if !dry_run && existed {
        remove_existing_path(&path)?;
    }
    Ok(json!({
        "label": label,
        "path": display_path(&path),
        "kind": path_kind,
        "existed": existed,
        "removed": existed && !dry_run,
        "dry_run": dry_run,
    }))
}

fn remove_app_state_action(path: &Path, dry_run: bool) -> Result<Value> {
    let path = expand_path(path)?;
    reject_dangerous_delete_path(&path)?;
    let existed = path.exists();
    let marker_hits = app_state_marker_hits(&path);
    let basename_matches =
        path.file_name().and_then(|name| name.to_str()) == Some("codex-automation");
    if !dry_run && existed && !basename_matches && marker_hits.is_empty() {
        bail!(
            "refusing to remove app_state; path does not look owned by codex-automation: {}",
            display_path(&path)
        );
    }
    let path_kind = path_kind(&path);
    if !dry_run && existed {
        remove_existing_path(&path)?;
    }
    Ok(json!({
        "label": "app_state",
        "path": display_path(&path),
        "kind": path_kind,
        "existed": existed,
        "removed": existed && !dry_run,
        "dry_run": dry_run,
        "marker_hits": marker_hits,
    }))
}

fn remove_setup_skill_action(codex_home: Option<&Path>, dry_run: bool) -> Result<Value> {
    let codex_home = resolve_codex_home(codex_home)?;
    let destination = codex_home.join("skills").join(SETUP_SKILL_NAME);
    let existed = destination.exists();
    if !dry_run && existed {
        remove_existing_path(&destination)?;
    }
    Ok(json!({
        "status": if dry_run { "planned" } else { "ok" },
        "skill": SETUP_SKILL_NAME,
        "codex_home": display_path(&codex_home),
        "path": display_path(&destination),
        "existed": existed,
        "removed": existed && !dry_run,
        "dry_run": dry_run,
        "restart_required": existed && !dry_run,
    }))
}

fn resolve_codex_home(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return expand_path(path);
    }
    if let Some(raw) = std::env::var_os("CODEX_HOME") {
        if raw.is_empty() {
            bail!("CODEX_HOME is set but empty");
        }
        return Ok(PathBuf::from(raw));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".codex"));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(profile).join(".codex"));
    }
    bail!("CODEX_HOME, HOME, or USERPROFILE is required to resolve Codex skills")
}

fn app_state_marker_hits(path: &Path) -> Vec<String> {
    [
        "codex-automation.sqlite",
        "worktrees",
        "logs",
        "artifacts",
        "backups",
    ]
    .iter()
    .filter(|marker| path.join(marker).exists())
    .map(|marker| (*marker).to_owned())
    .collect()
}

fn path_kind(path: &Path) -> &'static str {
    if path.is_dir() {
        "directory"
    } else if path.is_file() {
        "file"
    } else {
        "missing"
    }
}

fn remove_existing_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", display_path(path)))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove {}", display_path(path)))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove {}", display_path(path)))?;
    }
    Ok(())
}

fn reject_dangerous_delete_path(path: &Path) -> Result<()> {
    if path.parent().is_none() {
        bail!("refusing to remove filesystem root: {}", display_path(path));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if path == home {
            bail!("refusing to remove HOME: {}", display_path(path));
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        if path == profile {
            bail!("refusing to remove USERPROFILE: {}", display_path(path));
        }
    }
    Ok(())
}

fn run_init(args: InitArgs) -> Result<Value> {
    let control_workspace = expand_path(&args.workspace)?;
    let clone_dir = expand_path(&args.clone_dir)?;
    let target_record = resolve_target_arg(&args.target, &clone_dir)?;
    let target_path = PathBuf::from(
        target_record
            .get("path")
            .and_then(Value::as_str)
            .context("resolved target path is missing")?,
    );
    let resolved_target_id = if let Some(target_id) = args.target_id {
        target_id
    } else if is_git_url(&args.target) {
        workspace::slugify_id(&repo_name_from_url(&args.target)?)
    } else {
        let name = target_path
            .file_name()
            .and_then(|value| value.to_str())
            .context("target path must have a directory name")?;
        workspace::slugify_id(name)
    };

    let doctor_dirs = ensure_app_dirs()?;
    let doctor = json!({"status": "ok", "app_state": doctor_dirs.as_json()});
    let db = storage::db_doctor()?;

    let workspace_config = control_workspace.join("codex-automation.toml");
    let (workspace_action, workspace_payload) = if workspace_config.exists()
        && !args.overwrite_workspace
    {
        ("reused", workspace::workspace_status(&control_workspace)?)
    } else {
        (
            "initialized",
            workspace::initialize_workspace(&control_workspace, None, args.overwrite_workspace)?,
        )
    };

    let target_payload = workspace::add_target(
        &control_workspace,
        &resolved_target_id,
        &target_path,
        &args.profile,
    )?;
    let mut conn = storage::connect()?;
    let mut worker_registrations = Vec::new();
    let mut worker_config_paths = Vec::new();
    for worker_file in DEFAULT_RUNNER_WORKERS {
        let worker_path = control_workspace.join("workers").join(worker_file);
        if !worker_path.is_file() {
            bail!(
                "default worker definition is missing: {}",
                worker_path.to_string_lossy()
            );
        }
        let worker_payload = worker_payload_from_file(&worker_path)?;
        worker_config_paths.push(worker_path.to_string_lossy().to_string());
        worker_registrations.push(control::add_worker(
            &conn,
            &resolved_target_id,
            worker_payload,
        )?);
    }
    let target_pack = control::generate_target_pack(&conn, &resolved_target_id)?;
    let heartbeat = control::run_heartbeat(&mut conn, &resolved_target_id, false, false, 1)?;
    let target_status = workspace::target_status(&resolved_target_id)?;
    let paths = paths_summary(Some(&control_workspace))?;
    let targets = workspace::list_targets(Some(&control_workspace))?;
    let app_state = paths.get("app_state").cloned().unwrap_or(Value::Null);
    let target_config_path = target_payload
        .get("target_config_path")
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "status": "ready_for_handoff",
        "target": target_record,
        "target_id": resolved_target_id,
        "setup_skill": {
            "status": "external",
            "skill": SETUP_SKILL_NAME,
            "reason": "setup skill is distributed as a release asset and is not embedded in the binary",
        },
        "doctor": doctor,
        "db": db,
        "workspace_action": workspace_action,
        "workspace": workspace_payload,
        "target_registration": target_payload,
        "worker_registrations": worker_registrations,
        "target_pack": target_pack,
        "heartbeat": heartbeat,
        "target_status": target_status,
        "paths": paths,
        "targets": targets,
        "handoff": {
            "control_workspace": control_workspace.to_string_lossy(),
            "app_state": app_state,
            "target_config_path": target_config_path,
            "worker_config_paths": worker_config_paths,
            "next_prompt": "Open the control workspace in Codex App. Inspect the heartbeat output and runner package before enabling execution.",
        },
    }))
}

fn is_git_url(value: &str) -> bool {
    value.contains("://") || value.starts_with("git@")
}

fn repo_name_from_url(url: &str) -> Result<String> {
    let trimmed = url.trim_end_matches('/');
    let without_git = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let Some(name) = without_git.rsplit(['/', ':']).next() else {
        bail!("cannot derive repository name from URL: {url}");
    };
    if name.is_empty() {
        bail!("cannot derive repository name from URL: {url}");
    }
    Ok(name.to_owned())
}

fn resolve_target_arg(target: &str, clone_dir: &Path) -> Result<Value> {
    if is_git_url(target) {
        return clone_or_pull(target, clone_dir);
    }
    let path = expand_path(Path::new(target))?;
    if !path.is_dir() {
        bail!(
            "target repository does not exist: {}",
            path.to_string_lossy()
        );
    }
    Ok(json!({
        "kind": "local_path",
        "action": "resolved",
        "path": path.to_string_lossy(),
    }))
}

fn clone_or_pull(url: &str, clone_dir: &Path) -> Result<Value> {
    fs::create_dir_all(clone_dir)
        .with_context(|| format!("failed to create {}", clone_dir.to_string_lossy()))?;
    let destination = clone_dir.join(repo_name_from_url(url)?);
    if destination.exists() {
        if !destination.join(".git").is_dir() {
            bail!(
                "checkout path exists but is not a Git repo: {}",
                destination.to_string_lossy()
            );
        }
        let command = run_process("git", &["pull", "--ff-only"], &destination)?;
        return Ok(json!({
            "kind": "git_url",
            "action": "pulled",
            "url": url,
            "path": destination.to_string_lossy(),
            "command": command,
        }));
    }
    let destination_text = destination.to_string_lossy().into_owned();
    let command = run_process("git", &["clone", url, &destination_text], clone_dir)?;
    Ok(json!({
        "kind": "git_url",
        "action": "cloned",
        "url": url,
        "path": destination.to_string_lossy(),
        "command": command,
    }))
}

fn run_process(program: &str, args: &[&str], cwd: &Path) -> Result<Value> {
    let output = ProcessCommand::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        bail!(
            "command failed: {} {} (cwd: {})\nstdout: {}\nstderr: {}",
            program,
            args.join(" "),
            cwd.to_string_lossy(),
            stdout,
            stderr
        );
    }
    Ok(json!({
        "program": program,
        "args": args,
        "cwd": cwd.to_string_lossy(),
        "status": output.status.code().unwrap_or(0),
        "stdout": stdout,
        "stderr": stderr,
    }))
}

fn expand_path(path: &Path) -> Result<PathBuf> {
    let raw = path.to_string_lossy();
    let expanded = if raw == "~" || raw.starts_with("~/") {
        let home = std::env::var_os("HOME").context("HOME is required for ~ expansion")?;
        let suffix = raw.strip_prefix("~/").unwrap_or("");
        PathBuf::from(home).join(suffix)
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(expanded.canonicalize().unwrap_or(expanded))
}

fn ensure_runner_execution_enabled() -> Result<()> {
    if std::env::var("CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION")
        .ok()
        .as_deref()
        == Some("1")
    {
        Ok(())
    } else {
        anyhow::bail!("runner execution is gated; omit --execute to create a runner package, or set CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION=1 after reviewing the generated prompt")
    }
}

fn worker_payload_from_file(path: &PathBuf) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read worker file {}", path.to_string_lossy()))?;
    let value: toml::Value = text
        .parse()
        .with_context(|| format!("worker file is not valid TOML: {}", path.to_string_lossy()))?;
    serde_json::to_value(value).context("failed to convert worker TOML to JSON")
}

fn result_payload(args: ResultSubmitArgs) -> Result<Value> {
    let mut payload = if let Some(path) = args.from_file {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read result file {}", path.to_string_lossy()))?;
        let value: Value = serde_json::from_str(&text).with_context(|| {
            format!("result file is not valid JSON: {}", path.to_string_lossy())
        })?;
        if !value.is_object() {
            anyhow::bail!("result file must contain a JSON object");
        }
        value
    } else {
        json!({
            "workorder_id": args.workorder_id.context("--workorder-id is required unless --from-file is used")?,
            "status": args.status.context("--status is required unless --from-file is used")?,
            "summary": args.summary.context("--summary is required unless --from-file is used")?,
            "next_action": args.next_action.context("--next-action is required unless --from-file is used")?,
        })
    };
    let object = payload
        .as_object_mut()
        .context("result payload is not an object")?;
    object.insert("_target_id".to_owned(), json!(args.target_id));
    Ok(payload)
}

fn print_payload(payload: &Value, as_json: bool) -> Result<()> {
    if as_json {
        println!("{}", serde_json::to_string_pretty(payload)?);
    } else if let Some(status) = payload.get("status").and_then(Value::as_str) {
        println!("{status}");
    } else {
        println!("ok");
    }
    Ok(())
}
