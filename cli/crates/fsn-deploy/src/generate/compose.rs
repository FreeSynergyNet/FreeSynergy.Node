// Docker/Podman Compose YAML generator.
//
// Produces standalone compose.yml + .env.example for public distribution.
// People without FSN can use these files directly:
//   1. Copy compose.yml and .env.example to their server
//   2. Fill in .env from .env.example
//   3. Run: podman-compose up -d   or   docker compose up -d
//
// Variable syntax: ${VAR:-default} — compatible with Docker Compose and Podman Compose.

use fsn_node_core::config::{ProjectConfig, ServiceEntry};

// ── Known image paths ─────────────────────────────────────────────────────────

/// Docker image path for a service class key.
/// Kept here (not in module TOML) so compose generation is registry-free.
fn image_for_class(class_key: &str) -> Option<&'static str> {
    match class_key {
        "proxy/zentinel"     => Some("ghcr.io/freesynergy/zentinel"),
        "git/forgejo"        => Some("codeberg.org/forgejo/forgejo"),
        "iam/kanidm"         => Some("docker.io/kanidm/server"),
        "mail/stalwart"      => Some("docker.io/stalwartlabs/mail-server"),
        "wiki/outline"       => Some("docker.io/outlinewiki/outline"),
        "chat/matrix"        => Some("ghcr.io/tuwunel/tuwunel"),
        "tasks/vikunja"      => Some("docker.io/vikunja/api"),
        "monitoring/netdata" => Some("docker.io/netdata/netdata"),
        _                    => None,
    }
}

/// Variable prefix exported by a service class (for cross-service wiring).
pub fn class_var_prefix(class_key: &str) -> Option<&'static str> {
    match class_key.split('/').next()? {
        "mail"       => Some("MAIL"),
        "iam"        => Some("IAM"),
        "git"        => Some("GIT"),
        "chat"       => Some("CHAT"),
        "wiki"       => Some("WIKI"),
        "tasks"      => Some("TASKS"),
        "collab"     => Some("COLLAB"),
        "monitoring" => Some("MONITORING"),
        "tickets"    => Some("TICKETS"),
        "maps"       => Some("MAPS"),
        _            => None,
    }
}

// ── Main generators ───────────────────────────────────────────────────────────

/// Generate a `compose.yml` for all services in the project.
pub fn generate_compose(project: &ProjectConfig) -> String {
    let mut services = String::new();

    for (name, entry) in &project.load.services {
        let Some(image) = image_for_class(&entry.service_class) else { continue };
        services.push_str(&service_block(name, entry, image));
    }

    format!(
        "# FreeSynergy.Node — auto-generated Docker/Podman Compose\n\
         # Standalone usage:\n\
         #   cp .env.example .env && $EDITOR .env\n\
         #   podman-compose up -d   or   docker compose up -d\n\
         name: ${{PROJECT_NAME}}\n\n\
         services:\n\
         {services}\n\
         networks:\n\
           fsn:\n\
             driver: bridge\n"
    )
}

/// Generate `.env.example` listing all expected variables.
pub fn generate_env_example(project: &ProjectConfig) -> String {
    let mut lines: Vec<String> = vec![
        "# FreeSynergy.Node — environment variables".into(),
        "# Copy to .env and fill in your values".into(),
        String::new(),
        "# ── Project ──────────────────────────────────────────────────────────".into(),
        format!("PROJECT_NAME={}", project.project.meta.name),
        format!("PROJECT_DOMAIN={}", project.project.domain),
        "DATA_DIR=./data".into(),
        String::new(),
    ];

    for (name, entry) in &project.load.services {
        if image_for_class(&entry.service_class).is_none() { continue }

        let name_upper = name.to_uppercase().replace('-', "_");
        let prefix = class_var_prefix(&entry.service_class);
        let subdomain = entry.subdomain.as_deref().unwrap_or(name.as_str());

        lines.push(format!("# ── {} ({}) ──", name, entry.service_class));
        lines.push(format!("{name_upper}_VERSION={}", entry.version));

        if let Some(pfx) = prefix {
            lines.push(format!("{pfx}_DOMAIN={subdomain}.$(PROJECT_DOMAIN)"));
            lines.push(format!("{pfx}_URL=https://{subdomain}.$(PROJECT_DOMAIN)"));
            lines.push(format!("{pfx}_HOST={name}"));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn service_block(name: &str, entry: &ServiceEntry, image: &str) -> String {
    let name_upper = name.to_uppercase().replace('-', "_");
    let prefix     = class_var_prefix(&entry.service_class);
    let ver        = &entry.version;

    let mut block = String::new();
    block.push_str(&format!("  {name}:\n"));
    block.push_str(&format!("    image: {image}:${{{name_upper}_VERSION:-{ver}}}\n"));
    block.push_str(&format!("    container_name: ${{PROJECT_NAME}}_{name}\n"));
    block.push_str("    restart: unless-stopped\n");
    block.push_str("    networks: [fsn]\n");
    block.push_str("    env_file: .env\n");
    block.push_str("    volumes:\n");
    block.push_str(&format!("      - ${{DATA_DIR:-./data}}/{name}:/data\n"));

    if let Some(pfx) = prefix {
        block.push_str("    labels:\n");
        block.push_str("      - \"traefik.enable=true\"\n");
        block.push_str(&format!("      - \"traefik.http.routers.{name}.rule=Host(`${{{pfx}_DOMAIN}}`)\"\n"));
    }

    block.push('\n');
    block
}

/// Convert Jinja2 `{{ variable }}` syntax to Compose `${VARIABLE}` syntax.
/// Used when adapting module environment templates for public templates.
pub fn jinja_to_compose_var(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second '{'
            let mut var_name = String::new();
            while let Some(nc) = chars.next() {
                if nc == '}' && chars.peek() == Some(&'}') {
                    chars.next(); // consume second '}'
                    break;
                }
                var_name.push(nc);
            }
            out.push_str(&format!("${{{}}}", var_name.trim().to_uppercase()));
        } else {
            out.push(c);
        }
    }

    out
}
