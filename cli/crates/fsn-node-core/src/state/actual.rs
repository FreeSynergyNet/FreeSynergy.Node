// Actual state – what IS currently running (observed from systemd/podman).

/// Observed state of all services on this host.
#[derive(Debug, Clone, Default)]
pub struct ActualState {
    pub services: Vec<ServiceStatus>,
}

impl ActualState {
    pub fn find(&self, name: &str) -> Option<&ServiceStatus> {
        self.services.iter().find(|s| s.name == name)
    }
}

/// Status of a single running (or missing) service.
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    /// Instance name (e.g. "forgejo")
    pub name: String,

    pub state: RunState,
    pub health: HealthStatus,

    /// Deployed version recorded by the last deploy operation.
    pub deployed_version: String,

    pub container_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// systemd unit active + container running
    Running,
    /// systemd unit loaded but inactive
    Stopped,
    /// systemd unit in failed state
    Failed,
    /// No systemd unit / container found
    Missing,
}

impl RunState {
    /// i18n key for human-readable status label.
    /// Defined here (not in fsn-tui) so any consumer can translate without reimplementing.
    pub fn i18n_key(self) -> &'static str {
        match self {
            RunState::Running => "status.running",
            RunState::Stopped => "status.stopped",
            RunState::Failed  => "status.error",
            RunState::Missing => "status.unknown",
        }
    }
}

impl std::fmt::Display for RunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunState::Running => write!(f, "running"),
            RunState::Stopped => write!(f, "stopped"),
            RunState::Failed  => write!(f, "failed"),
            RunState::Missing => write!(f, "missing"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Starting,
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy   => write!(f, "healthy"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
            HealthStatus::Starting  => write!(f, "starting"),
            HealthStatus::Unknown   => write!(f, "unknown"),
        }
    }
}
