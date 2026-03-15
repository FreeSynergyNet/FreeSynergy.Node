// Integration test: parse all modules from the Node.Store.
//
// Loads every .toml in FreeSynergy.Node.Store/Node/modules/ via ServiceRegistry
// and asserts that key modules are present and well-formed.
//
// The test is skipped gracefully when the store directory does not exist
// (e.g. in CI without a checked-out Node.Store repo).

use std::path::PathBuf;

use fsn_core::config::registry::ServiceRegistry;

fn store_modules_dir() -> PathBuf {
    // From cli/crates/fsn-core/ go up 4 levels → /home/kal/Server/
    // then into FreeSynergy.Node.Store/Node/modules/
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../FreeSynergy.Node.Store/Node/modules")
}

#[test]
fn all_store_modules_parse_without_error() {
    let dir = store_modules_dir();
    if !dir.exists() {
        eprintln!("SKIP: Node.Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");
    let classes: Vec<_> = registry.all().collect();

    assert!(!classes.is_empty(), "expected at least one module to be loaded");
    eprintln!("Loaded {} module classes", classes.len());
}

#[test]
fn expected_modules_are_present() {
    let dir = store_modules_dir();
    if !dir.exists() {
        eprintln!("SKIP: Node.Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");

    let required = [
        "git/forgejo",
        "iam/kanidm",
        "mail/stalwart",
        "wiki/outline",
        "chat/tuwunel",
        "database/postgres",
        "cache/dragonfly",
        "monitoring/openobserver",
        "proxy/zentinel",
    ];

    for key in &required {
        assert!(
            registry.get(key).is_some(),
            "expected module '{key}' not found in registry"
        );
    }
}

#[test]
fn all_modules_have_container_image() {
    let dir = store_modules_dir();
    if !dir.exists() {
        eprintln!("SKIP: Node.Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");

    for (key, class) in registry.all() {
        assert!(
            !class.container.image.is_empty(),
            "module '{key}' has empty container.image"
        );
        assert!(
            !class.container.image_tag.is_empty(),
            "module '{key}' has empty container.image_tag"
        );
    }
}

#[test]
fn all_modules_have_healthcheck() {
    let dir = store_modules_dir();
    if !dir.exists() {
        eprintln!("SKIP: Node.Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");

    for (key, class) in registry.all() {
        assert!(
            class.container.healthcheck.is_some(),
            "module '{key}' is missing container.healthcheck (required by convention)"
        );
    }
}

#[test]
fn plugin_dns_and_acme_plugins_parse() {
    let dir = store_modules_dir();
    if !dir.exists() {
        eprintln!("SKIP: Node.Store not found at {}", dir.display());
        return;
    }

    let registry = ServiceRegistry::load(&dir).expect("ServiceRegistry::load");
    let plugins: Vec<_> = registry.all_plugins().collect();

    // At least hetzner + cloudflare + none DNS, and letsencrypt + none ACME
    assert!(plugins.len() >= 5, "expected at least 5 plugins, got {}", plugins.len());

    let required_plugins = [
        "proxy/dns/hetzner",
        "proxy/dns/cloudflare",
        "proxy/acme/letsencrypt",
    ];
    for key in &required_plugins {
        assert!(
            registry.get_plugin("proxy",
                key.split('/').nth(1).unwrap(),
                key.split('/').nth(2).unwrap(),
            ).is_some(),
            "expected plugin '{key}' not found"
        );
    }
}
