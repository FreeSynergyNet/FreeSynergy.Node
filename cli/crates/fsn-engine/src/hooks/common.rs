// Common hook helpers: directory creation, template rendering, podman exec.

use std::path::Path;
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};
use tracing::debug;

use super::HookContext;
use crate::template::TemplateContext;

/// Ensure the instance data directory exists.
pub fn ensure_data_dir(ctx: &HookContext<'_>) -> Result<()> {
    let dir = ctx.instance_data_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating data dir {}", dir.display()))
}

/// Create a directory, optionally setting permissions.
pub fn create_dir(path: &Path, mode: u32) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("creating {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("chmod {:o} {}", mode, path.display()))
}

/// Render a Jinja2 template from the module's templates/ directory.
/// `template_name` is the filename (e.g. "kanidm.toml.j2").
pub fn render_template(ctx: &HookContext<'_>, template_name: &str) -> Result<String> {
    let tpl_path = ctx.templates_dir().join(template_name);
    let source = std::fs::read_to_string(&tpl_path)
        .with_context(|| format!("reading template {}", tpl_path.display()))?;

    let project_root_str = ctx.data_root.parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tctx = TemplateContext {
        project_name:           &ctx.project.project.meta.name,
        project_domain:         &ctx.project.project.domain,
        instance_name:          &ctx.instance.name,
        service_domain:         &ctx.instance.service_domain,
        parent_instance_name:   &ctx.instance.name,
        project_root:           &project_root_str,
        vault:                  ctx.vault,
        cross_vars:             crate::resolve::collect_cross_service_vars(ctx.project),
        module_vars:            std::collections::HashMap::new(),
        plugin_vars:            std::collections::HashMap::new(),
        proxy_services:         Vec::new(),
    };

    crate::template::render(&source, &tctx)
        .with_context(|| format!("rendering template {}", template_name))
}

/// Write a rendered template to disk (only if content changed).
pub fn write_template(
    ctx:           &HookContext<'_>,
    template_name: &str,
    dest:          &Path,
) -> Result<()> {
    let content = render_template(ctx, template_name)?;
    if dest.exists() {
        let existing = std::fs::read_to_string(dest).unwrap_or_default();
        if existing == content { return Ok(()); }
    }
    if let Some(parent) = dest.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(dest, &content)
        .with_context(|| format!("writing {}", dest.display()))
}

/// Run `podman exec {container} {cmd...}` and return stdout.
pub async fn podman_exec(container: &str, args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["exec", container];
    cmd_args.extend_from_slice(args);

    debug!("podman {}", cmd_args.join(" "));

    let out = tokio::process::Command::new("podman")
        .args(&cmd_args)
        .output()
        .await
        .with_context(|| format!("podman exec {} {:?}", container, args))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Many init commands fail with "already exists" – treat as OK
        if stderr.contains("already exists") || stderr.contains("AlreadyExists") {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            anyhow::bail!(
                "podman exec {} {:?} failed ({}): {}",
                container, args, out.status, stderr.trim()
            )
        }
    }
}

/// Read the last N lines from a container's logs.
pub async fn podman_logs_tail(container: &str, lines: usize) -> Result<String> {
    let out = tokio::process::Command::new("podman")
        .args(["logs", "--tail", &lines.to_string(), container])
        .output()
        .await?;
    // logs go to stderr in Podman
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(combined)
}
