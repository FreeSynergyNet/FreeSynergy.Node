use std::path::Path;
use anyhow::{Context, Result};

use fsn_node_core::config::find_project;
use crate::cli::ConfigCommand;

pub async fn run(root: &Path, project: Option<&Path>, cmd: ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Show     => run_show(root, project).await,
        ConfigCommand::Edit     => run_edit(root, project).await,
        ConfigCommand::Validate => run_validate(root, project).await,
    }
}

async fn run_show(root: &Path, project: Option<&Path>) -> Result<()> {
    let path = resolve_project(root, project)?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Reading {}", path.display()))?;
    println!("{}", content);
    Ok(())
}

pub async fn run_edit(root: &Path, project: Option<&Path>) -> Result<()> {
    let path = resolve_project(root, project)?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("Launching editor '{}'", editor))?;
    if !status.success() {
        anyhow::bail!("Editor exited with status {}", status);
    }
    Ok(())
}

pub async fn run_validate(root: &Path, project: Option<&Path>) -> Result<()> {
    let path = resolve_project(root, project)?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Reading {}", path.display()))?;
    let _parsed: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Parsing {}", path.display()))?;
    println!("Config OK: {}", path.display());
    Ok(())
}

fn resolve_project(root: &Path, explicit: Option<&Path>) -> anyhow::Result<std::path::PathBuf> {
    find_project(root, explicit).with_context(|| {
        format!("No *.project.toml found under {}", root.join("projects").display())
    })
}
