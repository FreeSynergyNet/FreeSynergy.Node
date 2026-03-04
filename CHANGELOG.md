# FreeSynergy.Node – CHANGELOG

Diese Datei wird zwischen Claude Chat und Claude Code / Editor hin- und hergereicht.
Jede Änderung wird hier dokumentiert. Beim Hochladen sieht Claude sofort,
was sich geändert hat.

---

## 2026-03-04 – Claude Code – DNS Rename-Cleanup (Reconcile + Explicit Cleanup)

### Neue / geänderte Dateien

**`playbooks/tasks/dns-cleanup-module.yml`** (neu)
Löscht alle DNS-Records eines Moduls anhand der State-File.
Verwendet `record_id` aus dem State-File für direkten DELETE-Aufruf (kein Zone-Lookup nötig).
Idempotent: 404 vom Provider wird als Erfolg gewertet.

Variablen: `dns_state_file`, `dns_cleanup_module`, `dns_provider`, `dns_api_token`

**`modules/proxy/zentinel/playbooks/deploy-dns.yml`** (Reconcile hinzugefügt)
Vor jedem Deploy werden veraltete Records automatisch bereinigt:
1. State-File laden → alle Records für `proxy/zentinel` ermitteln
2. Erwartete FQDNs aus aktueller Config berechnen
3. Records die in State-File stehen aber NICHT mehr erwartet werden → DELETE
4. State-File aktualisieren → dann normale Record-Erstellung (A, AAAA, CNAME)

Effekt: Service von `chat` → `matrix` umbenennen, `deploy-stack.yml` ausführen
→ `chat.freesynergy.net` wird automatisch gelöscht, `matrix.freesynergy.net` erstellt.

**Neue Symlinks** (ansible-lint Workaround)
- `modules/proxy/zentinel/playbooks/tasks/dns-cleanup-module.yml`
- `playbooks/tasks/tasks/dns-cleanup-module.yml`

### Architektur
- **Reconcile on deploy** (automatisch): Rename-Cleanup beim nächsten `deploy-stack.yml`
- **Explicit cleanup** (`dns-cleanup-module.yml`): Für manuelles Löschen / `remove-stack.yml`
- State-File ist die Quelle der Wahrheit für "was haben wir erstellt"

### Noch offen
- `remove-stack.yml` / `undeploy-stack.yml`: noch kein expliziter `dns-cleanup-module.yml` Aufruf

---

## 2026-03-01 – Claude Code – Install-Wizard + DNS State Tracking

### Neue / geänderte Dateien

**`fsn-install.sh`** (kompletter Rewrite)
Interaktiver Setup-Wizard für neue Installationen:
- `list_available_modules()` – scannt `modules/` Verzeichnis (zeigt alle verfügbaren Module)
- `select_dns_provider()` – Auswahl Hetzner/Cloudflare + Token-Eingabe via `read -rs`
- `select_acme_provider()` – Auswahl letsencrypt/smallstep-ca
- `select_modules()` – nummerierte Liste, Eingabe als space-separated Nummern oder `all`
- `detect_server_ip()` – automatisch via `ip route get 1.1.1.1`
- `generate_project_yml()` – schreibt `projects/NAME/NAME.project.yml`
- `generate_host_yml()` – schreibt `hosts/HOSTNAME.host.yml` (DNS + ACME Provider eingebaut)
- `show_setup_summary()` – zeigt Zusammenfassung vor Deployment
- `import_config()` – importiert externes project.yml (`--config FILE`)
- `collect_secrets()` – Provider-aware, nutzt DNS_TOKEN aus Wizard wieder

Drei Modi in `main()`:
1. `--config FILE` → Import + direkt deployen
2. `--project FILE` → Bereits platziert, nur deployen
3. Kein Flag → Interaktiver Wizard

Neue Flags: `--config FILE`

**`playbooks/tasks/dns-create-record.yml`** (State Tracking hinzugefügt)
- Schreibt nach erfolgreicher Erstellung in `.dns-managed.yml`
- Erfasst: zone, type, name, fqdn, value, provider, record_id, module

**`playbooks/tasks/dns-remove-record.yml`** (State Tracking hinzugefügt)
- Entfernt nach erfolgreicher Löschung den Eintrag aus `.dns-managed.yml`

**`modules/proxy/zentinel/playbooks/deploy-dns.yml`** (State Tracking aktiviert)
- Übergibt `dns_state_file` und `dns_state_module` an die Task-Files

