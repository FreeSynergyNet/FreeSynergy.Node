# FreeSynergy.Node – Complete Rule Set

## Philosophy: Why This Exists

FreeSynergy.Node is built around one core principle: **decentralization with
voluntary cooperation**.

Everyone runs their own instance. Nobody has to trust a central authority.
Nobody gives their data to anyone else. Anyone can install this themselves,
on their own hardware, without asking permission.

At the same time, cooperation is possible – but always opt-in, always
transparent, always revokable. You decide who you work with. You decide
what you share.

This is not just a technical decision. It is the reason the whole system
is designed the way it is.

---

## Core Concept

```
FreeSynergy.Node = Modular, decentralized Podman/Quadlet deployment system
│
├── modules/       Git-tracked: reusable module classes (templates)
├── hosts/         Git-tracked: one file per host (infrastructure layer)
├── projects/      Mixed: project files git-tracked, data/configs git-ignored
│   ├── {name}.project.yml       local deployment (this machine)
│   └── {name}.{hostname}.yml    remote deployment (other machine, same project)
│
└── Ansible reads everything -> generates Quadlets -> deploys
```

---

## Directory Structure

```
/opt/FreeSynergy.Node/
├── modules/
│   ├── proxy/
│   │   ├── plugins/                       # plugins belong to the TYPE
│   │   │   ├── dns/
│   │   │   │   ├── hetzner.yml
│   │   │   │   └── cloudflare.yml
│   │   │   └── acme/
│   │   │       ├── letsencrypt.yml
│   │   │       └── smallstep-ca.yml
│   │   └── zentinel/                      # module = directory
│   │       ├── zentinel.yml               # class file (same name as dir)
│   │       ├── playbooks/
│   │       │   ├── deploy-dns.yml
│   │       │   ├── deploy-kdl.yml
│   │       │   └── undeploy-dns.yml
│   │       ├── templates/
│   │       │   └── zentinel.kdl.j2
│   │       └── zentinel-control-plane/
│   │           └── zentinel-control-plane.yml
│   ├── auth/
│   │   └── kanidm/
│   │       ├── kanidm.yml
│   │       ├── playbooks/deploy-setup.yml
│   │       └── templates/kanidm.toml.j2
│   ├── mail/stalwart/
│   ├── git/forgejo/
│   ├── wiki/outline/
│   ├── collab/cryptpad/
│   ├── database/postgres/
│   └── cache/dragonfly/
│
├── hosts/
│   └── hetzner-1.host.yml                 # one file per host
│
├── playbooks/
│   ├── install-project.yml
│   ├── deploy-stack.yml
│   ├── undeploy-stack.yml
│   ├── update-stack.yml
│   ├── restart-stack.yml
│   ├── clean-stack.yml
│   ├── remove-stack.yml
│   ├── generate-config-examples.yml
│   ├── templates/
│   │   ├── container.quadlet.j2
│   │   └── container.env.j2
│   └── tasks/
│       ├── generate-quadlet.yml
│       ├── generate-single-example.yml
│       ├── run-module-hooks.yml
│       └── record-deployed-version.yml
│
└── projects/
    └── FreeSynergy.Net/
        ├── freesynergy.project.yml        # git-tracked (local modules)
        ├── freesynergy.turbo.yml          # git-tracked (remote host modules)
        ├── freesynergy.federation.yml     # git-tracked (federation config)
        ├── configs/                       # git-ignored (instance secrets)
        └── data/                          # git-ignored (container volumes)
```

---

## File Naming Convention (Projects)

The filename defines the deployment target:

```
{projectname}.project.yml       -> local (this machine)
{projectname}.{hostname}.yml    -> remote (copied to host, executed there)
{projectname}.federation.yml    -> federation config (who can access)
```

Same project name = same project. Different file = different host.
The remote file is copied to the target host and deployed there.
It can load modules and create containers just like a local project file.
No `host:` field needed – the filename is the information.

---

## Host File (Infrastructure Layer)

One file per physical or virtual host. The host file defines what runs
at the OS/infrastructure level, independent of any project.

A host file is ALWAYS required, even for localhost. The installer
creates it automatically during setup.

