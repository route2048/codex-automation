use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn run_json(args: &[&str], app_home: &Path) -> Value {
    run_json_with_env(args, app_home, &[])
}

fn run_json_with_env(args: &[&str], app_home: &Path, extra_env: &[(&str, &str)]) -> Value {
    let mut command = Command::cargo_bin("codex-automation").expect("binary should build");
    command
        .args(args)
        .arg("--json")
        .env("CODEX_AUTOMATION_HOME", app_home);
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&output).expect("command should print JSON")
}

fn run_failure(args: &[&str], app_home: &Path) -> String {
    let mut command = Command::cargo_bin("codex-automation").expect("binary should build");
    let output = command
        .args(args)
        .arg("--json")
        .env("CODEX_AUTOMATION_HOME", app_home)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    String::from_utf8(output).expect("stderr should be utf8")
}

fn git(path: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_git_repo(path: &Path) {
    git(path, &["init"]);
    git(path, &["config", "user.email", "codex@example.test"]);
    git(path, &["config", "user.name", "Codex Test"]);
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "initial"]);
}

#[test]
fn cli_prints_version() {
    let mut command = Command::cargo_bin("codex-automation").expect("binary should build");
    let output = command
        .arg("--version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("version output should be utf-8");
    assert!(text.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn cli_init_bootstraps_workspace_without_embedded_skill() {
    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let init_codex_home = temp.path().join("init-codex-home");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    std::fs::write(target.join("README.md"), "demo").expect("readme");
    init_git_repo(&target);

    let plan = run_json(
        &[
            "init",
            target.to_str().expect("target path"),
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--target-id",
            "demo-init",
            "--profile",
            "observe",
            "--plan",
        ],
        &app_home,
    );
    assert_eq!(plan["status"], "planned");
    assert_eq!(plan["target_id"], "demo-init");
    assert_eq!(plan["writes"]["target_repo"]["will_write"], false);
    assert_eq!(
        plan["writes"]["control_workspace"]["destination_status"]["collision"],
        false
    );
    assert!(!workspace.exists());
    assert!(!app_home.exists());

    let source_like_workspace = temp.path().join("codex-automation-source-like");
    std::fs::create_dir(&source_like_workspace).expect("source-like workspace dir");
    std::fs::write(source_like_workspace.join("Cargo.toml"), "[workspace]\n")
        .expect("source-like marker");
    let error = run_failure(
        &[
            "init",
            target.to_str().expect("target path"),
            "--workspace",
            source_like_workspace
                .to_str()
                .expect("source-like workspace path"),
            "--target-id",
            "demo-init",
            "--profile",
            "observe",
        ],
        &app_home,
    );
    assert!(error.contains("refusing to initialize control workspace"));
    assert!(error.contains("existing_directory_looks_like_source_repo"));

    let init_codex_home_text = init_codex_home.to_str().expect("init codex home");
    let setup_skill = init_codex_home
        .join("skills")
        .join("codex-automation-setup");
    std::fs::create_dir_all(&setup_skill).expect("setup skill dir");
    std::fs::write(setup_skill.join("SKILL.md"), "name: codex-automation-setup")
        .expect("setup skill marker");

    let init = run_json_with_env(
        &[
            "init",
            target.to_str().expect("target path"),
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--target-id",
            "demo-init",
            "--profile",
            "observe",
        ],
        &app_home,
        &[("CODEX_HOME", init_codex_home_text)],
    );
    assert_eq!(init["status"], "ready_for_handoff");
    assert_eq!(init["target_id"], "demo-init");
    assert_eq!(init["setup_skill"]["status"], "external");
    assert_eq!(init["workspace_action"], "initialized");
    assert_eq!(init["target_registration"]["status"], "registered");
    assert_eq!(init["workspace_destination"]["collision"], false);
    assert_eq!(init["target_git"]["unchanged"], true);
    assert_eq!(init["worker_registrations"].as_array().unwrap().len(), 3);
    assert!(init["worker_registrations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|worker| worker["status"] == "registered"));
    assert_eq!(init["target_pack"]["status"], "generated");
    assert_eq!(init["heartbeat"]["status"], "ok");
    assert_eq!(init["heartbeat"]["runner_execution"], "package_only");
    assert!(init["handoff"]["binary_path"].as_str().is_some());
    assert!(workspace.join("codex-automation.toml").is_file());
    assert!(workspace.join("targets").join("demo-init.toml").is_file());
    assert!(app_home.join("codex-automation.sqlite").is_file());
    assert!(setup_skill.join("SKILL.md").is_file());
    assert!(!target.join(".codex-automation").exists());

    let uninstall_plan = run_json_with_env(
        &[
            "uninstall",
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--codex-home",
            init_codex_home_text,
        ],
        &app_home,
        &[("CODEX_HOME", init_codex_home_text)],
    );
    assert_eq!(uninstall_plan["status"], "planned");
    assert_eq!(uninstall_plan["dry_run"], true);
    assert!(workspace.join("codex-automation.toml").is_file());
    assert!(app_home.join("codex-automation.sqlite").is_file());
    assert!(init_codex_home
        .join("skills")
        .join("codex-automation-setup")
        .join("SKILL.md")
        .is_file());

    let uninstalled = run_json_with_env(
        &[
            "uninstall",
            "--remove-app-state",
            "--remove-skills",
            "--remove-control-workspace",
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--codex-home",
            init_codex_home_text,
            "--yes",
        ],
        &app_home,
        &[("CODEX_HOME", init_codex_home_text)],
    );
    assert_eq!(uninstalled["status"], "ok");
    assert_eq!(uninstalled["dry_run"], false);
    assert!(!workspace.exists());
    assert!(!app_home.exists());
    assert!(!init_codex_home
        .join("skills")
        .join("codex-automation-setup")
        .exists());
    assert!(!target.join(".codex-automation").exists());
}

#[test]
fn cli_registers_target_and_records_result() {
    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    std::fs::create_dir_all(target.join(".github").join("workflows")).expect("workflow dir");
    std::fs::create_dir_all(target.join(".runtime")).expect("runtime dir");
    std::fs::create_dir_all(target.join(".pytest_cache")).expect("pytest cache dir");
    std::fs::create_dir_all(target.join("out")).expect("out dir");
    std::fs::create_dir_all(target.join("src")).expect("src dir");
    std::fs::create_dir_all(target.join("tests")).expect("tests dir");
    std::fs::write(target.join("README.md"), "demo").expect("readme");
    std::fs::write(target.join("src").join("index.ts"), "export {};").expect("source");
    std::fs::write(target.join("tests").join("index.test.ts"), "export {};").expect("test source");
    std::fs::write(
        target.join(".github").join("workflows").join("ci.yml"),
        "name: ci",
    )
    .expect("workflow");
    std::fs::write(target.join(".env"), "SECRET=1").expect("env");
    std::fs::write(target.join(".DS_Store"), "hidden").expect("hidden");
    std::fs::write(target.join("out").join("bundle.js"), "generated").expect("generated");
    std::fs::write(target.join(".runtime").join("token.json"), "{}").expect("token");
    std::fs::write(target.join(".pytest_cache").join("README.md"), "cache").expect("cache");
    init_git_repo(&target);

    let db = run_json(&["db", "doctor"], &app_home);
    assert_eq!(db["status"], "ok");
    assert_eq!(db["target_count"], 0);

    let init = run_json(
        &[
            "workspace",
            "init",
            workspace.to_str().expect("workspace path"),
        ],
        &app_home,
    );
    assert_eq!(init["status"], "initialized");
    assert!(workspace.join("codex-automation.toml").is_file());
    assert!(workspace.join("targets").is_dir());
    assert!(workspace
        .join("workers")
        .join("control-plane.toml")
        .is_file());
    assert!(workspace
        .join("workers")
        .join("repo-maintainer.toml")
        .is_file());
    assert!(workspace.join("workers").join("ops-analyst.toml").is_file());
    assert!(workspace
        .join("workers")
        .join("release-manager.toml")
        .is_file());
    assert!(!workspace.join("worktrees").exists());
    let workspace_config =
        std::fs::read_to_string(workspace.join("codex-automation.toml")).expect("workspace config");
    assert!(workspace_config.contains("[app_state]"));
    assert!(workspace_config.contains("database = "));
    assert!(workspace_config.contains("worktrees = "));
    assert!(workspace_config.contains("logs = "));
    assert!(workspace_config.contains("artifacts = "));
    assert!(workspace_config.contains("backups = "));

    let paths = run_json(
        &[
            "paths",
            "--workspace",
            workspace.to_str().expect("workspace path"),
        ],
        &app_home,
    );
    assert_eq!(paths["status"], "ok");
    assert_eq!(
        paths["app_state"]["state_root"]["path"],
        app_home.to_str().expect("app home")
    );
    assert_eq!(paths["control_workspace"]["config"]["exists"], true);

    let target_payload = run_json(
        &[
            "target",
            "add",
            "demo",
            "--repo",
            target.to_str().expect("target path"),
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--profile",
            "observe",
        ],
        &app_home,
    );
    assert_eq!(target_payload["status"], "registered");
    assert_eq!(target_payload["runtime_state_location"], "app_state");
    assert!(workspace.join("targets").join("demo.toml").is_file());
    assert!(!target.join(".codex-automation").exists());
    let target_config = std::fs::read_to_string(workspace.join("targets").join("demo.toml"))
        .expect("target config");
    assert!(target_config.contains("root = "));
    assert!(target_config.contains("custom_instructions"));
    assert!(target_config.contains("logs = "));
    assert!(target_config.contains("artifacts = "));

    let worker = run_json(
        &[
            "worker",
            "add",
            "demo",
            "--from-file",
            workspace
                .join("workers")
                .join("repo-maintainer.toml")
                .to_str()
                .expect("worker path"),
        ],
        &app_home,
    );
    assert_eq!(worker["status"], "registered");
    assert_eq!(worker["worker_id"], "repo-maintainer");
    assert_eq!(worker["sandbox"], "workspace-write");
    assert!(worker["custom_instructions"]
        .as_str()
        .expect("custom instructions")
        .contains("Read the shared worktree first"));

    let workers = run_json(&["worker", "list", "demo"], &app_home);
    assert_eq!(workers["workers"][0]["id"], "repo-maintainer");

    let pack = run_json(&["target", "pack", "demo"], &app_home);
    assert_eq!(pack["status"], "generated");
    assert!(pack["pack"]["git"]["status"].as_str().is_some());
    let pack_text = std::fs::read_to_string(
        pack["pack_path"]
            .as_str()
            .expect("pack path should be present"),
    )
    .expect("target pack should exist");
    assert!(pack_text.contains("suggested_workers"));
    assert!(pack_text.contains(".github/workflows/ci.yml"));
    assert!(pack_text.contains("repo-maintainer"));
    assert!(pack_text.contains("ops-analyst"));
    assert!(!pack_text.contains(".env"));
    assert!(!pack_text.contains(".DS_Store"));
    assert!(!pack_text.contains(".runtime"));
    assert!(!pack_text.contains(".pytest_cache"));
    assert!(!pack_text.contains("out/bundle.js"));

    let submitted = run_json(
        &[
            "result",
            "submit",
            "demo",
            "--workorder-id",
            "wo-1",
            "--status",
            "fixed",
            "--summary",
            "ok",
            "--next-action",
            "no_action",
        ],
        &app_home,
    );
    assert_eq!(submitted["status"], "recorded");
    assert_eq!(submitted["workorder_status"], "completed");

    let listed = run_json(&["result", "list", "demo"], &app_home);
    assert_eq!(listed["results"][0]["workorder_id"], "wo-1");

    let migrated = run_json(&["db", "migrate"], &app_home);
    assert_eq!(migrated["status"], "migrated");
    assert_eq!(migrated["schema_version"], 3);

    let updated = run_json(
        &[
            "update",
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--target-id",
            "demo",
        ],
        &app_home,
    );
    assert_eq!(updated["status"], "updated");
    assert_eq!(updated["mode"], "apply");
    assert_eq!(updated["database"]["status"], "migrated");
    assert_eq!(updated["database_check"]["schema_version"], 3);
    assert_eq!(updated["workspace"]["status"], "ok");
    assert_eq!(updated["targets"]["status"], "ok");
    assert_eq!(updated["target"]["status"], "ok");
    assert_eq!(updated["target_pack"]["status"], "generated");
    assert_eq!(updated["heartbeat"]["status"], "ok");
    assert_eq!(updated["heartbeat"]["dry_run"], true);
    assert_eq!(updated["runner_execution"], "not_supported");
    assert_eq!(updated["runner_handoff"], "codex_app");

    let checked = run_json(
        &[
            "update",
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--target-id",
            "demo",
            "--check",
        ],
        &app_home,
    );
    assert_eq!(checked["status"], "checked");
    assert_eq!(checked["mode"], "check");
    assert_eq!(checked["database"]["status"], "ok");
    assert!(checked["target_pack"].is_null());
    assert_eq!(checked["heartbeat"]["dry_run"], true);

    let workorder = run_json(
        &[
            "workorder",
            "create",
            "demo",
            "--id",
            "wo-2",
            "--kind",
            "repo_discovery",
            "--title",
            "Inspect target",
            "--payload-json",
            r#"{"scope":"read_only"}"#,
        ],
        &app_home,
    );
    assert_eq!(workorder["status"], "created");
    assert_eq!(workorder["workorder_status"], "queued");

    let loop_run = run_json(&["loop", "run", "demo"], &app_home);
    assert_eq!(loop_run["status"], "ready_for_worker");
    assert_eq!(loop_run["workorder_id"], "wo-2");

    let runner = run_json(
        &[
            "runner",
            "dispatch",
            "demo",
            "--workorder-id",
            "wo-2",
            "--worker",
            "repo-maintainer",
        ],
        &app_home,
    );
    assert_eq!(runner["status"], "handoff_ready");
    assert_eq!(runner["command"]["mode"], "handoff");
    assert_eq!(runner["command"]["runner"], "codex-app");
    assert_eq!(runner["command"]["worker"]["id"], "repo-maintainer");
    let prompt_path = runner["command"]["package"]["prompt_path"]
        .as_str()
        .expect("prompt path");
    let prompt = std::fs::read_to_string(prompt_path).expect("prompt should exist");
    assert!(prompt.contains("repo-maintainer"));
    assert!(prompt.contains("Custom Instructions"));
    assert!(prompt.contains("Keep orchestration simple"));
    assert!(prompt.contains("Follow the target repository AGENTS.md files"));
    assert!(prompt.contains("Read the shared worktree first"));
    assert!(prompt.contains("Working directory"));
    assert_eq!(runner["command"]["worktree"]["mode"], "shared_per_target");
    assert_eq!(
        runner["command"]["working_directory"],
        runner["command"]["worktree"]["path"]
    );
    let working_directory = runner["command"]["working_directory"]
        .as_str()
        .expect("working directory");
    assert!(Path::new(working_directory).join(".git").exists());
    assert!(prompt.contains("codex-automation result submit demo --workorder-id wo-2"));
    let rendered = run_json(
        &[
            "prompt",
            "render",
            "demo",
            "--workorder-id",
            "wo-2",
            "--worker",
            "repo-maintainer",
        ],
        &app_home,
    );
    assert_eq!(rendered["status"], "rendered");
    assert!(rendered["prompt"]
        .as_str()
        .expect("rendered prompt")
        .contains("Read the shared worktree first"));
    let command_path = runner["command"]["package"]["command_path"]
        .as_str()
        .expect("command path");
    let command_text = std::fs::read_to_string(command_path).expect("command should exist");
    let command_json: Value = serde_json::from_str(&command_text).expect("command json");
    assert_eq!(command_json["mode"], "handoff");
    assert_eq!(command_json["execution"], "package_only");
    assert!(command_json["binary_path"].as_str().is_some());
    assert!(command_json["result_contract"]["command"]
        .as_str()
        .expect("result command")
        .contains("result submit demo --workorder-id wo-2"));
    let handoff_path = runner["command"]["package"]["handoff_path"]
        .as_str()
        .expect("handoff path");
    assert!(std::fs::read_to_string(handoff_path)
        .expect("handoff should exist")
        .contains("Codex Automation Handoff"));
    let result_path = runner["command"]["package"]["result_path"]
        .as_str()
        .expect("result path");
    assert!(result_path.ends_with("result.json"));

    let runner_list = run_json(&["runner", "list", "demo"], &app_home);
    assert_eq!(
        runner_list["runner_runs"][0]["runner_status"],
        "handoff_ready"
    );
    assert_eq!(
        runner_list["runner_runs"][0]["worker_id"],
        "repo-maintainer"
    );
    assert_eq!(
        runner_list["runner_runs"][0]["worker_name"],
        "Repo Maintainer"
    );
    let runner_id = runner["runner_id"].as_i64().expect("runner id").to_string();
    let runner_status = run_json(&["runner", "status", "demo", &runner_id], &app_home);
    assert_eq!(
        runner_status["command"]["package"]["prompt_path"],
        prompt_path
    );
    assert_eq!(runner_status["worker_id"], "repo-maintainer");

    let approval = run_json(
        &[
            "approval",
            "request",
            "demo",
            "--workorder-id",
            "wo-2",
            "--approval-id",
            "approval-wo-2",
            "--reason",
            "Need operator decision",
        ],
        &app_home,
    );
    assert_eq!(approval["status"], "pending");

    let recorded = run_json(
        &[
            "approval",
            "record",
            "demo",
            "approval-wo-2",
            "--decision",
            "approved",
            "--decided-by",
            "test",
            "--message",
            "ok",
        ],
        &app_home,
    );
    assert_eq!(recorded["status"], "recorded");
    assert_eq!(recorded["decision"], "approved");

    let approvals = run_json(&["approval", "list", "demo"], &app_home);
    assert_eq!(approvals["approvals"][0]["decision"], "approved");
}

#[test]
fn cli_heartbeat_generates_pack_and_dispatches_ready_work() {
    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    std::fs::write(target.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("marker");
    init_git_repo(&target);

    run_json(
        &[
            "workspace",
            "init",
            workspace.to_str().expect("workspace path"),
        ],
        &app_home,
    );
    run_json(
        &[
            "target",
            "add",
            "demo",
            "--repo",
            target.to_str().expect("target path"),
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--profile",
            "observe",
        ],
        &app_home,
    );
    run_json(
        &[
            "worker",
            "add",
            "demo",
            "--from-file",
            workspace
                .join("workers")
                .join("repo-maintainer.toml")
                .to_str()
                .expect("worker path"),
        ],
        &app_home,
    );

    let heartbeat = run_json(&["heartbeat", "run", "demo"], &app_home);
    assert_eq!(heartbeat["status"], "ok");
    assert_eq!(heartbeat["dispatch_summary"]["status"], "dispatched");
    assert_eq!(heartbeat["dispatch_summary"]["dispatched_count"], 1);
    assert_eq!(heartbeat["dispatched"][0]["status"], "handoff_ready");
    assert_eq!(
        heartbeat["dispatched"][0]["command"]["worker"]["id"],
        "repo-maintainer"
    );
    let heartbeat_working_directory = heartbeat["dispatched"][0]["command"]["working_directory"]
        .as_str()
        .expect("heartbeat working directory");
    assert!(Path::new(heartbeat_working_directory).join(".git").exists());
    assert!(std::fs::read_to_string(
        heartbeat["target_pack"]["pack_path"]
            .as_str()
            .expect("pack path")
    )
    .expect("target pack")
    .contains("Cargo.toml"));

    let idle_heartbeat = run_json(&["heartbeat", "run", "demo"], &app_home);
    assert_eq!(idle_heartbeat["status"], "ok");
    assert_eq!(idle_heartbeat["dispatch_summary"]["status"], "idle");
    assert_eq!(
        idle_heartbeat["dispatch_summary"]["reason"],
        "active_workorder_exists"
    );
    assert_eq!(idle_heartbeat["dispatch_summary"]["dispatched_count"], 0);
}

#[test]
fn cli_can_ingest_handoff_result_file() {
    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    std::fs::write(target.join("README.md"), "demo").expect("readme");
    init_git_repo(&target);

    run_json(
        &[
            "workspace",
            "init",
            workspace.to_str().expect("workspace path"),
        ],
        &app_home,
    );
    run_json(
        &[
            "target",
            "add",
            "demo",
            "--repo",
            target.to_str().expect("target path"),
            "--workspace",
            workspace.to_str().expect("workspace path"),
            "--profile",
            "observe",
        ],
        &app_home,
    );
    run_json(
        &[
            "worker",
            "add",
            "demo",
            "--from-file",
            workspace
                .join("workers")
                .join("repo-maintainer.toml")
                .to_str()
                .expect("worker path"),
        ],
        &app_home,
    );
    run_json(
        &[
            "workorder",
            "create",
            "demo",
            "--id",
            "wo-exec",
            "--kind",
            "repo_discovery",
            "--title",
            "Handoff worker",
            "--payload-json",
            r#"{"scope":"read_only"}"#,
        ],
        &app_home,
    );
    let started = run_json_with_env(
        &[
            "runner",
            "dispatch",
            "demo",
            "--workorder-id",
            "wo-exec",
            "--worker",
            "repo-maintainer",
        ],
        &app_home,
        &[],
    );
    assert_eq!(started["status"], "handoff_ready");
    assert_eq!(started["command"]["handoff"]["status"], "ready");
    let result_path = started["command"]["package"]["result_path"]
        .as_str()
        .expect("result path");
    std::fs::write(
        result_path,
        r#"{"workorder_id":"wo-exec","status":"discovery_no_findings","summary":"handoff completed","next_action":"no_action"}"#,
    )
    .expect("result file");

    let refreshed = run_json(&["runner", "refresh", "demo"], &app_home);
    assert_eq!(refreshed["runner_refresh"]["checked"], 1);
    assert_eq!(
        refreshed["runner_refresh"]["runners"][0]["status"],
        "completed_from_result"
    );
    let results = run_json(&["result", "list", "demo"], &app_home);
    assert_eq!(results["results"][0]["workorder_id"], "wo-exec");
    assert_eq!(results["results"][0]["status"], "discovery_no_findings");
}
