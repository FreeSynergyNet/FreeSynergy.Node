// Integration test: deploy lifecycle (generate → write Quadlet files).
//
// Uses dry_run = true to skip systemd operations.
// Verifies that .network, .container and .env files are written correctly
// for a minimal service instance.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use fsn_node_core::{
    config::{
        project::{ProjectConfig, ProjectLoad, ProjectMeta, ServiceSlots},
        service::{
            Constraints, ContainerDef, ServiceClass, ServiceContract, ServiceLoad, ServiceMeta,
            ServiceSetup, ServiceType,
        },
        vault::VaultConfig,
        meta::ResourceMeta,
    },
    state::desired::{DesiredState, ServiceInstance},
};
use fsn_deploy::deploy::{deploy_all, DeployOpts};
use indexmap::IndexMap;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn minimal_project(name: &str, domain: &str) -> ProjectConfig {
    ProjectConfig {
        project: ProjectMeta {
            meta: ResourceMeta {
                name: name.to_string(),
                alias: None,
                description: None,
                version: String::new(),
                tags: vec![],
            },
            domain: domain.to_string(),
            language: "en".to_string(),
            languages: vec![],
            install_dir: None,
            contact: None,
            branding: None,
            sites: None,
        },
        services: ServiceSlots::default(),
        load: ProjectLoad::default(),
    }
}

fn minimal_service_instance(name: &str) -> ServiceInstance {
    ServiceInstance {
        name: name.to_string(),
        class_key: format!("git/{name}"),
        class: ServiceClass {
            meta: ServiceMeta {
                name: name.to_string(),
                alias: vec![],
                service_types: vec![ServiceType::Git],
                author: None,
                version: "1.0".to_string(),
                tags: vec![],
                description: None,
                website: None,
                repository: None,
                port: 3000,
                constraints: Constraints::default(),
                federation: None,
                health_path: Some("/health".to_string()),
                health_port: None,
                health_scheme: None,
                capabilities: vec![],
                roles: Default::default(),
                ui: Default::default(),
            },
            vars: IndexMap::default(),
            load: ServiceLoad::default(),
            container: ContainerDef {
                name: name.to_string(),
                image: "ghcr.io/forgejo/forgejo".to_string(),
                image_tag: "9".to_string(),
                networks: vec![],
                volumes: vec!["/data/forgejo:/data".to_string()],
                published_ports: vec![],
                healthcheck: None,
                user: None,
                read_only: false,
                tmpfs: vec![],
                security_opt: vec![],
                ulimits: IndexMap::default(),
            },
            environment: IndexMap::default(),
            setup: ServiceSetup::default(),
            contract: ServiceContract::default(),
            manifest: None,
        },
        service_types: vec![ServiceType::Git],
        resolved_env: {
            let mut m = HashMap::new();
            m.insert("GIT_HOST".to_string(), "forgejo.example.com".to_string());
            m
        },
        service_domain: format!("{name}.example.com"),
        alias_domains: vec![],
        sub_services: vec![],
        version: "9.0.0".to_string(),
        resolved_volumes: vec!["/opt/data/forgejo:/data".to_string()],
        capabilities: vec![],
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn dry_run_writes_quadlet_and_env_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let quadlet_dir = tmp.path().join("systemd");
    let state_dir   = tmp.path().join("state");

    let desired = DesiredState {
        project_name: "testproject".to_string(),
        domain:       "example.com".to_string(),
        services:     vec![minimal_service_instance("forgejo")],
    };
    let project  = minimal_project("testproject", "example.com");
    let vault    = VaultConfig::default();
    let opts = DeployOpts {
        quadlet_dir:    quadlet_dir.clone(),
        state_dir:      state_dir.clone(),
        dry_run:        true,
        health_timeout: Duration::from_secs(1),
        store_root:     None,
        remote_host:    None,
    };

    deploy_all(&desired, &project, &vault, &opts, tmp.path(), tmp.path())
        .await
        .expect("deploy_all dry_run");

    // Network file
    assert!(quadlet_dir.join("fsn-testproject.network").exists(),
        ".network file must be written");

    // Container + env files
    assert!(quadlet_dir.join("forgejo.container").exists(),
        ".container file must be written");
    assert!(quadlet_dir.join("forgejo.env").exists(),
        ".env file must be written");
}

#[tokio::test]
async fn dry_run_container_file_contains_image() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let quadlet_dir = tmp.path().join("systemd");

    let desired = DesiredState {
        project_name: "myproject".to_string(),
        domain:       "myproject.example.com".to_string(),
        services:     vec![minimal_service_instance("forgejo")],
    };
    let opts = DeployOpts {
        quadlet_dir:    quadlet_dir.clone(),
        state_dir:      tmp.path().join("state"),
        dry_run:        true,
        health_timeout: Duration::from_secs(1),
        store_root:     None,
        remote_host:    None,
    };

    deploy_all(
        &desired,
        &minimal_project("myproject", "myproject.example.com"),
        &VaultConfig::default(),
        &opts,
        tmp.path(),
        tmp.path(),
    )
    .await
    .expect("deploy_all dry_run");

    let container = std::fs::read_to_string(quadlet_dir.join("forgejo.container")).unwrap();
    assert!(container.contains("Image=ghcr.io/forgejo/forgejo:9"));
    assert!(container.contains("ContainerName=forgejo"));
    assert!(container.contains("Network=fsn-myproject.network"));
}

#[tokio::test]
async fn dry_run_env_file_contains_resolved_vars() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let quadlet_dir = tmp.path().join("systemd");

    let desired = DesiredState {
        project_name: "envtest".to_string(),
        domain:       "envtest.example.com".to_string(),
        services:     vec![minimal_service_instance("forgejo")],
    };
    let opts = DeployOpts {
        quadlet_dir:    quadlet_dir.clone(),
        state_dir:      tmp.path().join("state"),
        dry_run:        true,
        health_timeout: Duration::from_secs(1),
        store_root:     None,
        remote_host:    None,
    };

    deploy_all(
        &desired,
        &minimal_project("envtest", "envtest.example.com"),
        &VaultConfig::default(),
        &opts,
        tmp.path(),
        tmp.path(),
    )
    .await
    .expect("deploy_all dry_run");

    let env = std::fs::read_to_string(quadlet_dir.join("forgejo.env")).unwrap();
    assert!(env.contains("GIT_HOST=forgejo.example.com"));
}

#[tokio::test]
async fn dry_run_sub_services_written_before_parent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let quadlet_dir = tmp.path().join("systemd");

    // Parent (outline) with a sub-service (postgres)
    let mut parent = minimal_service_instance("outline");
    parent.sub_services = vec![minimal_service_instance("postgres")];

    let desired = DesiredState {
        project_name: "subtest".to_string(),
        domain:       "subtest.example.com".to_string(),
        services:     vec![parent],
    };
    let opts = DeployOpts {
        quadlet_dir:    quadlet_dir.clone(),
        state_dir:      tmp.path().join("state"),
        dry_run:        true,
        health_timeout: Duration::from_secs(1),
        store_root:     None,
        remote_host:    None,
    };

    deploy_all(
        &desired,
        &minimal_project("subtest", "subtest.example.com"),
        &VaultConfig::default(),
        &opts,
        tmp.path(),
        tmp.path(),
    )
    .await
    .expect("deploy_all dry_run with sub-services");

    assert!(quadlet_dir.join("outline.container").exists());
    assert!(quadlet_dir.join("postgres.container").exists());
}
