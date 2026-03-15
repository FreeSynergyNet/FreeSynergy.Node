// fsn init – Module-driven setup wizard.
//
// Flow:
//   Phase 1 – Project skeleton (if none exists)
//   Phase 2 – Module selection (interactive checklist)
//   Phase 3 – Module requirements (per [[setup.fields]] in each module)
//   Phase 4 – Confirm & (optionally) deploy

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fsn_core::config::{
    find_project,
    resolve_plugins_dir,
    service::FieldType,
    registry::ServiceRegistry,
    vault::VaultConfig,
};
use fsn_deploy::setup::collect_requirements;

pub async fn run(root: &Path) -> Result<()> {
    println!("{}\n", fsn_i18n::t("wizard.title"));

    let (slug, proj_dir) = ensure_project_skeleton(root)?;

    let modules_dir = resolve_plugins_dir(root);
    if modules_dir.exists() {
        select_modules(root, &proj_dir, &slug, &modules_dir)?;
    }

    collect_module_secrets(root, &proj_dir, &modules_dir)?;

    if confirm(&fsn_i18n::t("wizard.deploy-prompt"))? {
        println!("\n{}", fsn_i18n::t("wizard.deploying"));
        super::deploy::run(root, None, None, None).await?;
    } else {
        println!("\n{}", fsn_i18n::t("wizard.setup-complete"));
    }

    Ok(())
}

// ── Phase 1: Project skeleton ─────────────────────────────────────────────

fn ensure_project_skeleton(root: &Path) -> Result<(String, PathBuf)> {
    if let Some(existing) = find_project(root, None) {
        let stem = existing.file_stem().and_then(|s| s.to_str()).unwrap_or("project");
        let slug = stem.trim_end_matches(".project").to_string();
        let proj_dir = existing.parent().unwrap_or(root).to_path_buf();
        println!("{}\n", fsn_i18n::t_with("wizard.project-found", &[("path", &existing.display().to_string())]));
        return Ok((slug, proj_dir));
    }

    println!("{}", fsn_i18n::t("wizard.project-header"));
    let project_name = prompt("Project name", None)?;
    let domain       = prompt("Primary domain (e.g. example.com)", None)?;
    let contact      = prompt("Contact / ACME email", None)?;
    let host_ip      = prompt("Server IPv4 address", None)?;
    let host_ipv6    = prompt_optional("Server IPv6 address (optional)")?;
    let dns_provider = prompt("DNS provider [hetzner/cloudflare/none]", Some("hetzner"))?;
    let acme         = prompt("ACME provider [letsencrypt/smallstep-ca/none]", Some("letsencrypt"))?;

    let slug = project_name.to_lowercase().replace(' ', "-");
    let proj_dir = root.join("projects").join(&slug);
    std::fs::create_dir_all(&proj_dir)?;

    std::fs::write(
        proj_dir.join(format!("{}.project.toml", slug)),
        format!(
            "[project]\nname        = \"{name}\"\ndomain      = \"{domain}\"\ndescription = \"\"\n\n[project.contact]\nemail       = \"{contact}\"\nacme_email  = \"{contact}\"\n\n[load.services]\n# Added by wizard\n",
            name = project_name, domain = domain, contact = contact,
        ),
    )?;

    let hosts_dir = root.join("hosts");
    std::fs::create_dir_all(&hosts_dir)?;
    let ipv6_line = host_ipv6.map(|v| format!("\nipv6   = \"{}\"", v)).unwrap_or_default();
    std::fs::write(
        hosts_dir.join(format!("{}.host.toml", slug)),
        format!(
            "[host]\nname = \"{slug}\"\nip   = \"{ip}\"{ipv6}\n\n[proxy.zentinel]\nservice_class = \"proxy/zentinel\"\n\n[proxy.zentinel.load.plugins]\ndns        = \"{dns}\"\nacme       = \"{acme}\"\nacme_email = \"{contact}\"\n",
            slug = slug, ip = host_ip, ipv6 = ipv6_line,
            dns = dns_provider, acme = acme, contact = contact,
        ),
    )?;

    let vault_path = proj_dir.join("vault.toml");
    if !vault_path.exists() {
        std::fs::write(&vault_path, "# Secrets (vault_ prefix required)\n")?;
    }

    println!("\n{}\n", fsn_i18n::t_with("wizard.skeleton-created", &[("path", &slug)]));
    Ok((slug, proj_dir))
}

