// State diff – what needs to change to reach desired state.

use crate::state::desired::ServiceInstance;

/// The result of comparing desired state with actual state.
#[derive(Debug, Clone, Default)]
pub struct StateDiff {
    /// Modules to create (not currently running).
    pub to_deploy: Vec<ServiceInstance>,

    /// Modules to update (running, but version changed).
    pub to_update: Vec<ServiceInstance>,

    /// Service names to remove (running, but not in desired state).
    pub to_remove: Vec<String>,

    /// Service names already in desired state (no action needed).
    pub ok: Vec<String>,
}

impl StateDiff {
    pub fn is_empty(&self) -> bool {
        self.to_deploy.is_empty() && self.to_update.is_empty() && self.to_remove.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "deploy={}, update={}, remove={}, ok={}",
            self.to_deploy.len(),
            self.to_update.len(),
            self.to_remove.len(),
            self.ok.len(),
        )
    }
}
