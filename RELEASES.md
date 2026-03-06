# FreeSynergy.Node – Release History

---

## v0.0.3 — 2026-03-07

Installer i18n framework. All user-facing output is now translatable.

### What's new

- **i18n framework** — `_()` translation function + `load_translations()` embedded in `fsn-install.sh`
- **German translation** — all ~130 installer strings translated to German (`de`)
- **Locale auto-detection** — reads `LC_ALL` / `LANG`, strips encoding and region suffix
- **Translation files** — `locales/en.pot` (template) + `locales/de.po` (German)
- **Build tool** — `tools/generate-i18n.sh` converts `.po` files into embedded `declare -A` arrays
- **printf-style placeholders** — strings with variables use `%s` in both key and translation for correct word order in all languages
- **Graceful fallback** — unknown locale silently falls back to English; bash < 4.3 also falls back gracefully

### Architecture

Translations are embedded directly in `fsn-install.sh` as `declare -A _TR_xx=(...)` arrays.
No external downloads. No version mismatch risk. Single self-contained file.

`.po` files in `locales/` are the source of truth for translators.
`tools/generate-i18n.sh` is the build step (run after editing `.po` files).

### Still open

- Multi-host deployment
- Federation
- Cloudflare DNS
- Additional languages (Phase B)

---

## v0.0.2 — 2026-03-06

First full install works end-to-end. All deployment lifecycle operations implemented.

### What's new

- **Full deploy lifecycle** — deploy, undeploy, remove, restart, update all implemented
- **Sub-module recursion** — postgres and dragonfly sub-modules are correctly
  started, stopped, restarted, removed as part of their parent module's lifecycle
- **Critical bug fix** — Ansible `set_fact` scope: sub-module deployment no longer
  overwrites parent's `instance_name`, `container`, `module_environment`
- **Proxy KDL marker system** — Zentinel config is auto-generated and hot-reloaded
  on every deploy; stale blocks for removed services are cleaned up
- **vault.yml auto-generation** — project secrets generated on first install;
  DNS token baked in directly from installer wizard
- **DNS variables** — `project_services`, `dns_provider`, `dns_api_token` now
  correctly set in all stack playbooks (deploy, undeploy, remove)
- **Constraint enforcement** — `per_host` constraint checked on sync
- **Ansible Collections** — `ansible.posix` and `community.general` installed
  automatically via `requirements.yml`
- **Installer improvements** — ACME contact email added to wizard and project.yml

### Still open

- Multi-host deployment
- Federation
- Cloudflare DNS
- i18n (installer messages)
- File header standardization

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
