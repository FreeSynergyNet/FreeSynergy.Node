// Module plugin runner — delegates to fsn-plugin-runtime.
//
// Dispatch priority:
//   1. plugin.wasm  — WASM plugin via wasmtime sandbox  (feature = "wasm")
//   2. plugin       — native process plugin (stdin JSON → stdout JSON)
//
// `ContextBuilder` builds a `fsn_plugin_sdk::PluginContext` from FSN engine types.

use anyhow::Result;
use fsn_plugin_sdk::{PluginContext, PluginResponse};
use fsn_plugin_runtime::ProcessPluginRunner;
use std::path::PathBuf;

// ── ModuleRunner ──────────────────────────────────────────────────────────────

/// Plugin runner for a store module directory.
///
/// Tries `plugin.wasm` (WASM runtime) first, then falls back to
/// the `plugin` executable (process plugin protocol).
pub struct ModuleRunner {
    store_module_dir: PathBuf,
}

impl ModuleRunner {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { store_module_dir: dir.into() }
    }

    /// Invoke the plugin with the given context and return its response.
    pub fn run(&self, ctx: &PluginContext) -> Result<PluginResponse> {
        #[cfg(feature = "wasm")]
        {
            let wasm_path = self.store_module_dir.join("plugin.wasm");
            if wasm_path.exists() {
                return self.run_wasm(&wasm_path, ctx);
            }
        }

        // Fallback: process plugin executable
        let runner = ProcessPluginRunner::new(&self.store_module_dir);
        Ok(runner.run(ctx)?)
    }

    /// Apply a plugin response: write declared files, run declared shell commands.
    pub fn apply(&self, response: &PluginResponse) -> Result<()> {
        let runner = ProcessPluginRunner::new(&self.store_module_dir);
        Ok(runner.apply(response)?)
    }

    #[cfg(feature = "wasm")]
    fn run_wasm(&self, wasm_path: &std::path::Path, ctx: &PluginContext) -> Result<PluginResponse> {
        use fsn_plugin_runtime::{PluginRuntime, PluginSandbox};

        let runtime = PluginRuntime::new()?;
        let sandbox = PluginSandbox::minimal();
        let mut handle = runtime.load_file(wasm_path, sandbox)?;

        tracing::debug!(
            path = %wasm_path.display(),
            command = %ctx.command,
            "invoking WASM plugin"
        );

        Ok(handle.execute(ctx)?)
    }
}

// ── ContextBuilder ────────────────────────────────────────────────────────────

/// Builds a [`PluginContext`] from resolved FSN engine types.
pub struct ContextBuilder;

impl ContextBuilder {
    /// Construct a [`PluginContext`] from a resolved `ServiceInstance` and its peers.
    pub fn build(
        command: &str,
        instance: &fsn_node_core::state::desired::ServiceInstance,
        project_domain: &str,
        data_root: &str,
        peers: &[&fsn_node_core::state::desired::ServiceInstance],
    ) -> PluginContext {
        use fsn_node_core::resource::VarProvider as _;
        use fsn_plugin_sdk::{InstanceInfo, PeerRoute, PeerService};

        let peer_services: Vec<PeerService> = peers.iter().map(|p| {
            let routes: Vec<PeerRoute> = p.class.contract.routes.iter().map(|r| PeerRoute {
                id:   r.id.clone(),
                path: r.path.clone(),
                strip: r.strip,
            }).collect();

            PeerService {
                name:          p.name.clone(),
                class_key:     p.class_key.clone(),
                types:         p.service_types.iter().map(|t| t.to_string()).collect(),
                domain:        p.service_domain.clone(),
                port:          p.class.meta.port,
                upstream_tls:  p.class.contract.upstream_tls,
                routes,
                exported_vars: p.exported_vars(),
            }
        }).collect();

        // Merge all peer exported vars into a flat env map for the context.
        let env: std::collections::HashMap<String, String> = peers.iter()
            .flat_map(|p| p.exported_vars())
            .collect();

        PluginContext {
            protocol: 1,
            command: command.to_string(),
            instance: InstanceInfo {
                name:           instance.name.clone(),
                class_key:      instance.class_key.clone(),
                domain:         instance.service_domain.clone(),
                project:        project_domain.split('.').next().unwrap_or("").to_string(),
                project_domain: project_domain.to_string(),
                data_root:      data_root.to_string(),
                env:            instance.resolved_env.clone(),
            },
            peers: peer_services,
            env,
        }
    }
}
