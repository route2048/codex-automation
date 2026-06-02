use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use codex_automation_core::app_dirs::ensure_app_dirs;
use codex_automation_core::control;
use codex_automation_core::storage;
use codex_automation_core::workspace;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

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
    Doctor,
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    App {
        #[command(subcommand)]
        command: AppCommand,
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

#[derive(Debug, Subcommand)]
enum DbCommand {
    Doctor,
    Migrate,
}

#[derive(Debug, Subcommand)]
enum AppCommand {
    Update(AppUpdateArgs),
}

#[derive(Debug, Args)]
struct AppUpdateArgs {
    #[arg(long)]
    check: bool,
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
        Command::Doctor => {
            let dirs = ensure_app_dirs()?;
            Ok(json!({"status": "ok", "app_state": dirs.as_json()}))
        }
        Command::Db { command } => match command {
            DbCommand::Doctor => storage::db_doctor(),
            DbCommand::Migrate => storage::db_migrate(),
        },
        Command::App { command } => match command {
            AppCommand::Update(args) => {
                if !args.check {
                    anyhow::bail!("app update currently supports --check only");
                }
                Ok(json!({
                    "status": "ok",
                    "update_mode": "check_only",
                    "message": "Pull or reinstall the source checkout, then run codex-automation db migrate.",
                }))
            }
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