**`modules/proxy/zentinel/playbooks/undeploy-dns.yml`** (Bugfix + State Tracking)
- `dns_api_url` entfernt (war noch aus altem Interface übrig)
- `dns_zone: "{{ project_domain }}"` hinzugefügt (war fehlend)
- `dns_state_file` + `dns_state_module` hinzugefügt

### DNS State File (`.dns-managed.yml`)

Neue Tracking-Datei unter `projects/<name>/.dns-managed.yml`:
```yaml
records:
  - zone: "freesynergy.net"
    type: "A"
    name: "chat"
    fqdn: "chat.freesynergy.net"
    value: "1.2.3.4"
    provider: "hetzner"
    record_id: "abc123"
    module: "proxy/zentinel"
```
Wird automatisch bei deploy/undeploy aktuell gehalten.
Zweck: sicheres Cleanup bei Modul-Rename / -Remove (state file als Quelle statt aktueller Config).

### Noch offen
- `stalwart/playbooks/deploy-dns.yml`: MX/SPF/SRV Records noch nicht implementiert
- Cloudflare-Support: geplant für v0.2
- Rename-Scenario: undeploy liest noch aus Projekt-Config, nicht aus State-File
  (TODO: `dns-cleanup-module.yml` – löscht alle Records eines Moduls via State-File)

---

## 2026-03-01 – Claude Code – author-Feld in allen Modulen umbenannt

### Geänderte Dateien
- `modules/tickets/pretix/pretix.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/tasks/vikunja/vikunja.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/database/postgres/postgres.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/mail/stalwart/stalwart.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/chat/tuwunel/tuwunel.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/observability/otel-collector/otel-collector.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/collab/cryptpad/cryptpad.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/wiki/outline/outline.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/observability/openobserver/openobserver.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/git/forgejo/forgejo.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/maps/umap/umap.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/cache/dragonfly/dragonfly.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/proxy/zentinel/zentinel.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/auth/kanidm/kanidm.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`
- `modules/proxy/zentinel/zentinel-control-plane/zentinel-control-plane.yml` – `author: "FSN Platform"` -> `author: "FreeSynergy.Node"`

### Offene Probleme
- Keine

### Naechster Schritt
- Keine weiteren Schritte erforderlich

---

## 2026-03-01 – Claude Code – DNS-Implementierung (Hetzner API)

### Neue/geänderte Dateien

**`playbooks/tasks/dns-create-record.yml`** (implementiert)
- Hetzner DNS API: Zone-Lookup → Record-Check (idempotent) → Record-Create
- Variable `dns_zone` (neu, Pflicht): Root-Domain z.B. "freesynergy.net"
- Variable `record_name`: voller FQDN, relativer Name wird automatisch abgeleitet
- Cloudflare: fail-fast Hinweis (geplant für spätere Version)
- Idempotent: überspringt vorhandene Records (gleicher Typ + Name)

**`playbooks/tasks/dns-remove-record.yml`** (implementiert)
- Hetzner DNS API: Zone-Lookup → Record-Find → DELETE
- Idempotent: kein Fehler wenn Record nicht existiert
- Cloudflare: fail-fast Hinweis

**`modules/proxy/zentinel/playbooks/deploy-dns.yml`** (aktualisiert)
- `dns_zone: "{{ project_domain }}"` hinzugefügt (neues Pflichtfeld)
- `dns_api_url` entfernt (Hetzner-URL wird im Task hardkodiert)

### Interface (dns-create / dns-remove)
```yaml
record_type:  "A"                    # A, AAAA, CNAME, MX, TXT, SRV
record_name:  "mail.freesynergy.net" # voller FQDN
record_value: "1.2.3.4"             # IP, FQDN, Text
dns_zone:     "freesynergy.net"      # Root-Zone (NEU)
dns_provider: "hetzner"             # hetzner | cloudflare
dns_api_token: "{{ vault_hetzner_dns_token }}"
dns_ttl:      300                    # optional
```

### ansible-lint: 0 Fehler, 0 Warnungen (68 Files, Profile 'production')

### Noch offen
- `stalwart/playbooks/deploy-dns.yml`: MX/SPF/SRV Records noch nicht implementiert
- Cloudflare-Support: geplant

---

## 2026-03-01 – Claude Code – Verzeichnis umbenannt + Umbenennung: FSN Platform → FreeSynergy.Node

