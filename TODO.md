# FreeSynergy — Master TODO

Stand: 2026-03

---

## Phase 0 — Aufräumen (erledigt ✓)

- [x] `FreeSynergy/Desktop` Repo erstellen + CI einrichten
- [x] `CLAUDE.md` in Node aktualisieren: Modul-Pfad `.toml` statt `.yml`
- [x] `REFACTORING-PLAN-v3-FINAL.md` entfernen oder in `docs/ARCHITECTURE.md` umschreiben
- [x] `fsn-wizard` Crate in Node anlegen

---

## FreeSynergy.Lib — Was fehlt

### Phase 3: Store + Plugins (erledigt ✓)

- [x] `fsn-store`: StoreClient implementiert (HTTP + Local, CatalogCache, Retry via Timeout)
  - [x] Retry + Backoff bei Netzwerkfehler
  - [x] Offline-Fallback: bei HTTP-Fehler auf lokalen Cache zurückfallen
- [x] `fsn-plugin-sdk`: wit-bindgen Interfaces definiert (`.wit` Dateien)
- [x] `fsn-plugin-runtime`: wasmtime Host implementiert
  - [x] WASM-Modul laden + ausführen
  - [x] Sandboxing (WASI, Capabilities begrenzen)
  - [x] Plugin-Protokoll: JSON-RPC über stdio (Fallback für nicht-WASM Plugins)

### Phase 4: Auth + Federation (erledigt ✓)

- [x] `fsn-auth`: JWT-Parsing + Validierung implementiert
  - [x] RBAC Permission-Check (`AccessControl` trait, `is_allowed`)
  - [x] Claims extrahieren aus Token (`Claims::new`, `JwtValidator::validate`)
- [x] `fsn-federation`: OIDC/SCIM/ActivityPub/WebFinger implementiert
  - [x] OIDC: discovery + userinfo (`OidcClient`)
  - [x] SCIM-User-Sync (`ScimClient`: create/get/list users + groups)
  - [x] ActivityPub: Actor-Typen + `FsyFederationConfig` builder
  - [x] WebFinger (RFC 7033): `WebFingerClient::lookup` + `lookup_acct`
- [x] `fsn-crypto`: age-Encryption implementiert
  - [x] Passphrase-basiert: `AgePassphraseEncryptor` / `AgePassphraseDecryptor`
  - [x] Public-Key X25519: `AgeEncryptor` / `AgeDecryptor`
  - [x] mTLS: `CaBundle::generate`, `issue_server_cert`, `issue_client_cert` via rcgen

### Phase 5: Container + Templates (erledigt ✓)

- [x] `fsn-container`: PodmanClient + SystemdManager in Lib implementiert und aktiv (fsn-podman aus Node entfernt)
- [x] `fsn-template`: Tera-Wrapper implementiert (`TemplateEngine`, `TemplateContext`, Filter)

### Phase 6: DB + Sync (erledigt ✓)

- [x] `fsn-db`: SeaORM Entities implementiert
  - [x] `resource`, `permission`, `sync_state`, `plugin`, `audit_log`
  - [x] Migration-System (embedded SQL via `Migrator::run()`)
  - [x] WriteBuffer flush-loop (`run_auto_flush`)

### Allgemein (Lib)

- [x] Feature Flags überall einführen (`sqlite`/`postgres` in fsn-db, `jwt` in fsn-auth, `age`/`mtls`/`keygen` in fsn-crypto, `oidc`/`scim`/`activitypub`/`webfinger` in fsn-federation, `wasm` in fsn-plugin-runtime)
- [x] README.md pro Crate (alle 20 Crates)
- [x] `#[doc]` auf allen `pub` Items
- [ ] `examples/` Verzeichnis in jeder Library (deferred)

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

### Unit Tests

- [x] `fsn-config`: Lade/Speicher-Tests, Auto-Repair Tests
- [x] `fsn-health`: Health-Check Tests mit Mock-Configs
- [x] `fsn-deploy`: Quadlet-Generation Tests
- [x] `fsn-deploy`: Diff-Berechnung Tests
- [x] `fsn-deploy`: KDL-Generator Tests
- [x] `fsn-core`: Config-Parser Tests (ProjectConfig, HostConfig)
- [x] `fsn-core`: ServiceType Tests (from_class_prefix, exported_contract, ...)
- [x] `fsn-core`: Health-Check Tests (ProjectConfig, HostConfig, ServiceInstanceConfig)
- [x] `fsn-dns`: DNS-Provider Tests (MockDns, reconcile, NoopDns, Factory)

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

1. ~~**fsn-store** implementieren~~ ✓ fertig — Node nutzt jetzt fsn-store (Lib)
2. ~~**fsn-container** in Lib fertigstellen + Node-Implementierung ablösen~~ ✓ fertig — fsn-podman entfernt, Lib-Version aktiv
3. **fsn-host** SSH (Remote-Deploy)
4. **fsn-crypto** age-Encryption (vault.toml braucht das)
5. **i18n** `.ftl`-Migration
6. **FreeSynergy.Desktop** Repo anlegen + fsd-conductor als erstes
7. **fsn-plugin-runtime** WASM Host (für Zentinel als echtes Plugin)
8. **fsn-auth + fsn-federation** (für Bot-Management + OIDC)
