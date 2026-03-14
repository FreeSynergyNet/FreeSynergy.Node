# FreeSynergy — Master TODO

Stand: 2026-03

---

## Phase 0 — Aufräumen (noch offen)

- [ ] `FreeSynergy/Desktop` Repo erstellen + CI einrichten
- [ ] `CLAUDE.md` in Node aktualisieren: Modul-Pfad `.toml` statt `.yml` (CLAUDE.md sagt noch `.yml`)
- [ ] `REFACTORING-PLAN-v3-FINAL.md` entfernen oder in `docs/ARCHITECTURE.md` umschreiben
- [ ] `fsn-wizard` Crate in Node anlegen (fehlt noch komplett, steht im Plan)

---

## FreeSynergy.Lib — Was fehlt

### Phase 3: Store + Plugins (Stubs)

- [ ] `fsn-store`: StoreClient implementieren (Download, Registry, Suche, Cache)
  - [ ] `catalog.toml` fetchen + parsen
  - [ ] Offline-Fallback (`index.toml` aus Binary)
  - [ ] Modulverzeichnis lokal cachen (`~/.local/share/fsn/store/`)
  - [ ] Retry + Backoff (Netzwerk offline → kein Crash)
- [ ] `fsn-plugin-sdk`: Echte wit-bindgen Interfaces definieren (`.wit` Dateien)
- [ ] `fsn-plugin-runtime`: wasmtime Host implementieren
  - [ ] WASM-Modul laden + ausführen
  - [ ] Sandboxing (WASI, Capabilities begrenzen)
  - [ ] Plugin-Protokoll: JSON-RPC über stdio (Fallback für nicht-WASM Plugins)

### Phase 4: Auth + Federation (Stubs)

- [ ] `fsn-auth`: JWT-Parsing + Validierung implementieren
  - [ ] RBAC Permission-Check (is_allowed)
  - [ ] Claims extrahieren aus Token
- [ ] `fsn-federation`: OIDC-Client implementieren
  - [ ] SCIM-User-Sync
  - [ ] ActivityPub: Follow/Unfollow, Note, Actor
  - [ ] WebFinger
- [ ] `fsn-crypto`: age-Encryption implementieren
  - [ ] Passphrase-basiert (vault.toml)
  - [ ] Public-Key (mTLS)

### Phase 5: Container + Templates (Stubs)

- [ ] `fsn-container`: PodmanClient via bollard implementieren
  - [ ] Container listen, starten, stoppen, logs streamen
  - [ ] Image pull
  - [ ] Volume-Management
- [ ] `fsn-container`: SystemdManager implementieren (ist noch Stub in Lib, genutzt wird fsn-container aus Node)
  - Achtung: `fsn-container` in Node.cli hat eine eigene Implementierung — prüfen ob Lib-Version die ablösen kann
- [ ] `fsn-template`: Tera-Wrapper implementieren (ist noch Stub, Node-Wrapper existiert schon)

### Phase 6: DB + Sync

- [ ] `fsn-db`: SeaORM Entities implementieren
  - [ ] `resource`, `permission`, `sync_state`, `plugin`, `audit_log`
  - [ ] Migration-System (sea-orm-cli)
  - [ ] WriteBuffer flush-loop

### Allgemein (Lib)

- [ ] Feature Flags überall einführen (`sqlite`, `postgres`, `sync`, `federation`, `wasm-plugins`)
- [ ] README.md pro Crate (alle fehlen)
- [ ] `#[doc]` auf allen `pub` Items
- [ ] `examples/` Verzeichnis in jeder Library

---

## FreeSynergy.Node — Was fehlt

### fsn-host (SSH / Remote-Deploy)

- [ ] SSH-Session via `russh` implementieren
- [ ] Remote-Befehlsausführung (shell commands über SSH)
- [ ] Dateitransfer (Quadlet-Dateien remote schreiben)
- [ ] Remote systemd steuern (daemon-reload, start, stop via SSH)
- [ ] `DeployOpts` um SSH-Target erweitern
- [ ] `fsn deploy --host <name>` — Remote-Deploy Pfad

### fsn-cli

- [ ] `fsn serve` implementieren (Web-UI, oder klar als "Öffnet Desktop" umschreiben)
- [ ] `fsn tui` — entweder `fsd-conductor` starten (per `which fsd`) oder entfernen
- [ ] `fsn store search/install/update` Subcommands hinzufügen (wenn fsn-store fertig)
- [ ] `fsn conductor start/stop/logs` Subcommands hinzufügen
- [ ] `generate_secret()` in `init.rs` → `/dev/urandom` oder `rand` crate (aktuell nano-Zeit PRNG, unsicher)

### fsn-wizard (fehlt komplett)