```yaml
# hosts/hetzner-1.host.yml
host:
  name: "hetzner-1"
  ip: "1.2.3.4"
  ipv6: "2a01::1"               # optional
  external: false               # true = no SSH access, read-only

  proxy:
    zentinel:
      module_class: "proxy/zentinel"
      load:
        plugins:
          dns: "hetzner"
          acme: "letsencrypt"
  # Routes auto-collected from ALL project files on this host.
  # No manual list needed.
```

Host files are git-ignored. Only example files are tracked:
```
hosts/
├── example.host.yml              # git-tracked (template)
└── .gitignore                    # ignores everything except example*
```

Key rules:
- `external: false` = Ansible can connect and deploy
- `external: true` = no deployment, only config variables are read
- The proxy lives in the host file, not in any project file
- The proxy collects routes automatically from all projects on this host

### Why the Proxy Belongs to the Host

The proxy is bound to an IP address. An IP belongs to a host, not to a
project. Multiple projects share one proxy per host. Therefore the proxy
is defined at host level and collects its routes automatically.

### Proxy KDL Management (Marker-Based)

The proxy KDL config can be modified by two sources: the deployer
(Ansible) and the Control-Plane (manual/API). To prevent conflicts,
the deployer uses markers to identify its managed sections:

```
# === FSN-MANAGED-START: forgejo ===
forgejo.freesynergy.net {
    reverse_proxy forgejo:3000
}
# === FSN-MANAGED-END: forgejo ===
```

Rules:
- Deployer only touches content between its own markers
- New service → add new marker block
- Changed service → update marker block
- Removed service → delete marker block
- Content outside markers is NEVER touched (Control-Plane owned)
- After changes: Zentinel hot-reload (no restart needed)

---

## Project File Structure

```yaml
# projects/FreeSynergy.Net/freesynergy.project.yml
project:
  name: "FreeSynergy.Net"
  domain: "freesynergy.net"
  description: "..."

  branding:
    logo: "branding/logo.svg"
    favicon: "branding/favicon.ico"
    background: "branding/background.jpg"
    css: "branding/custom.css"
    color_primary: "#1a7f64"
    color_accent: "#22d3ee"

  contact:
    email: "admin@freesynergy.net"
    acme_email: "admin@freesynergy.net"

load:
  modules:
    "kanidm":
      module_class: "auth/kanidm"
    "stalwart":
      module_class: "mail/stalwart"
    "forgejo":
      module_class: "git/forgejo"
    "outline":
      module_class: "wiki/outline"
    "cryptpad":
      module_class: "collab/cryptpad"
```

Branding files live in the project directory (git-tracked):
```
projects/FreeSynergy.Net/
├── branding/                    # git-tracked
│   ├── logo.svg
│   ├── favicon.ico
│   ├── background.jpg
│   └── custom.css
├── freesynergy.project.yml
├── configs/
└── data/
```

Each module decides if and how it uses branding. Modules that support
custom logos, CSS, or colors read from `project.branding`. Modules
that don't support it simply ignore the block.

Kanidm can display logos per federation group or OIDC client.

Rules:
- Only modules running on THIS host belong in `*.project.yml`
- Modules on other hosts go in `{name}.{hostname}.yml`
- No cross-host references inside a single project file
- The proxy is NOT listed here – it lives in the host file

---

## external Flag

Can be set at two levels independently:

```yaml
# Host level: no SSH access at all
host:
  external: true

# Instance level: service exists but is not mine to deploy
load:
  modules:
    their-wiki:
      module_class: "wiki/outline"
      external: true    # no container created, only connection vars used
```

Use cases:
- `host.external: true` = server I know about but cannot control
- `instance.external: true` = service someone else runs, I only consume it

---

## DNS Convention

```
Project name  = domain          freesynergy.net
Module name   = subdomain       forgejo.freesynergy.net
Module alias  = CNAME           git.freesynergy.net -> forgejo.freesynergy.net
Host.ip       = A-Record        forgejo.freesynergy.net -> 1.2.3.4
Host.ipv6     = AAAA-Record     forgejo.freesynergy.net -> 2a01::1
```

