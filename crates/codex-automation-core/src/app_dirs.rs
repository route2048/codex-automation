use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppDirs {
    pub state_root: PathBuf,
    pub database: PathBuf,
    pub worktrees: PathBuf,
    pub logs: PathBuf,
    pub artifacts: PathBuf,
    pub backups: PathBuf,
}

impl AppDirs {
    pub fn as_json(&self) -> Value {
        json!({
            "state_root": display_path(&self.state_root),
            "database": display_path(&self.database),
            "worktrees": display_path(&self.worktrees),
            "logs": display_path(&self.logs),
            "artifacts": display_path(&self.artifacts),
            "backups": display_path(&self.backups),
        })
    }
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn resolve_state_root() -> Result<PathBuf> {
    if let Some(raw) = env::var_os("CODEX_AUTOMATION_HOME") {
        if raw.is_empty() {
            bail!("CODEX_AUTOMATION_HOME is set but empty");
        }
        return Ok(PathBuf::from(raw));
    }
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").context("HOME is required on macOS")?;
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("codex-automation"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local_app_data).join("codex-automation"));
        }
        let profile = env::var_os("USERPROFILE")
            .context("LOCALAPPDATA or USERPROFILE is required on Windows")?;
        return Ok(PathBuf::from(profile)
            .join("AppData")
            .join("Local")
            .join("codex-automation"));
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        if let Some(state_home) = env::var_os("XDG_STATE_HOME") {
            if state_home.is_empty() {
                bail!("XDG_STATE_HOME is set but empty");
            }
            return Ok(PathBuf::from(state_home).join("codex-automation"));
        }
        let home = env::var_os("HOME").context("HOME is required when XDG_STATE_HOME is unset")?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("codex-automation"))
    }
}

pub fn app_dirs() -> Result<AppDirs> {
    let state_root = resolve_state_root()?;
    Ok(AppDirs {
        database: state_root.join("codex-automation.sqlite"),
        worktrees: state_root.join("worktrees"),
        logs: state_root.join("logs"),
        artifacts: state_root.join("artifacts"),
        backups: state_root.join("backups"),
        state_root,
    })
}

pub fn ensure_app_dirs() -> Result<AppDirs> {
    let dirs = app_dirs()?;
    std::fs::create_dir_all(&dirs.state_root)
        .with_context(|| format!("failed to create {}", display_path(&dirs.state_root)))?;
    std::fs::create_dir_all(&dirs.worktrees)
        .with_context(|| format!("failed to create {}", display_path(&dirs.worktrees)))?;
    std::fs::create_dir_all(&dirs.logs)
        .with_context(|| format!("failed to create {}", display_path(&dirs.logs)))?;
    std::fs::create_dir_all(&dirs.artifacts)
        .with_context(|| format!("failed to create {}", display_path(&dirs.artifacts)))?;
    std::fs::create_dir_all(&dirs.backups)
        .with_context(|| format!("failed to create {}", display_path(&dirs.backups)))?;
    Ok(dirs)
}
