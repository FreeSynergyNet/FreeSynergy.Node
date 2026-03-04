# FreeSynergy.Node – Open Topics & TODO

This file tracks everything that is not yet decided or implemented.
It is a working document, not a specification.

---

## Architecture: Still Open

### Multi-Host Deployer Logic
**Decision made:**
- `deploy-stack.yml` reads ALL `*.project.yml` files in the project dir
- For remote hosts: reads `*.{hostname}.yml` files
- `sync-stack.yml` compares desired vs actual across all hosts (read-only)
- `deploy-stack.yml` calls sync-stack then applies changes
- `clean-stack.yml` removes orphans across all project hosts
- Deploy supports `--local`, `--host {name}` flags

**TODO:**
- Implement sync-stack.yml (read-only comparison)
- Implement multi-host logic in deploy-stack.yml
- Implement SSH connection to remote hosts for verification
- Define output format for sync-stack (what does the report look like?)

### Proxy Route Collection
**Decision made:** Marker-based KDL management.
Deployer uses `FSN-MANAGED-START/END` markers per service.
Only touches its own markers, never Control-Plane content.
Zentinel hot-reload after changes (no restart).

**TODO:**
- Implement marker-based KDL template generation
- Implement diff logic: compare marker blocks vs desired state
- Trigger Zentinel hot-reload after KDL changes
- Test coexistence with Control-Plane manual changes

### Host File Location
**Decision made:** Host files are git-ignored. Only `example.host.yml`
is tracked in the repo. Each deployment creates its own host files.
A host file is always required, even for localhost.
The installer creates the host file automatically during setup.

**TODO:**
- Create `hosts/example.host.yml` template
- Add `hosts/.gitignore` (ignore all except example*)
- Installer must ask for IP and create host file

### Module Constraints Enforcement
**Decision made:**
- Unique service names per project (duplicate = error, abort)
- Constraints checked during sync-stack and deploy-stack
- Checked across host file AND all project files on that host
- Proxy in host file counts as a module for constraint checks

**TODO:**
- Implement constraint check in sync-stack.yml
- Implement duplicate service name check per project
- Clear error messages: "zentinel already running on this host (per_host: 1)"
- Check constraints across: host file + all *.project.yml on this host

---

## Installer: Mostly Decided

### Server Setup = Outside This Project
Installing the OS (Butane/Ignition, provider-specific setup) is NOT
part of FreeSynergy.Node. Prerequisite: server runs, SSH works.

### fsn-install.sh (Bash Bootstrap)
**Decision made:** Detects OS, installs Python3 + Ansible, hands off.

Flow:
```
1. Detect OS (Debian/Ubuntu/Fedora/CoreOS/Arch...)
2. Detect package manager (apt/dnf/rpm-ostree/pacman)
3. Check Python3, install if missing
4. Check Ansible, install if missing (pip or package manager)
5. SSH key path (default: ~/.ssh/id_ed25519)
6. Hand off to setup-server.yml
```

**TODO:**
- Implement OS detection logic
- Implement package manager mapping
- Handle pip vs system package for Ansible

### setup-server.yml (Ansible)
**Decision made:** Prepares a running server for FreeSynergy.Node.

Flow:
```
1. Check Podman version (minimum 5.0)
2. Set net.ipv4.ip_unprivileged_port_start=80 in /etc/sysctl.d/
3. Check/create deploy user (default: fsn)
4. Enable loginctl enable-linger for the user
5. Switch to deploy user
6. Hand off to install-project.yml
```

**TODO:**
- Implement setup-server.yml
- Podman install logic per distro if not present

### SSH Key Handling
**Decision made:** Save SSH key path, never the key itself.
Verification of key connectivity: nice to have, not critical.

---

## Multi-Server Projects: Still Open

### Deployer on Remote Hosts
Currently all playbooks run on localhost. For remote hosts, the deployer
needs to either SSH in or run on the remote host itself.

**Open:**
- Does each server run its own copy of the platform repo?
- Or does one central machine SSH into all servers?
- Recommendation: Each server runs its own deployer (decentralized)
  but the initial setup can be done remotely

### `{name}.{hostname}.yml` Files
The file convention is defined but the deployer does not yet handle
these files differently from `*.project.yml`.

**TODO:**
- Deployer copies `*.{hostname}.yml` to the target host
- Containers ARE deployed on the remote host (not reference only)
- The file is a full project file for that host
- Config variables from the local project file are available for
  cross-referencing (e.g. shared Kanidm URL)

---

## Federation: Design Done, Implementation Open

### Decisions Made
- Federation = mutual OIDC trust between autonomous Kanidm nodes
- Every node is self-sufficient, no single point of failure
- Signed provider list (Ed25519) distributed across nodes
- Priority-based failover: if signer goes down, next node takes over
- Invite tokens (Ed25519 signed) for partner onboarding
- Trust levels 0-4, permissions managed via Kanidm groups
- Module interface: `federation.enabled` + `federation.min_trust`
- Project file: `{name}.federation.yml` (separate from project.yml)
- Sub-projects: auto-generated project directory on host

### TODO: Implementation

**Provider List:**
- Implement Ed25519 signing/verification for provider list
- Implement auto-update mechanism (fetch + verify + replace local copy)
- Implement failover logic (try next priority when source unreachable)
- Define how a new signer announces takeover to the network
- Endpoint to serve the provider list (static file via Zentinel?)