DNS records are generated automatically. The deployer collects all modules
from all projects on a host and creates DNS entries for each one.

---

## Security: Ports and Network Isolation

### No sudo required

Low ports (80, 443, 25, 587) are handled via kernel parameter:
```
net.ipv4.ip_unprivileged_port_start=80
```
Set once during server setup via `/etc/sysctl.d/`. After that, Podman
can bind any port without root. No sudo, no capabilities, no risk.

Minimum Podman version: 5.0 (Quadlet support + stability).

### Network isolation (MANDATORY)

```
Internet -> Zentinel (80, 443)
                |
                +-- HTTPS (Layer 7) -> services via internal Podman networks
                |
                +-- SMTP/IMAP (Layer 4 TCP) -> stalwart via internal network
```

Rules:
- ONLY Zentinel has external network access
- Zentinel forwards SMTP/IMAP via Layer-4 TCP to Stalwart
- Stalwart has NO published_ports
- All other services: internal networks only, zero external access
- No service may reach the internet except through Zentinel

If a service is compromised, it cannot phone home. The blast radius
of any breach is contained to its local network segment.

### Firewall Management

Firewall ports are managed by the proxy module via deploy/undeploy hooks.
Only the proxy opens external ports. All other services stay internal.

Rules:
- Deploy opens ports: 80/tcp, 443/tcp (always), 25/587/993/tcp (if mail)
- Undeploy closes the same ports
- Only port 22 (SSH) is open by default
- Firewall changes use `firewalld` (permanent + immediate)
- No module other than the proxy may open firewall ports

---

## Data and Configs Location

```
projects/{ProjectName}/
├── configs/{instance_name}/    # git-ignored (secrets + instance config)
└── data/{instance_name}/       # git-ignored (container volumes)
```

- Directory name = instance name (not module class name)
- `data/wiki/` if instance is named "wiki", even if module_class is wiki/outline
- One `tar` of the project directory = complete backup
- `config_dir` in every module: `{{ project_root }}/data/{{ instance_name }}`

---

## Vault vs Config Variables

```yaml
# vault_ prefix = SECRETS (passwords, tokens, private keys)
vault_db_password             # database password
vault_hetzner_dns_api_token   # API token
vault_outline_secret_key      # application secret

# No prefix = CONFIGURATION (not secret, instance-specific)
outline_domain                # "wiki.freesynergy.net"
log_level                     # "info"
oidc_auth_uri                 # "https://auth.freesynergy.net/..."
```

Rule: `vault_` only when the value must never appear in logs or plain text.

---

## Module Interface (Standard)

### Module vs Service

A **module** is a class/template: `auth/kanidm`, `git/forgejo`. It is a
blueprint. A **service** is a running instance created from a module. It
has a name, a subdomain, a port. The proxy knows services. DNS knows
services. Everything outside of `load:` operates on services.

### load: at Project Level vs Module Level

**Project level** (`*.project.yml`):
Only `load.modules` exists. These are the main programs of the project.
Each entry creates a service. No `load.services` at project level because
nothing is instantiated yet to reference.

Sub-modules (postgres, dragonfly) CAN be loaded at project level if the
service should be shared across multiple modules. In that case, modules
reference the shared instance via `load.services` instead of loading
their own sub-instance.

**Module level** (inside a module class):
- `load.modules` = sub-dependencies. Creates a sub-instance owned by this
  module (e.g. forgejo loads its own postgres).
- `load.services` = config access. Reads another service's variables
  without creating anything. Used when a module needs to know where
  another service lives (e.g. its domain, port, credentials).

The proxy uses `load.services` to discover which services need subdomains
and aliases. If a sub-module of the proxy is not a standalone module, it
is loaded via `load.modules` within the proxy module.

### Service Naming and Network Grouping

In `load.services`, a name prefix can be added. This becomes the network
name: `{prefix}-{servicename}-net`. This allows grouping services into
shared networks when needed.

### Plugins

Plugins are NOT modules. They are helper configurations per type, with
their own permissions scope. Examples: DNS providers, ACME providers.
They live in `modules/{type}/plugins/{plugin_type}/`.

### Plugin Convention

