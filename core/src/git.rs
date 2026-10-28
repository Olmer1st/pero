//! Thin wrapper around the system `git` binary for the book repositories.
//! Arguments are passed as a vector (no shell); the user's and the system's Git configuration are
//! ignored so every repository behaves the same on every machine. Blocking: call from `spawn_blocking`.

use std::{
    path::Path,
    process::{Command, Output},
};

const EMAIL: &str = "pero@localhost";

fn command(dir: &Path, actor: &str) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .args([
            "-c",
            "core.quotepath=off",
            "-c",
            "core.autocrlf=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .arg("-c")
        .arg(format!("user.name={}", clean_actor(actor)))
        .arg("-c")
        .arg(format!("user.email={EMAIL}"));
    c
}

/// A commit author name that cannot break the identity line.
fn clean_actor(actor: &str) -> String {
    let name: String = actor
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>'))
        .collect();
    let name = name.trim();
    if name.is_empty() {
        "pero".to_string()
    } else {
        name.chars().take(60).collect()
    }
}

fn run(dir: &Path, actor: &str, args: &[&str]) -> Result<Output, String> {
    command(dir, actor)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))
}

fn run_ok(dir: &Path, actor: &str, args: &[&str]) -> Result<String, String> {
    let out = run(dir, actor, args)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        // Git's messages name paths and refs, never the file contents.
        Err(format!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub fn init(dir: &Path, actor: &str) -> Result<(), String> {
    run_ok(dir, actor, &["init", "-q", "-b", "main", "--template="]).map(|_| ())
}

/// True when nothing in the working tree differs from the last commit (untracked files count).
pub fn is_clean(dir: &Path, actor: &str) -> Result<bool, String> {
    Ok(run_ok(dir, actor, &["status", "--porcelain"])?.is_empty())
}

/// Stages everything and commits. Returns the new commit ID, or `None` when nothing changed.
pub fn commit_all(dir: &Path, actor: &str, message: &str) -> Result<Option<String>, String> {
    run_ok(dir, actor, &["add", "-A"])?;
    let diff = run(dir, actor, &["diff", "--cached", "--quiet"])?;
    match diff.status.code() {
        Some(0) => return Ok(None),
        Some(1) => {}
        _ => return Err("git diff failed".into()),
    }
    run_ok(dir, actor, &["commit", "-q", "-m", message])?;
    head(dir, actor).map(Some)
}

pub fn head(dir: &Path, actor: &str) -> Result<String, String> {
    run_ok(dir, actor, &["rev-parse", "HEAD"])
}

/// Undoes everything since the last commit. Only called after `is_clean` was true, so it can only
/// discard the change that has just failed.
pub fn rollback(dir: &Path, actor: &str) -> Result<(), String> {
    run_ok(dir, actor, &["reset", "-q", "--hard", "HEAD"])?;
    run_ok(dir, actor, &["clean", "-fdq"]).map(|_| ())
}

#[cfg(test)]
pub fn commit_count(dir: &Path) -> usize {
    run_ok(dir, "pero", &["rev-list", "--count", "HEAD"])
        .unwrap()
        .parse()
        .unwrap()
}

#[cfg(test)]
pub fn log_messages(dir: &Path) -> Vec<String> {
    run_ok(dir, "pero", &["log", "--format=%an|%s"])
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}
