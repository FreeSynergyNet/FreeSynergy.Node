// State diff – compare desired vs actual to determine what needs to change.

use fsn_node_core::state::{ActualState, DesiredState, RunState, StateDiff};

/// Compare desired state with actual state and return what needs to change.
pub fn compute_diff(desired: &DesiredState, actual: &ActualState) -> StateDiff {
    let mut diff = StateDiff::default();

    // Check each desired module against actual state
    for instance in &desired.services {
        check_instance(instance, actual, &mut diff);
    }

    // Find services running that are NOT in desired state → remove them
    for service in &actual.services {
        let still_desired = desired
            .services
            .iter()
            .any(|m| m.name == service.name || m.sub_services.iter().any(|s| s.name == service.name));

        if !still_desired && service.state == RunState::Running {
            diff.to_remove.push(service.name.clone());
        }
    }

    diff
}

fn check_instance(
    instance: &fsn_node_core::state::desired::ServiceInstance,
    actual: &ActualState,
    diff: &mut StateDiff,
) {
    match actual.find(&instance.name) {
        None => {
            diff.to_deploy.push(instance.clone());
        }
        Some(status) => {
            if status.state == RunState::Missing {
                diff.to_deploy.push(instance.clone());
            } else if status.deployed_version != instance.version {
                diff.to_update.push(instance.clone());
            } else {
                diff.ok.push(instance.name.clone());
            }
        }
    }

    // Recurse into sub-modules
    for sub in &instance.sub_services {
        check_instance(sub, actual, diff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use indexmap::IndexMap;
    use fsn_node_core::{
        config::service::{
            Capability, Constraints, ContainerDef, ServiceClass, ServiceContract,
            ServiceLoad, ServiceMeta, ServiceSetup, ServiceType,
        },
        state::{
            actual::{ActualState, HealthStatus as ContainerHealth, RunState, ServiceStatus},
            desired::{DesiredState, ServiceInstance},
        },
    };

    fn make_class(name: &str) -> ServiceClass {
        ServiceClass {
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
                health_path: None,
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
                image: format!("docker.io/{name}"),
                image_tag: "latest".to_string(),
                networks: vec![],
                volumes: vec![],
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
        }
    }

    fn make_instance(name: &str, version: &str) -> ServiceInstance {
        ServiceInstance {
            name: name.to_string(),
            class_key: format!("test/{name}"),
            class: make_class(name),
            service_types: vec![ServiceType::Git],
            resolved_env: HashMap::new(),
            service_domain: format!("{name}.example.com"),
            alias_domains: vec![],
            sub_services: vec![],
            version: version.to_string(),
            resolved_volumes: vec![],
            capabilities: vec![],
        }
    }

    fn make_status(name: &str, version: &str, state: RunState) -> ServiceStatus {
        ServiceStatus {
            name: name.to_string(),
            state,
            health: ContainerHealth::Unknown,
            deployed_version: version.to_string(),
            container_id: None,
        }
    }

    fn desired(services: Vec<ServiceInstance>) -> DesiredState {
        DesiredState { project_name: "test".to_string(), domain: "example.com".to_string(), services }
    }

    #[test]
    fn new_instance_goes_to_deploy() {
        let diff = compute_diff(&desired(vec![make_instance("forgejo", "1.0")]), &ActualState::default());
        assert_eq!(diff.to_deploy.len(), 1);
        assert_eq!(diff.to_deploy[0].name, "forgejo");
        assert!(diff.to_update.is_empty());
        assert!(diff.to_remove.is_empty());
    }

    #[test]
    fn missing_state_triggers_deploy() {
        let actual = ActualState { services: vec![make_status("forgejo", "1.0", RunState::Missing)] };
        let diff = compute_diff(&desired(vec![make_instance("forgejo", "1.0")]), &actual);
        assert_eq!(diff.to_deploy.len(), 1);
        assert!(diff.to_update.is_empty());
    }

    #[test]
    fn version_mismatch_triggers_update() {
        let actual = ActualState { services: vec![make_status("forgejo", "1.0", RunState::Running)] };
        let diff = compute_diff(&desired(vec![make_instance("forgejo", "2.0")]), &actual);
        assert!(diff.to_deploy.is_empty());
        assert_eq!(diff.to_update.len(), 1);
        assert_eq!(diff.to_update[0].name, "forgejo");
    }

    #[test]
    fn running_with_matching_version_is_ok() {
        let actual = ActualState { services: vec![make_status("forgejo", "1.0", RunState::Running)] };
        let diff = compute_diff(&desired(vec![make_instance("forgejo", "1.0")]), &actual);
        assert!(diff.to_deploy.is_empty());
        assert!(diff.to_update.is_empty());
        assert!(diff.to_remove.is_empty());
        assert_eq!(diff.ok, vec!["forgejo"]);
    }

    #[test]
    fn running_service_not_in_desired_goes_to_remove() {
        let actual = ActualState { services: vec![make_status("forgejo", "1.0", RunState::Running)] };
        let diff = compute_diff(&desired(vec![]), &actual);
        assert!(diff.to_remove.contains(&"forgejo".to_string()));
    }

    #[test]
    fn stopped_service_not_in_desired_is_not_removed() {
        let actual = ActualState { services: vec![make_status("forgejo", "1.0", RunState::Stopped)] };
        let diff = compute_diff(&desired(vec![]), &actual);
        assert!(!diff.to_remove.contains(&"forgejo".to_string()));
    }

    #[test]
    fn sub_services_are_checked_recursively() {
        let mut parent = make_instance("outline", "1.0");
        parent.sub_services = vec![make_instance("postgres", "15")];
        let diff = compute_diff(&desired(vec![parent]), &ActualState::default());
        assert_eq!(diff.to_deploy.len(), 2);
        let names: Vec<_> = diff.to_deploy.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"outline"));
        assert!(names.contains(&"postgres"));
    }
}