**Invite Tokens:**
- Implement `fsn federation invite` command (generate signed token)
- Implement `fsn federation join` command (redeem token at Kanidm)
- Implement `fsn federation revoke` command (deactivate Kanidm group)
- Define token format (JWT with Ed25519? Custom format?)

**Kanidm Integration:**
- Auto-create Kanidm groups from federation config
- Map trust levels to Kanidm group permissions
- Configure multiple OIDC providers per service (one per trusted issuer)
- Test: what happens when an OIDC provider is unreachable?

**Sub-Projects (Trust 4):**
- Auto-generate project directory from federation.subprojects config
- Set `parent:` and `managed_by: federation` in generated project file
- DNS generation for sub-project subdomains
- Cleanup when sub-project is removed

**Playbooks:**
- `federation-deploy.yml` – process federation.yml, create Kanidm groups
- `federation-invite.yml` – generate invite token
- `federation-revoke.yml` – deactivate partner
- `federation-update-providers.yml` – fetch and verify provider list

### Still Open

**Q1: Provider list hosting**
Where does the signed provider list live?
- Static file served by Zentinel?
- Dedicated lightweight endpoint?
- Git repo that nodes pull from?

**Q2: Takeover announcement**
When priority 2 takes over as signer, how do other nodes know?
- They just try the next URL and accept the new signature?
- Or is there an explicit "takeover" message?
- Recommendation: implicit – nodes try URLs in priority order,
  accept any valid signature from a known public key

**Q3: Token format**
- JWT with Ed25519 signature? (standard, libraries exist)
- Custom YAML-based format? (simpler, but no tooling)
- Recommendation: JWT (widely supported, well-understood)

**Q4: Future layers**
VPN and Tor/I2P layers are architecturally compatible:
- Zentinel gets additional listeners
- Provider list can contain .onion addresses
- Modules don't change (they don't know about transport)
- Not designed yet, but nothing blocks it

---

## Bugs to Fix

No bugs yet – playbooks have not been created.
When implementing, avoid these known pitfalls:

| # | Pitfall | Correct approach |
|---|---------|-----------------|
| 1 | `import_playbook` inside a task | Use `include_role` or separate playbook step |
| 2 | `hosts: localhost` hardcoded | Use variable `target_host` from inventory |
| 3 | Missing inventory file | Generate inventory from host files |
| 4 | Jinja2 env vars not resolved | Full variable resolver pass during Quadlet generation |

---

## Nice to Have (Later)

- `fsn status` command: show all running services + health
- Auto-update via `systemd timer` calling `update-stack.yml`
- Backup integration: Rustic/Restic hook after deploy
- `fsn logs {instance}` shortcut
- Web UI for non-technical users (much later)
- Forgejo Actions integration for CI/CD on the platform itself

---

## Cache Slot Management: TODO

### Auto-Slot Assignment
The deployer must automatically assign cache slots to services.

**TODO:**
- Scan all modules on a host for `cache_slot_*` environment variables
- Assign slots sequentially (0-15) per dragonfly instance
- When instance full (16 slots used), create next instance (dragonfly-2)
- Resolve `cache_host`, `cache_port`, `cache_slot_N` variables during
  Quadlet generation
- No persistence needed – cache is ephemeral, slots can shift on re-deploy

### Shared vs Per-Module Cache
**Open:**
- If postgres can be shared at project level, should dragonfly also
  be shareable at project level?
- Current model: each module loads its own dragonfly sub-instance
- Alternative: one dragonfly at project level, modules reference via
  `load.services`
- This affects how slots are counted and assigned

---

## Decisions Already Made (for reference)

| Topic | Decision |
|-------|----------|
| Installer language | Bash (bootstrap only) + Ansible (everything else) |
| Port access | `net.ipv4.ip_unprivileged_port_start=80` – no sudo |
| Network isolation | Only Zentinel has external access |
| SMTP/IMAP | Zentinel Layer-4 TCP forward to Stalwart |
| Stalwart published_ports | REMOVED – Zentinel handles it |
| SSH key storage | Path only, never the key content |
| Editor in installer | `$EDITOR` env var, fallback to `vi` |
| Proxy location | Host file, not project file |
| File naming | `{name}.project.yml` = local, `{name}.{host}.yml` = remote |
| external flag | On host AND instance level independently |
| Module constraints | Declared in module class, enforced by deployer |
| Federation | Designed: signed provider list, OIDC trust, priority failover |
| Federation file | `{name}.federation.yml` separate from project file |
| Federation auth | Kanidm groups + OIDC, Ed25519 signed invite tokens |
| Cache slots | Auto-assigned, 16 per instance, overflow to new instance |
| load.services | Config access only, no container, module-level only |
| Project-host files | Copied to remote host and executed there (not reference only) |
| Proxy KDL management | Marker-based, deployer only touches own markers |
| Host files in repo | Git-ignored, only example.host.yml tracked |
| Host file required | Always, even for localhost. Created by installer |
| Service name uniqueness | Must be unique per project, duplicate = abort |
| Sync vs Deploy | sync-stack.yml (read-only) + deploy-stack.yml (sync + apply) |
| Branding | project.branding block, git-tracked branding/ directory |
| Plugins | Type-level, not module-level. Vars only, no containers |
