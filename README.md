# FreeSynergy.Node

**Run your own infrastructure. Trust no one. Cooperate freely.**

FreeSynergy.Node is a modular, decentralized deployment system for self-hosted services.
It uses [Podman Quadlets](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
and [Ansible](https://www.ansible.com/) to deploy a full-featured platform on any Linux server —
without Docker, without root, and without a central controller.

---

## Why This Exists

Most self-hosted platforms assume you trust a single organization, a single cloud provider,
or a single piece of software to hold everything together.

FreeSynergy.Node is built around a different principle: **decentralization with voluntary cooperation**.

- Everyone runs their own instance, on their own hardware.
- Nobody gives their data to anyone else.
- Cooperation with other nodes is possible — but always opt-in, transparent, and revokable.
- You decide who you work with. You decide what you share.

This is not just a technical decision. It is the reason the whole system is designed the way it is.

---

## What It Does

FreeSynergy.Node reads your configuration (which services you want, on which hosts),
generates [Quadlet](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
unit files, and deploys them as rootless `systemd` services via Podman.

It handles:

- **Service deployment** — pull images, generate configs, start containers
- **Reverse proxy** — automatic route collection via [Zentinel](https://zentric.dev/) (Caddy-based)
- **TLS certificates** — automatic via Let's Encrypt (ACME)
- **DNS management** — create and clean up DNS records automatically (Hetzner DNS today, Cloudflare planned)
- **DNS reconciliation** — stale records from renamed services are removed on the next deploy
- **Network isolation** — only the proxy has external access; all other containers are internal
- **Multi-host projects** — one project can span multiple servers
- **Federation** — (designed, implementation in progress) mutual OIDC trust between autonomous nodes

---

## Available Modules

| Category | Module | Description |
|---|---|---|
| `proxy` | **zentinel** | Reverse proxy + TLS + DNS (Caddy-based) |
| `auth` | **kanidm** | Identity provider (OIDC, OAuth2, WebAuthn) |
| `mail` | **stalwart** | Mail server (SMTP, IMAP, JMAP) |
| `git` | **forgejo** | Git hosting + CI/CD |
| `wiki` | **outline** | Team wiki and knowledge base |
| `collab` | **cryptpad** | End-to-end encrypted collaborative documents |
| `chat` | **tuwunel** | Matrix homeserver |
| `tasks` | **vikunja** | Project management and task tracker |
| `tickets` | **pretix** | Event ticketing |
| `maps` | **umap** | Self-hosted OpenStreetMap instance |
| `database` | **postgres** | PostgreSQL database (shared, per project) |
| `cache` | **dragonfly** | Redis-compatible cache (auto-slot assignment) |
| `observability` | **openobserver** | Metrics, logs, traces |
| `observability` | **otel-collector** | OpenTelemetry collector |

---

## How It Works

### Three layers of configuration

```
modules/        Module classes (templates) — git-tracked, reusable
hosts/          One file per server (infrastructure layer) — git-ignored
projects/       What runs where — partially git-tracked
```

**Module classes** define what a service needs: which container image, which ports,
which environment variables, which health checks. They are generic and reusable.

**Host files** define the physical server: IP, proxy settings, DNS provider.
Each server gets exactly one host file. Host files are git-ignored — they contain
infrastructure-specific details that differ per deployment.

**Project files** define which modules run for your specific project:

```yaml
# projects/myproject/myproject.project.yml
project:
  name: "myproject"
  domain: "example.com"

services:
  - name: "git"
    subdomain: "git"
    module_class: "git/forgejo"
    load:
      services: ["database/postgres", "cache/dragonfly"]
```

The same project name across multiple files = the same project on multiple hosts.
The filename tells the deployer where to deploy:

```
myproject.project.yml        → this machine (local)
myproject.hetzner-1.yml      → remote host named "hetzner-1"
myproject.federation.yml     → federation config (who can access)
```

### Deployment flow

```
fsn-install.sh
  → detects OS, installs Python3 + Ansible
  → calls setup-server.yml

setup-server.yml
  → checks Podman ≥ 5.0
  → creates deploy user, enables linger
  → calls install-project.yml

deploy-stack.yml
  → reads all *.project.yml files for the project
  → resolves module dependencies
  → generates Quadlet unit files
  → generates container env files
  → deploys via systemd
  → collects proxy routes → updates Zentinel config
  → creates/reconciles DNS records
```

---

## Security Model

- All containers run **rootless** (no root, no `--privileged`)
- Only Zentinel binds to external ports (80/443 + TCP 25/143/993 for mail)
- All other containers communicate via internal Podman networks
- No published ports except the proxy
- Secrets live in `projects/{name}/configs/` — git-ignored, never committed
- Host files are git-ignored — infrastructure details stay local

---

## Project Status

**v0.0.1 — Architecture complete, deployment in progress**

| Component | Status |
|---|---|
| Module definitions (all 13 modules) | Done |
| Project/host file schema | Done |
| Ansible playbook structure | Done |
| DNS management (Hetzner) | Done |
| DNS reconciliation (rename cleanup) | Done |
| Bootstrap installer (`fsn-install.sh`) | Done |
| Quadlet generation | Done |
| Deploy/undeploy playbooks | Stub (in progress) |
| Proxy route collection (KDL markers) | In progress |
| Multi-host deploy | Planned |
| Federation | Designed, not yet implemented |
| Cloudflare DNS | Stub (planned) |

ansible-lint: **0 failures, 0 warnings** (Production Profile, 69 files)

---

## Requirements

- Linux (Fedora, Debian, Ubuntu, Arch, CoreOS — detected automatically)
- Podman ≥ 5.0
- Python 3 + Ansible (installed automatically by `fsn-install.sh`)
- A domain name
- A DNS provider with API access (Hetzner DNS today)

---

## Quick Start

```bash
# Download and run the bootstrap installer
curl -fsSL https://raw.githubusercontent.com/FreeSynergyNet/FreeSynergy.Node/main/fsn-install.sh | bash
```

Or manually:

```bash
git clone https://github.com/FreeSynergyNet/FreeSynergy.Node.git
cd FreeSynergy.Node
bash fsn-install.sh
```

The installer will:
1. Detect your OS and install dependencies
2. Walk you through project configuration
3. Create your host file and project file
4. Deploy your services

---

## FreeSynergy.Net

[FreeSynergy.Net](https://freesynergy.net) is the reference deployment of FreeSynergy.Node —
a federated network of autonomous nodes running the full module stack.

It is built with this exact codebase and serves as the live proof of concept.

---

## License

MIT — see [LICENSE](LICENSE).

Note: We are working on a custom license that better reflects the project's values
(freedom, decentralization, voluntary cooperation). See [contributors.md](contributors.md)
for the current contribution policy.

---

## Contributing

Contributions are not yet accepted while the license and CLA are being finalized.
See [contributors.md](contributors.md) for details on what you can do now.

Bug reports and ideas are welcome via [GitHub Issues](https://github.com/FreeSynergyNet/FreeSynergy.Node/issues).