// ── Phase 2: Module selection ─────────────────────────────────────────────

fn select_modules(_root: &Path, proj_dir: &Path, slug: &str, modules_dir: &Path) -> Result<()> {
    let registry = ServiceRegistry::load(modules_dir)?;
    let mut all_classes: Vec<(&str, &fsn_core::config::service::ServiceClass)> =
        registry.all().collect();
    all_classes.sort_by_key(|(k, _)| *k);

    if all_classes.is_empty() {
        return Ok(());
    }

    println!("{}", fsn_i18n::t("wizard.module-header"));
    println!("{}\n", fsn_i18n::t("wizard.module-hint"));

    let mut selected: Vec<(String, String)> = Vec::new();

    for (class_key, class) in &all_classes {
        let desc = class.meta.description.as_deref().unwrap_or("");
        let label = format!("  [{:<22}]  {}", class_key, desc);
        if confirm_default_no(&label)? {
            let instance_name = prompt(
                &format!("    Instance name for {}", class_key),
                Some(&class.meta.name),
            )?;
            selected.push((instance_name, class_key.to_string()));
        }
    }

    if selected.is_empty() {
        println!("{}\n", fsn_i18n::t("wizard.no-modules"));
        return Ok(());
    }

    let proj_toml = proj_dir.join(format!("{}.project.toml", slug));
    let mut existing = std::fs::read_to_string(&proj_toml)?;

    let mut additions = String::new();
    for (instance_name, class_key) in &selected {
        additions.push_str(&format!(
            "\n[load.services.{}]\nservice_class = \"{}\"\n",
            instance_name, class_key
        ));
    }

    // Replace placeholder comment if present
    existing = existing.replace("# Added by wizard\n", &additions);
    std::fs::write(&proj_toml, existing)?;
    println!("\n{}\n", fsn_i18n::t_with("wizard.modules-added", &[("n", &selected.len().to_string())]));
    Ok(())
}

// ── Phase 3: Module requirements ─────────────────────────────────────────

