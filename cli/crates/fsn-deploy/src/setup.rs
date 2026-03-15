// Setup requirement collection.
//
// Reads setup.fields from every active module (and their sub-modules),
// deduplicates by key (same vault_* key shared across modules → asked only once),
// and returns an ordered list of what the wizard needs to ask.

use fsn_node_core::{
    config::service::SetupField,
    state::desired::{DesiredState, ServiceInstance},
};

/// A setup field requirement tied to the module that declared it.
#[derive(Debug, Clone)]
pub struct SetupRequirement {
    /// Instance name (e.g. "forgejo", "forgejo-postgres")
    pub instance_name: String,
    /// Class key (e.g. "git/forgejo", "database/postgres")
    pub class_key: String,
    pub field: SetupField,
}

/// Gather all `[[setup.fields]]` from all active modules + sub-modules.
/// Deduplicates by `key` — the first occurrence wins (preserves module order).
pub fn collect_requirements(desired: &DesiredState) -> Vec<SetupRequirement> {
    let mut seen_keys: std::collections::HashSet<String> = Default::default();
    let mut out = Vec::new();

    for instance in &desired.services {
        collect_from_instance(instance, &mut seen_keys, &mut out);
    }

    out
}

fn collect_from_instance(
    instance: &ServiceInstance,
    seen_keys: &mut std::collections::HashSet<String>,
    out: &mut Vec<SetupRequirement>,
) {
    for field in &instance.class.setup.fields {
        if seen_keys.insert(field.key.clone()) {
            out.push(SetupRequirement {
                instance_name: instance.name.clone(),
                class_key: instance.class_key.clone(),
                field: field.clone(),
            });
        }
    }
    for sub in &instance.sub_services {
        collect_from_instance(sub, seen_keys, out);
    }
}
