use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;
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

fn run_failure(args: &[&str], app_home: &Path, extra_env: &[(&str, &str)]) -> String {
    let mut command = Command::cargo_bin("codex-automation").expect("binary should build");
    command
        .args(args)
        .arg("--json")
        .env("CODEX_AUTOMATION_HOME", app_home);
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let output = command.assert().failure().get_output().stderr.clone();
    String::from_utf8(output).expect("stderr should be utf8")
}

#[test]
fn cli_installs_embedded_setup_skill_and_init_bootstraps_workspace() {
    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let codex_home = temp.path().join("codex-home");
    let init_codex_home = temp.path().join("init-codex-home");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    std::fs::write(target.join("README.md"), "demo").expect("readme");

    let codex_home_text = codex_home.to_str().expect("codex home");
    let before = run_json_with_env(
        &[
            "skill",
            "status",
            "codex-automation-setup",
            "--codex-home",
            codex_home_text,
        ],
        &app_home,
        &[("CODEX_HOME", codex_home_text)],
    );
    assert_eq!(before["status"], "ok");
    assert_eq!(before["installed"], false);

    let installed = run_json_with_env(
        &[
            "skill",
            "install",
            "codex-automation-setup",
            "--codex-home",
            codex_home_text,
        ],
        &app_home,
        &[("CODEX_HOME", codex_home_text)],
    );
    assert_eq!(installed["status"], "installed");
    assert_eq!(installed["restart_required"], true);
    assert!(codex_home
        .join("skills")
        .join("codex-automation-setup")
        .join("SKILL.md")
        .is_file());
    assert!(codex_home
        .join("skills")
        .join("codex-automation-setup")
        .join("scripts")
        .join("setup.py")
        .is_file());

    let init_codex_home_text = init_codex_home.to_str().expect("init codex home");
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
    assert_eq!(init["skill_install"]["status"], "installed");
    assert_eq!(init["workspace_action"], "initialized");
    assert_eq!(init["target_registration"]["status"], "registered");
    assert_eq!(init["worker_registrations"].as_array().unwrap().len(), 3);
    assert!(init["worker_registrations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|worker| worker["status"] == "registered"));
    assert_eq!(init["target_pack"]["status"], "generated");
    assert_eq!(init["heartbeat"]["status"], "ok");
    assert!(workspace.join("codex-automation.toml").is_file());
    assert!(workspace.join("targets").join("demo-init.toml").is_file());
    assert!(app_home.join("codex-automation.sqlite").is_file());
    assert!(init_codex_home
        .join("skills")
        .join("codex-automation-setup")
        .join("SKILL.md")
        .is_file());
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
        .contains("Read the target repository first"));

    let workers = run_json(&["worker", "list", "demo"], &app_home);
    assert_eq!(workers["workers"][0]["id"], "repo-maintainer");

    let pack = run_json(&["target", "pack", "demo"], &app_home);
    assert_eq!(pack["status"], "generated");
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
    assert_eq!(runner["status"], "package_ready");
    assert_eq!(runner["command"]["mode"], "package");
    assert_eq!(runner["command"]["worker"]["id"], "repo-maintainer");
    let prompt_path = runner["command"]["package"]["prompt_path"]
        .as_str()
        .expect("prompt path");
    let prompt = std::fs::read_to_string(prompt_path).expect("prompt should exist");
    assert!(prompt.contains("repo-maintainer"));
    assert!(prompt.contains("Custom Instructions"));
    assert!(prompt.contains("Keep orchestration simple"));
    assert!(prompt.contains("Follow the target repository AGENTS.md files"));
    assert!(prompt.contains("Read the target repository first"));
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
        .contains("Read the target repository first"));
    let command_path = runner["command"]["package"]["command_path"]
        .as_str()
        .expect("command path");
    let command_text = std::fs::read_to_string(command_path).expect("command should exist");
    let command_json: Value = serde_json::from_str(&command_text).expect("command json");
    assert_eq!(command_json["mode"], "package");
    let result_schema_path = runner["command"]["package"]["result_schema_path"]
        .as_str()
        .expect("result schema path");
    assert!(std::fs::read_to_string(result_schema_path)
        .expect("result schema should exist")
        .contains("codex-automation worker result"));

    let runner_list = run_json(&["runner", "list", "demo"], &app_home);
    assert_eq!(
        runner_list["runner_runs"][0]["runner_status"],
        "package_ready"
    );
    let runner_id = runner["runner_id"].as_i64().expect("runner id").to_string();
    let runner_status = run_json(&["runner", "status", "demo", &runner_id], &app_home);
    assert_eq!(
        runner_status["command"]["package"]["prompt_path"],
        prompt_path
    );

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
    assert_eq!(heartbeat["dispatched"][0]["status"], "package_ready");
    assert_eq!(
        heartbeat["dispatched"][0]["command"]["worker"]["id"],
        "repo-maintainer"
    );
    assert!(std::fs::read_to_string(
        heartbeat["target_pack"]["pack_path"]
            .as_str()
            .expect("pack path")
    )
    .expect("target pack")
    .contains("Cargo.toml"));
}