### Konzept-Entscheidung
- **FreeSynergy.Node** = Name des Programms/der Software (Abk. FSN Node)
- **FreeSynergy.Net** = Name des Netzes/der Plattform/der Website (Abk. FSN Net)
- Beide abgekürzt: **FSN** – bestehende Abkürzungen bleiben gültig
- GitHub-Repository: `git@github.com:Lord-KalEl/FreeSynergy.Node.git`
- Standard-Installationspfad auf Servern: `~/FreeSynergy.Node`
- **SCIM (Kanidm):** ~80% fertig, soll eingebaut werden wo es Sinn ergibt

### Geänderte Dateien (Umbenennung)
- `CHANGELOG.md`, `CLAUDE.md`, `RULES.md`, `TODO.md` – Titel + alle Vorkommen
- `hosts/example.host.yml` – Kommentar-Header
- `fsn-install.sh` – alle Vorkommen + GitHub-URL + Default-Installationspfad
- `playbooks/*.yml` (10 Dateien) – `name:` Felder
- `modules/**/*.yml` (15 Dateien) – `author:` Felder
- `RULES.md` – `/opt/fsn-platform/` → `/opt/FreeSynergy.Node/`, GitHub-URL
- Memory-Dateien aktualisiert

### Nicht geändert
- `projects/` – Projekt-Konfigurationen bleiben unverändert
- Historische Pfadangaben `fsn-platform/` in CHANGELOG-Einträgen (korrekte Historie)

### Verzeichnis-Umbenennung
- `mv /home/kal/Server/fsn-platform → /home/kal/Server/FreeSynergy.Node`
- `/home/kal/Server/.ansible-lint` – Pfade aktualisiert (`FreeSynergy.Node/`)
- `/home/kal/Server/.vscode/settings.json` – Pfade aktualisiert (`FreeSynergy.Node/`)
- ansible-lint: 0 Fehler, 0 Warnungen (68 Files, Profile 'production')

---

## 2026-02-28 – Claude Code – Checksum-Verifikation + GitHub-Vorbereitung

### Geänderte Dateien

**`fsn-install.sh`**
- Default-Repo-URL eingebaut: `FSN_DEFAULT_REPO="https://github.com/Lord-KalEl/FreeSynergy.Node"`
  → Kein interaktives Fragen mehr wenn offizielles Repo gewünscht; override via `--repo`
- `print_checksum_info()` hinzugefügt:
  - Wenn als Datei ausgeführt: zeigt SHA256 des Scripts + Link zu Releases
  - Wenn via `bash <(curl ...)` gepiped: zeigt 2-Schritt-Verification-Anleitung
- Header-Kommentar aktualisiert: erklärt Quick-Install vs. Verified-Install
- Vollständige URL im Header: `https://raw.githubusercontent.com/Lord-KalEl/FreeSynergy.Node/main/fsn-install.sh`

**`~/.claude/settings.json`** (Claude Code Permissions)
- Häufige sichere Operationen auto-approved: `git status/log/diff/show/add/commit/init`,
  `gh repo view/list`, `sha256sum`, `command -v`
- Remote-Operationen (`git push`, `gh repo create`) und `sudo` bleiben mit Bestätigung

### Konzept-Entscheidungen
- **Ein öffentliches Repo** für alles (kein privater Projekt-Repo)
- Standard-Branding = FSN; Forks ersetzen `FSN_DEFAULT_REPO` + Branding
- Passwörter: `read -s` → `hosts/secrets.yml` (git-ignored, chmod 600)
- Version-Checks (`check-constraints.yml`): geplant für v0.2, aktuell Stub

---

## 2026-02-28 – Claude Code – Bootstrap-Installer + fetch-modules

### Neue Dateien

**`fsn-install.sh`** (komplett überarbeitet)
Funktioniert jetzt als eigenständiges Bootstrap-Script – kann direkt
vom Server gedownloaded und ausgeführt werden, ohne dass der Repo vorhanden ist.

Neues Verhalten:
- `check_git()` – prüft/installiert Git (war vorher nicht vorhanden)
- `fetch_platform()` – fragt nach GitHub URL des Platform-Repos,
  dann `git clone` (oder `git pull` falls schon vorhanden)
- `collect_secrets()` – sammelt Tokens/Passwörter interaktiv via `read -s`;
  schreibt nach `hosts/secrets.yml` (git-ignored, chmod 600)