Plugins belong to a **type**, not to a specific module. All modules
of the same type can use the same plugins. For example, all proxy
modules (zentinel, traefik, nginx) can use the same DNS and ACME plugins.

Directory structure:
```
modules/{type}/plugins/{plugin_type}/{name}.yml
```

Example:
```
modules/proxy/plugins/
├── dns/
│   ├── hetzner.yml
│   └── cloudflare.yml
└── acme/
    ├── letsencrypt.yml
    └── smallstep-ca.yml
```

#### Plugin Interface

```yaml
# modules/proxy/plugins/dns/hetzner.yml
plugin:
  name: "hetzner"
  type: "dns"
  description: "Hetzner DNS API provider"

vars:
  dns_provider: "hetzner"
  dns_api_url: "https://dns.hetzner.com/api/v1"
  dns_api_token: "{{ vault_dns_api_token }}"
  dns_ttl: 300
```

```yaml
# modules/proxy/plugins/acme/letsencrypt.yml
plugin:
  name: "letsencrypt"
  type: "acme"
  description: "Let's Encrypt ACME provider"

vars:
  acme_provider: "letsencrypt"
  acme_ca_url: "https://acme-v02.api.letsencrypt.org/directory"
  acme_email: "{{ acme_contact_email }}"
```

Field order: `plugin` → `vars`

Plugins have NO `load:`, `container:`, or `environment:` blocks.
They are purely variable collections. The module decides what to do
with the variables in its own templates.

#### Plugin Loading

Plugins are loaded in the host file via `load:` on the proxy:

```yaml
# hosts/hetzner-1.host.yml
host:
  name: "hetzner-1"
  ip: "1.2.3.4"

  proxy:
    zentinel:
      module_class: "proxy/zentinel"
      load:
        plugins:
          dns: "hetzner"
          acme: "letsencrypt"
```

The module resolves `dns: "hetzner"` to `modules/proxy/plugins/dns/hetzner.yml`
and loads its vars.

#### Plugin Secrets

Secret values (tokens, keys) are set in the instance config, not in the
plugin file. The plugin references them via `vault_` variables:

```yaml
# Plugin defines the variable name:
vars:
  dns_api_token: "{{ vault_dns_api_token }}"

# Instance config provides the value:
# projects/FreeSynergy.Net/configs/zentinel/zentinel.yml
vault_dns_api_token: "my-secret-token"
```

Different hosts can use the same plugin with different secrets, because
each host has its own instance config directory.

```yaml
# modules/{type}/{name}/{name}.yml

module:
  name: "{name}"
  alias: []               # optional -> CNAME records
  dns: {}                 # ONLY for type: mail (mx, srv, txt)
  type: "{type}"
  author: "FreeSynergy.Node"
  version: "1.0.0"        # increment to trigger update-stack
  tags: []
  description: "..."
  website: "..."
  repository: "..."
  port: {internal_port}   # always required

  constraints:            # deployer enforces these
    per_host: ~           # max instances per host (null = unlimited)
    per_ip: ~             # max instances per IP
    locality: ~           # "same_host" = must run with consumer

  federation:             # optional, only if module supports federation
    enabled: false        # true = can be used by federated partners
    min_trust: 3          # minimum trust level required (0-4)

vars:
  config_dir: "{{ project_root }}/data/{{ instance_name }}"

load:
  modules: {}             # loads module class -> creates sub-instance (e.g. postgres, dragonfly)
  services: {}            # reads another service's config vars, no container created

container:
  name: "{{ instance_name }}"
  image: "..."
  image_tag: "latest"
  networks: []            # AUTO-GENERATED - never set manually
  volumes:
    - "{{ vars.config_dir }}/data:/data:Z"
  published_ports: []     # FORBIDDEN except proxy

environment:
  SECRET: "{{ vault_secret }}"
  DOMAIN: "{{ service_domain }}"
  LOG_LEVEL: "{{ log_level | default('info') }}"
```

### Field Order (mandatory)
`module` → `vars` → `load` → `container` → `environment`

---

## Module Hook Convention

Hooks in `modules/{type}/{name}/playbooks/`:

| Glob | Triggered by | When |
|------|-------------|------|
| `deploy-*.yml` | `deploy-stack.yml` | after container start |
| `undeploy-*.yml` | `undeploy-stack.yml` | before container stop |
| `update-*.yml` | `update-stack.yml` | after module update |
| `restart-*.yml` | `restart-stack.yml` | after restart |
| `clean-*.yml` | `clean-stack.yml` | during cleanup |
| `remove-*.yml` | `remove-stack.yml` | before removal |

Auto-discovery via glob. File exists = runs. Missing = skipped silently.

---

## Version Management

One Git repo, version in module class:
```yaml
module:
  version: "1.2.0"   # increment to trigger update-stack
```

`update-stack.yml` compares `module.version` vs `instance.deployed_version`.
After deploy, `deployed_version` is written to the instance config.

---

## Global Playbooks

| Playbook | Purpose |
|----------|---------|
| `install-project.yml` | Setup + generate config examples |
| `sync-stack.yml` | Compare desired vs actual state across all hosts (read-only) |
| `deploy-stack.yml` | Sync + apply changes (calls sync-stack internally) |
| `undeploy-stack.yml` | undeploy hooks + stop |
| `update-stack.yml` | Version check + redeploy changed modules |
| `restart-stack.yml` | Stop + start + restart hooks |
| `clean-stack.yml` | Remove orphaned resources across all project hosts |
| `remove-stack.yml` | remove hooks + delete everything |

### deploy-stack options
```
deploy-stack.yml                   -> all hosts (local + remote)
deploy-stack.yml --local           -> only this host
deploy-stack.yml --host turbo      -> only host turbo
```

### sync-stack logic
```
1. Read all project files: *.project.yml (local) + *.{hostname}.yml (remote)
2. Together = desired state of the entire project
3. Check each host: does actual state match desired state?
4. Report: missing, diverged, extra, ok
5. No changes made (read-only)
```

### deploy-stack logic
```
1. Run sync-stack (compare desired vs actual)
2. For each host:
   - Missing module → deploy
   - Config diverged → update config + restart
   - Extra (not in any project file) → remove
   - Matching → skip
3. Result: entire project consistent across all hosts
```

### clean-stack rules
Cleans across all hosts belonging to this project.
Removes orphaned resources (services no longer in any project file).
Never touches services belonging to other projects.

---

## Installer Design

Entry point:
```
fsn-install.sh    <- Bash: checks Python3 + Ansible only
    └── fsn-install.yml  <- Ansible: everything else
```

First run flow:
```
1. New project or update existing?
2. Local or remote host?
   (hint: "you can also manage other servers from here")
3. Project name -> creates projects/{name}/ directory
4. Connection: SSH key path, user (path saved, never the key itself)
5. Domain, IPs, module repo URL
   (default: github.com/Lord-KalEl/FreeSynergy.Node)
6. Generate project file with ALL variables (empty strings for unset)
7. Open $EDITOR (fallback: vi) -> user fills in values
8. After editor closes: validate + deploy
```

Config file = single source of truth. Second run:
```
fsn-install.sh --config projects/FreeSynergy.Net/freesynergy.project.yml
```
No questions. Reads file, deploys. Changed file -> update.

---

## Available Modules

| Class path | Port | Constraints | Hooks |
|-----------|------|-------------|-------|
| `proxy/zentinel` | 443 | per_host:1, per_ip:1 | deploy-dns, deploy-kdl, deploy-firewall, undeploy-dns, undeploy-firewall |
| `proxy/zentinel/zentinel-control-plane` | 8080 | - | - |
| `auth/kanidm` | 8443 | - | deploy-setup |
| `mail/stalwart` | 443 | - | deploy-dns, undeploy-dns |
| `git/forgejo` | 3000 | - | - |
| `wiki/outline` | 3000 | - | - |
| `collab/cryptpad` | 3000 | - | deploy-setup |
| `database/postgres` | 5432 | locality:same_host | - |
| `cache/dragonfly` | 6379 | locality:same_host | - |

---

## Hard Rules