#[cfg(unix)]
#[test]
fn cli_can_start_mock_runner_and_ingest_final_result() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    let mock_codex = temp.path().join("mock-codex");
    std::fs::write(
        &mock_codex,
        r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    shift
    out="$1"
  fi
  shift
done
cat >/dev/null
mkdir -p "$(dirname "$out")"
printf '%s' '{"workorder_id":"wo-exec","status":"discovery_no_findings","summary":"mock runner completed","next_action":"no_action"}' > "$out"
"#,
    )
    .expect("mock codex script");
    let mut permissions = std::fs::metadata(&mock_codex)
        .expect("mock metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&mock_codex, permissions).expect("chmod mock");

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
            "Execute mock runner",
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
            "--execute",
        ],
        &app_home,
        &[
            ("CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION", "1"),
            (
                "CODEX_AUTOMATION_CODEX_BIN",
                mock_codex.to_str().expect("mock path"),
            ),
        ],
    );
    assert_eq!(started["status"], "running");
    assert_eq!(started["command"]["execution"]["status"], "running");

    let mut refreshed = run_json(&["runner", "refresh", "demo"], &app_home);
    for _ in 0..20 {
        if refreshed["runner_refresh"]["runners"][0]["status"] == "completed_from_result" {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        refreshed = run_json(&["runner", "refresh", "demo"], &app_home);
    }
    assert_eq!(refreshed["runner_refresh"]["checked"], 1);
    assert_eq!(
        refreshed["runner_refresh"]["runners"][0]["status"],
        "completed_from_result"
    );
    let results = run_json(&["result", "list", "demo"], &app_home);
    assert_eq!(results["results"][0]["workorder_id"], "wo-exec");
    assert_eq!(results["results"][0]["status"], "discovery_no_findings");
}

#[cfg(unix)]
#[test]
fn cli_blocks_execute_when_worker_concurrency_is_full() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().expect("tempdir");
    let app_home = temp.path().join("app-state");
    let workspace = temp.path().join("codex-automation");
    let target = temp.path().join("target-repo");
    std::fs::create_dir(&target).expect("target repo dir");
    let worker_file = temp.path().join("serial-worker.toml");
    std::fs::write(
        &worker_file,
        r#"version = 1

[worker]
id = "serial-discovery"
name = "Serial Discovery"
description = "Read-only serial worker."
skills = ["repo-discovery"]
allowed_workorder_kinds = ["repo_discovery"]
sandbox = "read-only"
approval_policy = "never"
autonomy_profile = "observe"
max_concurrency = 1
custom_instructions = "Inspect only."
"#,
    )
    .expect("worker file");
    let mock_codex = temp.path().join("mock-codex-sleep");
    std::fs::write(
        &mock_codex,
        r#"#!/bin/sh
cat >/dev/null
sleep 2
"#,
    )
    .expect("mock codex script");
    let mut permissions = std::fs::metadata(&mock_codex)
        .expect("mock metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&mock_codex, permissions).expect("chmod mock");

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
            worker_file.to_str().expect("worker path"),
        ],
        &app_home,
    );
    for id in ["wo-a", "wo-b"] {
        run_json(
            &[
                "workorder",
                "create",
                "demo",
                "--id",
                id,
                "--kind",
                "repo_discovery",
                "--title",
                "Serial work",
                "--payload-json",
                "{}",
            ],
            &app_home,
        );
        run_json(&["loop", "run", "demo"], &app_home);
    }
    let env = [
        ("CODEX_AUTOMATION_ENABLE_RUNNER_EXECUTION", "1"),
        (
            "CODEX_AUTOMATION_CODEX_BIN",
            mock_codex.to_str().expect("mock path"),
        ),
    ];
    let started = run_json_with_env(
        &[
            "runner",
            "dispatch",
            "demo",
            "--workorder-id",
            "wo-a",
            "--worker",
            "serial-discovery",
            "--execute",
        ],
        &app_home,
        &env,
    );
    assert_eq!(started["status"], "running");
    let error = run_failure(
        &[
            "runner",
            "dispatch",
            "demo",
            "--workorder-id",
            "wo-b",
            "--worker",
            "serial-discovery",
            "--execute",
        ],
        &app_home,
        &env,
    );
    assert!(error.contains("max_concurrency"));
}