- [ ] Crate `fsn-wizard` anlegen
- [ ] Docker-Compose / YAML → FSN-Modul-TOML Konverter
- [ ] Typ-Erkennung aus Image-Name + Ports + Volumes
- [ ] Setup-Fields generieren aus Modul-Metadaten

### Store-Module

- [ ] `[module.roles]` Block in allen Modulen ergänzen (`provides`, `requires`)
  - Betrifft: zentinel, kanidm, stalwart, forgejo, outline, tuwunel, vikunja, cryptpad, etc.
- [ ] `[module.ui]` Block ergänzen (`supports_web`, `open_mode`, `web_url_template`)
- [ ] Zentinel als echtes WASM-Plugin fertigstellen (aktuell built-in KDL-Generator als Fallback)

---

## FreeSynergy.Desktop — Alles fehlt (neues Repo)

- [ ] Repo `FreeSynergy/Desktop` erstellen
- [ ] Cargo Workspace mit Dioxus 0.7
- [ ] CI/CD (build, clippy, rustfmt)

### fsd-shell (Taskbar + Window Manager)

- [ ] Taskbar-Komponente (App-Icons, System-Tray, Uhr)
- [ ] App-Launcher (Grid + Suche)
- [ ] Window-Manager (Fenster öffnen, schließen, verschieben)
- [ ] Wallpaper-Anzeige
- [ ] Notification-System (Toast)

### fsd-conductor (Container Management)

- [ ] Service-Liste (alle laufenden FSN-Services)
- [ ] Start / Stop / Restart pro Service
- [ ] Log-Viewer (live streaming)
- [ ] Ressourcen-Anzeige (CPU, RAM, Volumes)
- [ ] Health-Status (✓/⚠/✗)
- [ ] Abhängigkeits-Graph visualisieren
- [ ] Bot-Management (wenn fsn-auth fertig)

### fsd-store (Package Manager)

- [ ] Modul-Browser (Katalog aus fsn-store)
- [ ] Suche + Filter
- [ ] Install-Wizard (Setup-Fields aus Modul-Metadaten)
- [ ] Update-Check + Update durchführen
- [ ] Modul entfernen

### fsd-studio (Plugin/Modul-Ersteller)

- [ ] Docker-Compose → FSN-Modul Konverter (UI für fsn-wizard)
- [ ] WASM-Plugin Template-Generator (wit-bindgen)
- [ ] i18n-Editor (Sprachdateien visuell bearbeiten)
- [ ] AI-Erweiterung (optional): Natürlichsprache → Modul-Metadaten

### fsd-settings

- [ ] Appearance: Theme wählen, Wallpaper, CSS
- [ ] Language: Sprache wählen, Sprachdateien aus Store laden
- [ ] Service Roles: welcher Container für welche Funktion
- [ ] Accounts: OIDC-Accounts verwalten
- [ ] Desktop: Taskbar-Position, Autostart

### fsd-profile

- [ ] User-Profil anzeigen + bearbeiten
- [ ] OIDC-Verbindungen
- [ ] SSH-Keys verwalten

### fsd-app (Einstiegspunkt)

- [ ] Shell laden + alle Apps registrieren
- [ ] Dioxus Multiwindow Setup

---

## i18n / Translations

- [ ] `.ftl`-Dateien (Fluent-Format) anlegen für alle i18n-Schlüssel
  - Aktuell: Store hat `.toml` Dateien, Code erwartet Fluent-Format
  - Entweder: `.toml`-Format in fsn-i18n nativ supporten ODER alle Store-i18n zu `.ftl` migrieren
- [ ] Schnipsel-Kategorien anlegen: `actions.ftl`, `nouns.ftl`, `status.ftl`, `errors.ftl`, `phrases.ftl`, `time.ftl`, `validation.ftl`, `help.ftl`
- [ ] Deutsche Übersetzungen (de) für alle Keys
- [ ] Englische Basis-Keys (en) als Fallback
- [ ] Hardcoded Strings aus `fsn-cli` Commands in i18n-Keys migrieren
  - `println!("=== FreeSynergy.Node Setup Wizard ===")` etc. in `init.rs`
  - Status-Ausgaben in `status.rs`, `sync.rs`
  - Fehlermeldungen in allen Commands
- [ ] Store-i18n: 51 Sprachen sind angelegt aber Inhalte prüfen (sind alle vollständig?)

---

## Theme-System

- [ ] `ThemeEngine::from_css()` implementieren (CSS Custom Properties → Theme)
- [ ] `ThemeEngine::to_tailwind_config()` implementieren
- [ ] Mehrere Themes unterstützen (Wechsel per Settings)
- [ ] Theme aus Store laden (Store-Themes)
- [ ] FreeSynergy-Default-Theme finalisieren (Cyan + White)

---

## Settings-System