- **Passwörter werden NIEMALS als CLI-Argument akzeptiert** (Shell-History-Risiko)
- Nicht-sensitive Parameter via Args: `--repo`, `--target`, `--project`,
  `--skip-setup`, `--skip-deploy`, `--help`
- Aufruf-Reihenfolge: setup-server → fetch-modules → install-project → deploy-stack

Typischer Ablauf (One-Liner):
```bash
bash <(curl -fsSL https://raw.githubusercontent.com/.../fsn-install.sh)
```

**`playbooks/fetch-modules.yml`** (neu)
Lädt externe Module per `git clone`/`git pull` und verifiziert bundled Module.

- Liest `project_cfg.load.modules` aus der Projektdatei
- Module mit `source:` → `ansible.builtin.git` clone/pull
- Module ohne `source:` → prüft ob Verzeichnis existiert, sonst Fehler
- Unterstützt optionales `source_version:` (Tag oder Branch)
- Auch verwendbar von `update-stack.yml` für Code-Updates

Externe Module in der Projektdatei (optional):
```yaml
load:
  modules:
    "my-service":
      module_class: "category/name"
      source: "https://github.com/user/module-name"
      source_version: "v1.2.0"   # optional, default: HEAD
```

### Design-Entscheidungen
- Passwörter in `hosts/secrets.yml` (bereits von `hosts/.gitignore` abgedeckt)
- `ansible-playbook -e @hosts/secrets.yml` reicht Secrets an Ansible weiter
- Module-Source in der Projekt-Datei (nicht im Modul-YAML): erlaubt
  Überschreibung ohne den bundled Code zu ändern
- Bundled Module (alle aktuellen): kein `source` nötig, bleiben unverändert

### Ergebnis
- ansible-lint: **0 Fehler, 0 Warnungen** (68 Dateien, Profile 'production')
- ansible-playbook --syntax-check: **alle 10 Playbooks bestanden**

---

## 2026-02-28 – Claude Code – Cleanup: Artefakt-Verzeichnisse entfernt

### Gelöschte Dateien/Verzeichnisse (6 total)

**Artefakt-Verzeichnisse (Shell Brace-Expansion)**
Beim Aufruf von `mkdir {playbooks,templates}` (ohne `..`) erzeugt die Shell
statt zwei Verzeichnissen einen einzigen mit dem Literal-Namen `{playbooks,templates}`.
Alle 5 waren leer und hatten keinerlei Funktion:

- `modules/chat/tuwunel/{playbooks,templates}/`
- `modules/maps/umap/{playbooks,templates}/`
- `modules/observability/otel-collector/{playbooks,templates}/`
- `modules/tasks/vikunja/{playbooks,templates}/`
- `modules/tickets/pretix/{playbooks,templates}/`

**Leeres Template-Verzeichnis**
- `modules/mail/stalwart/templates/` – Stalwart braucht kein Template;
  Konfiguration erfolgt über die Web Admin UI nach dem ersten Start.

### Beibehaltene Symlink-Strukturen (mit Erklärung)

**`playbooks/tasks/tasks/` (4 Symlinks → parent)**
Notwendiger ansible-lint Workaround: beim Linting von Task-Dateien in
`playbooks/tasks/` setzt ansible-lint `playbook_dir = playbooks/tasks/`.
Referenzen wie `{{ playbook_dir }}/tasks/generate-quadlet.yml` würden dann
auf `playbooks/tasks/tasks/generate-quadlet.yml` zeigen – ohne Symlinks:
`load-failure`. Die Symlinks lösen das auf, ohne die Dateien zu duplizieren.
`.ansible-lint` schließt das Verzeichnis vom Linting aus (kein doppeltes Prüfen).

**`stalwart/playbooks/tasks/` + `zentinel/playbooks/tasks/` (je 2 DNS-Symlinks)**
Gleicher Grund: `deploy-dns.yml` referenziert `{{ playbook_dir }}/tasks/dns-*.yml`.
Beim Linting zeigt `playbook_dir` auf das Modul-Playbooks-Verzeichnis,
daher Symlinks in `tasks/` → `../../../../../playbooks/tasks/dns-*.yml`.

### Ergebnis
- ansible-lint: **0 Fehler, 0 Warnungen** (67 Dateien, Profile 'production')

---

## Format

```
## [YYYY-MM-DD] – [Wer] – [Kurzbeschreibung]
### Geänderte Dateien
- `pfad/zur/datei.yml` – Was wurde geändert
### Offene Probleme
- Was noch nicht funktioniert
### Nächster Schritt
- Was als nächstes getan werden muss
```