### MUST
```
✅ Module = directory: modules/{type}/{name}/{name}.yml
✅ Class file same name as directory
✅ Shared sub-modules (postgres, dragonfly) at type level
✅ config_dir: project_root/data/instance_name
✅ vault_ prefix ONLY for real secrets
✅ module.version always set
✅ module.port always set
✅ module.constraints set for proxy and locality-bound modules
✅ Field order: module -> vars -> load -> container -> environment
✅ Hook names: {phase}-{resource}.yml
✅ Host file ALWAYS required, even for localhost
✅ Unique service names per project (duplicate = error, abort)
✅ Proxy defined in host file, not project file
✅ Filename: {name}.project.yml = local, {name}.{host}.yml = remote
✅ Filename: {name}.federation.yml = federation config
✅ Federation provider list must be signed (Ed25519)
✅ net.ipv4.ip_unprivileged_port_start=80 on every host
✅ Plugins belong to type level: modules/{type}/plugins/{plugin_type}/
✅ Plugin field order: plugin -> vars (no load, container, environment)
✅ Plugin secrets via vault_ variables in instance config
✅ Firewall ports opened on deploy, closed on undeploy (proxy only)
✅ Comments in files: English only
✅ Chat: German
✅ yamllint: max 160 chars, single space after colon
```

### FORBIDDEN
```
❌ published_ports on any module (proxy is the only exception)
❌ networks: set manually (auto-generated)
❌ Secrets in GIT
❌ dns: block on non-mail modules
❌ ip in module class
❌ vault_ on non-secret variables
❌ Jinja2 in directory names
❌ Duplicate service names within a project
❌ Proxy in project file
❌ Cross-host module references inside a single project file
❌ Plugins inside a module directory (belong to type level)
❌ Plugins with load:, container:, or environment: blocks
❌ Firewall ports opened by any module other than the proxy
❌ Any service communicating directly with the internet
```

---

## Federation

### Philosophy

Federation = decentralized cooperation between autonomous nodes.
Every node is self-sufficient. No node depends on another to function.
If a node goes down, the rest of the network continues.

Federation is NOT app-level federation (ActivityPub, Matrix, SCIM).
Those are protocols of individual apps. Platform federation controls
WHO gets access to WHICH services, via Kanidm + OIDC.

### Architecture

Every federated node runs its own Kanidm. Nodes trust each other
by accepting OIDC tokens from each other's Kanidm instances.
A signed provider list defines which Kanidm instances are trusted.

```
Node A (kanidm.freesynergy.net)     ──┐
Node B (kanidm.alice.example.org)   ──┼── mutual OIDC trust
Node C (kanidm.carol.net)           ──┘
```

Node A dies → B and C continue. Users registered on A cannot log in
elsewhere, but all other users and services are unaffected.

### Provider List (signed, distributed)

One node publishes the provider list. All other nodes fetch it
periodically. The list is signed with Ed25519. If the signing node
goes down, the next node in priority order takes over.

```yaml
# Published at: https://federation.freesynergy.net/providers.yml
federation_providers:
  version: 5
  updated: "2025-02-27T14:30:00Z"
  signed_by: "ed25519:aaa..."
  signature: "ed25519sig:xyz789..."

  providers:
    - issuer: "https://kanidm.freesynergy.net"
      public_key: "ed25519:aaa..."
      name: "FreeSynergy.Net"
      priority: 1
      status: active

    - issuer: "https://kanidm.alice.example.org"
      public_key: "ed25519:bbb..."
      name: "Alice"
      priority: 2
      status: active
```

### Failover

The `priority` field determines the signing order.
Priority 1 = current signer and publisher of the list.

```
1. Fetch list from source URL
2. URL unreachable? → try next node in priority order
3. All unreachable? → use local copy, keep working
4. Primary down permanently? → priority 2 signs a new list,
   becomes the new primary. All nodes already know its public key.
```

### Invite Flow

Partners join via signed invite tokens (Ed25519).

```
1. Operator generates invite token (signed with federation key)
2. Token contains: partner name, trust level, services, expiry
3. Token sent out-of-band (Signal, email, in person)
4. Partner redeems token at Kanidm
5. Kanidm validates signature → creates group with defined permissions
6. Revoke: deactivate Kanidm group → immediate access loss
```

