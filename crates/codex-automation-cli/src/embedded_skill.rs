use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const SETUP_SKILL_NAME: &str = "codex-automation-setup";

struct SkillAsset {
    path: &'static str,
    contents: &'static [u8],
    executable: bool,
}

const SETUP_SKILL_ASSETS: &[SkillAsset] = &[
    SkillAsset {
        path: "SKILL.md",
        contents: include_bytes!("../../../skills/codex-automation-setup/SKILL.md"),
        executable: false,
    },
    SkillAsset {
        path: "agents/openai.yaml",
        contents: include_bytes!("../../../skills/codex-automation-setup/agents/openai.yaml"),
        executable: false,
    },
    SkillAsset {
        path: "scripts/doctor.py",
        contents: include_bytes!("../../../skills/codex-automation-setup/scripts/doctor.py"),
        executable: true,
    },
    SkillAsset {
        path: "scripts/setup.py",
        contents: include_bytes!("../../../skills/codex-automation-setup/scripts/setup.py"),
        executable: true,
    },
    SkillAsset {
        path: "scripts/update.py",
        contents: include_bytes!("../../../skills/codex-automation-setup/scripts/update.py"),
        executable: true,
    },
];

pub fn install_setup_skill(
    skill: &str,
    codex_home: Option<&Path>,
    overwrite: bool,
    dry_run: bool,
) -> Result<Value> {
    ensure_supported_skill(skill)?;
    let codex_home = resolve_codex_home(codex_home)?;
    let destination = skill_path(&codex_home);
    let status = setup_skill_status_at(&codex_home)?;
    let missing = status
        .get("missing")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let changed = status
        .get("changed")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let installed = status
        .get("installed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if dry_run {
        return Ok(json!({
            "status": "planned",
            "skill": SETUP_SKILL_NAME,
            "action": if installed { "would_check_or_overwrite" } else { "would_install" },
            "codex_home": display_path(&codex_home),
            "path": display_path(&destination),
            "missing": missing,
            "changed": changed,
            "restart_required": true,
        }));
    }
    if installed && missing.is_empty() && changed.is_empty() {
        return Ok(json!({
            "status": "already_installed",
            "skill": SETUP_SKILL_NAME,
            "codex_home": display_path(&codex_home),
            "path": display_path(&destination),
            "restart_required": false,
        }));
    }
    if destination.exists() && !overwrite {
        return Ok(json!({
            "status": "needs_overwrite",
            "skill": SETUP_SKILL_NAME,
            "codex_home": display_path(&codex_home),
            "path": display_path(&destination),
            "missing": missing,
            "changed": changed,
            "restart_required": false,
            "next_command": "codex-automation skill install codex-automation-setup --overwrite --json",
        }));
    }
    if destination.exists() {
        let metadata = fs::symlink_metadata(&destination)
            .with_context(|| format!("failed to inspect {}", display_path(&destination)))?;
        if metadata.is_dir() {
            fs::remove_dir_all(&destination)
                .with_context(|| format!("failed to remove {}", display_path(&destination)))?;
        } else {
            fs::remove_file(&destination)
                .with_context(|| format!("failed to remove {}", display_path(&destination)))?;
        }
    }
    for asset in SETUP_SKILL_ASSETS {
        let path = destination.join(asset.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", display_path(parent)))?;
        }
        fs::write(&path, asset.contents)
            .with_context(|| format!("failed to write {}", display_path(&path)))?;
        set_executable_if_needed(&path, asset.executable)?;
    }
    Ok(json!({
        "status": "installed",
        "skill": SETUP_SKILL_NAME,
        "codex_home": display_path(&codex_home),
        "path": display_path(&destination),
        "files": asset_paths(),
        "restart_required": true,
    }))
}

pub fn setup_skill_status(skill: &str, codex_home: Option<&Path>) -> Result<Value> {
    ensure_supported_skill(skill)?;
    let codex_home = resolve_codex_home(codex_home)?;
    setup_skill_status_at(&codex_home)
}

pub fn uninstall_setup_skill(
    skill: &str,
    codex_home: Option<&Path>,
    dry_run: bool,
) -> Result<Value> {
    ensure_supported_skill(skill)?;
    let codex_home = resolve_codex_home(codex_home)?;
    let destination = skill_path(&codex_home);
    let existed = destination.exists();
    if !dry_run && existed {
        let metadata = fs::symlink_metadata(&destination)
            .with_context(|| format!("failed to inspect {}", display_path(&destination)))?;
        if metadata.is_dir() {
            fs::remove_dir_all(&destination)
                .with_context(|| format!("failed to remove {}", display_path(&destination)))?;
        } else {
            fs::remove_file(&destination)
                .with_context(|| format!("failed to remove {}", display_path(&destination)))?;
        }
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

fn setup_skill_status_at(codex_home: &Path) -> Result<Value> {
    let destination = skill_path(codex_home);
    let installed = destination.is_dir();
    let mut missing = Vec::new();
    let mut changed = Vec::new();
    for asset in SETUP_SKILL_ASSETS {
        let path = destination.join(asset.path);
        if !path.is_file() {
            missing.push(asset.path.to_owned());
            continue;
        }
        let existing =
            fs::read(&path).with_context(|| format!("failed to read {}", display_path(&path)))?;
        if existing != asset.contents {
            changed.push(asset.path.to_owned());
        }
    }
    Ok(json!({
        "status": "ok",
        "skill": SETUP_SKILL_NAME,
        "codex_home": display_path(codex_home),
        "path": display_path(&destination),
        "installed": installed,
        "missing": missing,
        "changed": changed,
        "files": asset_paths(),
        "restart_required": false,
    }))
}

fn ensure_supported_skill(skill: &str) -> Result<()> {
    if skill == SETUP_SKILL_NAME {
        Ok(())
    } else {
        bail!("unsupported embedded skill: {skill}");
    }
}

fn resolve_codex_home(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(expand_path(path)?);
    }
    if let Some(raw) = env::var_os("CODEX_HOME") {
        if raw.is_empty() {
            bail!("CODEX_HOME is set but empty");
        }
        return Ok(PathBuf::from(raw));
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".codex"));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(profile).join(".codex"));
    }
    bail!("CODEX_HOME, HOME, or USERPROFILE is required to resolve Codex skills")
}

fn skill_path(codex_home: &Path) -> PathBuf {
    codex_home.join("skills").join(SETUP_SKILL_NAME)
}

fn asset_paths() -> Vec<&'static str> {
    SETUP_SKILL_ASSETS.iter().map(|asset| asset.path).collect()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn expand_path(path: &Path) -> Result<PathBuf> {
    let raw = path.to_string_lossy();
    let expanded = if raw == "~" || raw.starts_with("~/") {
        let home = env::var_os("HOME").context("HOME is required for ~ expansion")?;
        let suffix = raw.strip_prefix("~/").unwrap_or("");
        PathBuf::from(home).join(suffix)
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    Ok(expanded.canonicalize().unwrap_or(expanded))
}

#[cfg(unix)]
fn set_executable_if_needed(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if executable {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to read metadata for {}", display_path(path)))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", display_path(path)))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_executable_if_needed(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}