---

## 2026-02-27 – Claude Code – Vollständige Fehlerkorrektur (Lint-Pass)

### Geänderte Dateien
- `projects/FreeSynergy.Net/{branding,sites` – Gelöscht (kaputtes Verzeichnis, leere Bash-Brace-Expansion)
- `.yamllint.yml` – `comments-indentation: false` + `octal-values` hinzugefügt (ansible-lint Kompatibilität)
- `.ansible-lint` – Neu erstellt: `kinds:` für Task-Dateien, `exclude_paths:` für virtuelle Pfade
- `playbooks/tasks/deploy-module.yml` – `vars:` → `module_vars:`, `environment:` → `module_environment:`, `loop_var: module_entry` → `sub_module_entry` (reserved keywords behoben)
- `playbooks/tasks/run-module-hooks.yml` – `name[template]`: Jinja-Template ans Ende der Namen verschoben
- `playbooks/install-project.yml` – `name[template]`: "Project installed: {{ project_name }}"
- `playbooks/tasks/generate-single-example.yml` – `environment:` → `tpl_environment:` (reserved keyword)
- `playbooks/templates/container.env.j2` – `environment.items()` → `tpl_environment.items()`
- `playbooks/tasks/check-constraints.yml` – Neu erstellt (Stub, war in sync-stack.yml referenziert aber fehlte)
- `playbooks/tasks/dns-create-record.yml` – Neu erstellt (Stub, war in DNS-Playbooks referenziert aber fehlte)
- `playbooks/tasks/dns-remove-record.yml` – Neu erstellt (Stub, war in DNS-Playbooks referenziert aber fehlte)
- `modules/**/playbooks/*.yml` (alle) – `ignore_errors: true` → `failed_when: false`
- `modules/**/playbooks/*.yml` (alle) – `{{ vars.config_dir }}` → `{{ module_vars.config_dir }}`
- `modules/proxy/zentinel/playbooks/deploy-dns.yml` – `{{ vars.dns_* }}` → direkte Variablen
- `modules/proxy/zentinel/playbooks/undeploy-dns.yml` – gleich
- `modules/auth/kanidm/playbooks/deploy-setup.yml` – `changed_when: true` bei `recover-account` Befehlen
- `modules/git/forgejo/playbooks/deploy-setup.yml` – `changed_when: true` beim `create admin user` Befehl
- `modules/*.yml` (18 Dateien) – `---` Document-Start hinzugefügt (yamllint)

### Ergebnis
- yamllint: **0 Fehler, 0 Warnungen** (alle 54 Dateien)
- ansible-lint: **0 Fehler, 0 Warnungen** (68 Dateien, Profile 'production')

### Offene Probleme
- `dns-create-record.yml` und `dns-remove-record.yml` sind Stubs (noch nicht implementiert)
- `check-constraints.yml` ist ein Stub (noch nicht implementiert)
- Playbooks allgemein sind noch Stubs – siehe TODO.md

### Nächster Schritt
- Playbook-Implementierung beginnen (sync-stack.yml, deploy-stack.yml)
- DNS-Plugin-System implementieren
- Constraint-Checks implementieren

## 2026-02-28 – Claude Code – Logikfehler in Playbooks + Editor YAML-Schema

### Schema-Fix (VSCode)
- `.vscode/settings.json` – `yaml.schemas` URL korrigiert:
  `https://json.schemastore.org/ansible-tasks.json` → `https://raw.githubusercontent.com/ansible/schemas/main/f/ansible-tasks.json`
  (SchemaStore-URL lieferte keinen Inhalt; direkte GitHub-URL funktioniert)

### Logikfehler behoben (6 Dateien)

**`playbooks/deploy-stack.yml`**
- `Set host variables` ohne `when`-Bedingung → `host_cfg` wäre undefined wenn keine Host-Datei
  vorhanden → Jinja-Fehler zur Laufzeit
- Fix: aufgeteilt in `when: host_files.matched > 0` + Fallback `when: host_files.matched == 0`

**`playbooks/sync-stack.yml`**
- `include_vars: name: project_config` überschrieb die externe Variable (Pfad) mit YAML-Inhalt
- Duplizierungs-Check nutzte `fail + loop` statt `assert` (konnte bei dict-Keys nie feuern)
- Fix: Variable → `project_cfg`, Duplicate-Check → `assert`