### Federation File

```yaml
# projects/FreeSynergy.Net/freesynergy.federation.yml

federation:
  name: "FreeSynergy Federation"
  signing_key: "{{ vault_federation_signing_key }}"

  provider_list:
    source: "https://federation.freesynergy.net/providers.yml"
    verify_key: "ed25519:abc123..."
    auto_update: true
    update_interval: 3600
    fallback: local

  trusted_issuers:
    - issuer: "https://kanidm.freesynergy.net"
      public_key: "ed25519:aaa..."
      name: "FreeSynergy.Net"
      priority: 1
      added: "2025-01-01"

    - issuer: "https://kanidm.alice.example.org"
      public_key: "ed25519:bbb..."
      name: "Alice"
      priority: 2
      added: "2025-03-15"

  subprojects:
    "bob":
      subdomain: "bob"
      services: [kanidm, stalwart]
      modules:
        "forgejo":
          module_class: "git/forgejo"
        "outline":
          module_class: "wiki/outline"
      status: active
```

### Trust Levels

```
Level 0: Public      -> open registration
Level 1: Invited     -> invite token required
Level 2: Approved    -> application + manual approval
Level 3: Trusted     -> full access to shared services
Level 4: Sub-project -> runs on my hardware, I am responsible
```

Permissions within a trust level are managed entirely through
Kanidm groups and OIDC claims. The module only declares
`federation.min_trust` – everything else is Kanidm's job.

---

## Special Cases

**CryptPad**: Two proxy entries required (main + sandbox domain).
**Stalwart**: Receives SMTP/IMAP via Zentinel Layer-4 TCP forward. No published_ports.
**Mail reputation**: Stalwart benefits from a dedicated IP.

### Cache Slot Management (Dragonfly/Redis)

Dragonfly (or Redis) provides 16 database slots (0-15) per instance.
Each service that needs a cache gets one or more slots assigned.

Rules:
- The deployer counts how many `cache_slot_*` variables a module
  references in its `environment:` block
- Slots are assigned sequentially (0, 1, 2, ...) across all services
  on a host that need cache
- When all 16 slots are used, a new cache instance is created
  automatically: `dragonfly`, `dragonfly-2`, `dragonfly-3`, etc.
- Cache is ephemeral – slot assignment does not need to be stable
  across re-deploys
- Each service gets a unique combination of instance + slot
- Some services need 2 slots, some need 0 – determined by environment vars

Example in a module:
```yaml
environment:
  REDIS_URL: "redis://:{{ vault_cache_password }}@{{ cache_host }}:{{ cache_port }}/{{ cache_slot_0 }}"
  REDIS_CACHE_URL: "redis://:{{ vault_cache_password }}@{{ cache_host }}:{{ cache_port }}/{{ cache_slot_1 }}"
```

The deployer resolves `cache_host`, `cache_port`, `cache_slot_0`,
`cache_slot_1` automatically based on which instance has free slots.

---

## Start Command (new chat)

```
Read RULES.md and continue building the FreeSynergy.Node.

Current state:
- Philosophy: decentralized, self-hosted, voluntary cooperation
- Structure: modules/ + hosts/ + projects/
- File convention: {name}.project.yml = local, {name}.{host}.yml = remote
- Proxy lives in host file, not project file
- external: flag on host or instance level
- Security: net.ipv4.ip_unprivileged_port_start=80, no published_ports except proxy
- All services isolated: only Zentinel has external access
- Zentinel forwards SMTP/IMAP via Layer-4 TCP to Stalwart
- Modules: proxy/zentinel (+control-plane, dns-hetzner), auth/kanidm,
  mail/stalwart, git/forgejo, wiki/outline, collab/cryptpad,
  database/postgres, cache/dragonfly
- Module constraints: per_host, per_ip, locality (enforced by deployer)
- Installer: Bash bootstrap -> Ansible, editor-based config
- DNS: projectname=domain, modulename=subdomain, alias=CNAME
- Federation: signed provider list, OIDC trust between nodes,
  priority-based failover, invite tokens (Ed25519 signed)

Next step: {describe what to do next}
```