- [ ] Settings-Datenstruktur in fsn-core (AppSettings TOML)
- [ ] Settings laden/speichern (`~/.config/fsn/settings.toml`)
- [ ] Service Roles System implementieren (welcher Container = welcher Handler)
- [ ] `[module.roles]` aus allen Modulen lesen + in Service-Role-Registry eintragen

---

## Tests / CI

### Unit Tests (fehlen fast überall)

- [ ] `fsn-config`: Lade/Speicher-Tests, Auto-Repair Tests
- [ ] `fsn-health`: Health-Check Tests mit Mock-Configs
- [ ] `fsn-deploy`: Quadlet-Generation Tests
- [ ] `fsn-deploy`: Diff-Berechnung Tests
- [ ] `fsn-deploy`: KDL-Generator Tests
- [ ] `fsn-core`: Config-Parser Tests (ProjectConfig, HostConfig)
- [ ] `fsn-dns`: DNS-Provider Tests (Mock-HTTP)

### Integration Tests

- [ ] Vollständiger Deploy-Lifecycle Test (generate → write → systemd mock)
- [ ] `fsn init` Wizard Test (stdin mock)
- [ ] Store-Modul-Parsing Tests (alle 21 Module parsen)

### CI/CD (GitHub Actions)

- [ ] Build + Clippy + Rustfmt auf jedem Push (Lib + Node)
- [ ] `cargo-deny` für License + Advisory Check
- [ ] Dependabot einrichten
- [ ] Nightly Fuzzing: `fsn-config`, `fsn-template` (alles was User-Input parst)

---

## Abhängigkeiten / Bibliotheken

### In FreeSynergy.Lib geladen aber noch nicht genutzt

- [ ] `fsn-federation`: `activitypub_federation` (0.6) — komplett Stub
- [ ] `fsn-federation`: `openidconnect` — komplett Stub
- [ ] `fsn-auth`: `jsonwebtoken` — Stub
- [ ] `fsn-bridge-sdk`: komplett leer
- [ ] `fsn-plugin-runtime`: `wasmtime` geladen, aber kein Host implementiert
- [ ] `fsn-plugin-sdk`: `wit-bindgen` — keine `.wit` Interfaces definiert
- [ ] `fsn-db`: `sea-orm` + `automerge` (für sync_state) — Entities definiert, aber kein echter Betrieb

### Zu ergänzen (fehlen noch, stehen im Plan)

- [ ] `russh` — für fsn-host SSH-Implementierung
- [ ] `tokio-tungstenite` — WebSocket (für Desktop Live-Updates)
- [ ] `tonic` — gRPC (für Inter-Process Communication)
- [ ] `schemars` — JSON-Schema Generation aus Rust-Structs
- [ ] `rstest` + `insta` — bessere Test-Infrastruktur
- [ ] `testcontainers` — Integration Tests mit echten Containern
- [ ] `cargo-fuzz` — Fuzzing Setup
- [ ] `backon` — Retry-Backoff (für Store-HTTP-Calls)
- [ ] `rand` — kryptographisch sicherer RNG (statt nano-Zeit PRNG in init.rs)

---

## Dokumentation

- [ ] `docs/ARCHITECTURE.md` pro Repo (Lib, Node, Desktop)
- [ ] README.md für alle fsn-* Crates
- [ ] `migration/` Verzeichnis mit Skripten für v1 → v2 Config-Migration
- [ ] JSON-Schema für alle Modul-Manifeste (für Validierung + UI-Generierung)

---

## Sonstiges / Kleinigkeiten

- [ ] `REFACTORING-PLAN-v3-FINAL.md` → nach `docs/ARCHITECTURE.md` refactoren und umbenennen
- [ ] `TODO.md` (diese Datei) im TODO-Abschnitt unten nicht als done markieren solange offen
- [ ] `fsn tui` Command: entweder `fsd-conductor` per `std::process::Command` starten oder Command entfernen
- [ ] Audit-Log Infrastruktur (AuditEntry → per CRDT synchronisiert)
- [ ] Offline-First: Store-Katalog-Cache-Strategie festlegen
- [ ] Graceful Degradation: Plugin lädt nicht → Rest funktioniert
- [ ] Error-Recovery für Netzwerk: RetryPolicy in fsn-store

---

## Reihenfolge (empfohlen)

1. **fsn-store** implementieren (Node braucht das für `fsn install`)
2. **fsn-container** in Lib fertigstellen + Node-Implementierung ablösen
3. **fsn-host** SSH (Remote-Deploy)
4. **fsn-crypto** age-Encryption (vault.toml braucht das)
5. **i18n** `.ftl`-Migration
6. **FreeSynergy.Desktop** Repo anlegen + fsd-conductor als erstes
7. **fsn-plugin-runtime** WASM Host (für Zentinel als echtes Plugin)
8. **fsn-auth + fsn-federation** (für Bot-Management + OIDC)
