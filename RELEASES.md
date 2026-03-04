# FreeSynergy.Node – Release History

---

## v0.0.1 — 2026-03-04

Initial public release. Architecture complete, deployment in progress.

### What's included

- **Module definitions** for 13 services:
  `zentinel`, `kanidm`, `stalwart`, `forgejo`, `outline`, `cryptpad`,
  `tuwunel`, `vikunja`, `pretix`, `umap`, `postgres`, `dragonfly`,
  `openobserver`, `otel-collector`
- **Ansible playbook structure** — all entry points defined
- **Quadlet generation** — container + env file templates
- **DNS management** — create, remove, and reconcile DNS records (Hetzner DNS)
- **DNS rename cleanup** — stale records from renamed services are removed automatically
- **Bootstrap installer** (`fsn-install.sh`) — OS detection, dependency install, setup wizard
- **Project/host file schema** — full specification in `RULES.md`
- **FreeSynergy.Net** reference project — 13 services configured

### What's not yet implemented

- Deploy/undeploy playbooks (stubs only)
- Proxy route collection (KDL marker system)
- Multi-host deployment
- Federation
- Cloudflare DNS support

### Quality

- `ansible-lint`: 0 failures, 0 warnings (Production Profile)
- `yamllint`: 0 errors, 0 warnings

---

*Older releases will be listed here as the project grows.*