fn collect_module_secrets(root: &Path, proj_dir: &Path, modules_dir: &Path) -> Result<()> {
    let slug = proj_dir.file_name().and_then(|n| n.to_str()).unwrap_or("project");
    let proj_toml = proj_dir.join(format!("{}.project.toml", slug));
    if !proj_toml.exists() {
        return Ok(());
    }

    let project = fsn_core::config::ProjectConfig::load(&proj_toml)
        .with_context(|| format!("Loading {}", proj_toml.display()))?;

    if project.load.services.is_empty() || !modules_dir.exists() {
        return Ok(());
    }

    let registry = ServiceRegistry::load(modules_dir)?;
    // During init, vault may not exist yet – load without passphrase (plaintext or empty)
    let vault = VaultConfig::load(proj_dir, None).unwrap_or_default();

    let host_path = root.join("hosts").join(format!("{}.host.toml", slug));
    let host = fsn_core::config::HostConfig::load(&host_path)
        .with_context(|| format!("Loading {}", host_path.display()))?;

    let desired = fsn_deploy::resolve::resolve_desired(&project, &host, &registry, &vault, None)
        .context("Resolving desired state")?;

    let requirements = collect_requirements(&desired);
    if requirements.is_empty() {
        return Ok(());
    }

    println!("{}", fsn_i18n::t("wizard.config-header"));

    let vault_path = proj_dir.join("vault.toml");
    // Load existing vault values to enable skip_if_set
    let mut vault_values: HashMap<String, String> = if vault_path.exists() {
        toml::from_str(&std::fs::read_to_string(&vault_path)?).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let mut added = 0usize;

    for req in &requirements {
        let field = &req.field;
        if field.skip_if_set && vault_values.contains_key(&field.key) {
            continue;
        }

        println!("  [{}] {}", req.class_key, field.label);
        if let Some(desc) = &field.description {
            println!("      {}", desc);
        }

        let value = match &field.field_type {
            FieldType::Secret => {
                if field.auto_generate {
                    let gen = generate_secret(32);
                    let show = format!("{}...{}", &gen[..4], &gen[gen.len()-4..]);
                    let input = prompt_secret(&format!("    auto [{}] (Enter=accept)", show))?;
                    if input.is_empty() { gen } else { input }
                } else {
                    prompt_secret("    value (hidden)")?
                }
            }
            FieldType::Select => {
                for (i, opt) in field.options.iter().enumerate() {
                    println!("      [{}] {}", i + 1, opt);
                }
                let def_idx = field.default.as_deref()
                    .and_then(|d| field.options.iter().position(|o| o == d))
                    .unwrap_or(0);
                let sel = prompt(&format!("    choose"), Some(&format!("{}", def_idx + 1)))?;
                sel.parse::<usize>().ok()
                    .filter(|&n| n >= 1 && n <= field.options.len())
                    .map(|n| field.options[n-1].clone())
                    .unwrap_or_else(|| field.options[def_idx].clone())
            }
            FieldType::Bool => {
                if confirm_default_no("    yes/no")? { "true".into() } else { "false".into() }
            }
            _ => prompt("    value", field.default.as_deref())?,
        };

        if field.key.starts_with("vault_") {
            vault_values.insert(field.key.clone(), value);
            added += 1;
        } else {
            println!("      Note: add {} = {:?} to project.toml [vars]\n", field.key, value);
        }

        println!();
    }

    if added > 0 {
        let mut content = "# Secrets – generated by fsn init. NEVER commit!\n".to_string();
        for (k, v) in &vault_values {
            content.push_str(&format!("{} = {:?}\n", k, v));
        }
        std::fs::write(&vault_path, content)?;
        println!("{}", fsn_i18n::t_with("wizard.vault-updated", &[("n", &vault_values.len().to_string())]));
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn prompt(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(d) => print!("  {} [{}]: ", label, d),
        None    => print!("  {}: ", label),
    }
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let t = buf.trim().to_string();
    Ok(if t.is_empty() { default.unwrap_or("").to_string() } else { t })
}

fn prompt_optional(label: &str) -> Result<Option<String>> {
    print!("  {} (Enter to skip): ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let t = buf.trim().to_string();
    Ok(if t.is_empty() { None } else { Some(t) })
}

fn prompt_secret(label: &str) -> Result<String> {
    print!("  {}: ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn confirm(label: &str) -> Result<bool> {
    print!("  {} [Y/n]: ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(!buf.trim().eq_ignore_ascii_case("n"))
}

fn confirm_default_no(label: &str) -> Result<bool> {
    print!("{} [y/N]: ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().eq_ignore_ascii_case("y"))
}

fn generate_secret(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn slug_generation_replaces_spaces_and_lowercases() {
        let name = "My Awesome Project";
        let slug = name.to_lowercase().replace(' ', "-");
        assert_eq!(slug, "my-awesome-project");
    }

    #[test]
    fn slug_generation_handles_single_word() {
        let name = "FreeSynergy";
        let slug = name.to_lowercase().replace(' ', "-");
        assert_eq!(slug, "freesynergy");
    }

    #[test]
    fn find_project_finds_project_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let proj_dir = tmp.path().join("projects").join("my-project");
        fs::create_dir_all(&proj_dir).unwrap();
        fs::write(proj_dir.join("my-project.project.toml"), "[project]\nname = \"my-project\"\ndomain = \"example.com\"").unwrap();

        let found = find_project(tmp.path(), None);
        assert!(found.is_some(), "should find the project file");
        assert!(found.unwrap().to_string_lossy().ends_with(".project.toml"));
    }

    #[test]
    fn find_project_returns_none_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("projects")).unwrap();

        let found = find_project(tmp.path(), None);
        assert!(found.is_none());
    }

    #[test]
    fn find_project_returns_none_without_projects_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let found = find_project(tmp.path(), None);
        assert!(found.is_none());
    }

    #[test]
    fn generate_secret_has_correct_length() {
        let s = generate_secret(32);
        assert_eq!(s.len(), 32);
    }

    #[test]
    fn generate_secret_is_alphanumeric() {
        let s = generate_secret(64);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()), "secret must be alphanumeric");
    }

    #[test]
    fn generate_secret_varies_between_calls() {
        // With 32 chars from 62-char alphabet, collision probability is negligible
        let a = generate_secret(32);
        let b = generate_secret(32);
        assert_ne!(a, b, "two generated secrets should not be identical");
    }
}