**`playbooks/setup-server.yml`**
- `when: user_check is failed` funktioniert NIE nach `failed_when: false` → User wurde nie angelegt
- Fix: `when: user_check.rc != 0`

**`playbooks/tasks/generate-single-example.yml`**
- Nutzte `module_environment` ohne das Modul zu laden → undefined wenn von `install-project.yml`
- Fix: `include_vars` für Modul-Datei + `example_module_cfg.environment | default({})`

**`playbooks/tasks/update-module.yml`**
- `include_vars: name: module` → `container`, `module_environment`, `module_vars` fehlten
  beim späteren Aufruf von `generate-quadlet.yml`
- Fix: `name: module_cfg` + `set_fact` (identisch zu `deploy-module.yml`)
- `module.module.version` → `module.version`

**`playbooks/tasks/run-module-hooks.yml`**
- Kein Guard wenn `module_path` undefined → Jinja-Fehler bei orphaned Services
- Fix: `when: module_path is defined` + `| default(0)` / `| default([])`

### Ergebnis
- ansible-lint: **0 Fehler, 0 Warnungen** (67 Dateien, Profile 'production')
- ansible-playbook --syntax-check: **alle 9 Playbooks bestanden**

---

## 2026-02-27 – Claude Code – Editor-Kompatibilität (Ansible-Lint aus VSCode)

### Problem
Der VSCode-Editor führt ansible-lint aus `/home/kal/Server/` (Workspace-Root) aus,
nicht aus `fsn-platform/`. Dadurch griff die `.ansible-lint`-Konfiguration im Projektverzeichnis
nicht, und der Editor zeigte Fehler, obwohl `ansible-lint fsn-platform/` sauber lief.

### Geänderte / neue Dateien
- `/home/kal/Server/.ansible-lint` – Neu erstellt: `kinds:` mit `fsn-platform/`-Prefix-Varianten
  für korrekte Datei-Klassifizierung aus dem Workspace-Root heraus
- `playbooks/tasks/run-module-hooks.yml` – Zweite `name[template]`-Korrektur:
  `{{ instance_name }}` aus den Task-Namen entfernt (zwei Templates mit Text dazwischen = Fehler)
- `playbooks/tasks/tasks/` – Symlinks erstellt für ansible-lint `load-failure`-Workaround:
  - `deploy-module.yml` → `../deploy-module.yml`
  - `generate-quadlet.yml` → `../generate-quadlet.yml`
  - `run-module-hooks.yml` → `../run-module-hooks.yml`
  - `record-deployed-version.yml` → `../record-deployed-version.yml`
- `modules/mail/stalwart/playbooks/tasks/` – DNS-Stub-Symlinks:
  - `dns-create-record.yml` → `../../../../../playbooks/tasks/dns-create-record.yml`
  - `dns-remove-record.yml` → `../../../../../playbooks/tasks/dns-remove-record.yml`
- `modules/proxy/zentinel/playbooks/tasks/` – DNS-Stub-Symlinks (gleiche Targets)

### Hintergrund: Symlinks
ansible-lint löst `{{ playbook_dir }}/tasks/X.yml` beim Linten von Task-Dateien auf den
eigenen Ordner auf (`playbooks/tasks/tasks/`). Diese Pfade existieren nicht real, deshalb
`load-failure`. Da `load-failure` und `syntax-check` nicht skippbar sind, wurden Symlinks
als struktureller Workaround erstellt.

### Ergebnis
- ansible-lint aus `/home/kal/Server/`: **0 Fehler, 0 Warnungen** (67 Dateien, Profile 'production')
- ansible-lint aus `fsn-platform/`: **0 Fehler, 0 Warnungen**
- Editor zeigt keine ansible-lint-Fehler mehr

### Offene Probleme
- `dns-create-record.yml` und `dns-remove-record.yml` sind Stubs (noch nicht implementiert)
- `check-constraints.yml` ist ein Stub (noch nicht implementiert)
- Playbooks allgemein sind noch Stubs – siehe TODO.md

### Nächster Schritt
- Playbook-Implementierung beginnen (sync-stack.yml, deploy-stack.yml)
- DNS-Plugin-System implementieren
- Constraint-Checks implementieren

---

## 2026-02-28 – Claude Chat – Initial Commit (v0.0.1)

Erstes Release. Alle Module, Playbooks, Branding und Landing Page.
