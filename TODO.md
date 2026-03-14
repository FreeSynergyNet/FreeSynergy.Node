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

- [x] SSH-Session via `russh` implementieren
- [x] Remote-Befehlsausführung (shell commands über SSH)
- [x] Dateitransfer (Quadlet-Dateien remote schreiben)
- [x] Remote systemd steuern (daemon-reload, start, stop via SSH)
- [x] `DeployOpts` um SSH-Target erweitern
- [x] `fsn deploy --host <name>` — Remote-Deploy Pfad

### fsn-cli

- [x] `fsn serve` — redirected zu fsd mit Hinweis
- [x] `fsn tui` — startet fsd-conductor → fsd (Fallback-Kette)
- [x] `fsn store search/info/install/update` Subcommands (StoreClient verdrahtet)
- [x] `fsn conductor list/start/stop/restart/logs` Subcommands (PodmanClient verdrahtet)
- [x] `generate_secret()` nutzt rand crate (bereits sicher)

### fsn-wizard (fehlt komplett)

- [x] Crate `fsn-wizard` anlegen
- [x] Docker-Compose / YAML → FSN-Modul-TOML Konverter
- [x] Typ-Erkennung aus Image-Name + Ports + Volumes
- [x] Setup-Fields generieren aus Modul-Metadaten

### Store-Module

- [x] `[module.roles]` Block in allen Modulen ergänzen (`provides`, `requires`)
- [x] `[module.ui]` Block ergänzen (`supports_web`, `open_mode`, `web_url_template`)
- [x] Zentinel als echtes Rust-Plugin fertigstellen (Python → Rust; WASM-Kompilierung via `cargo build --target wasm32-wasip1` wenn Toolchain vorhanden)

---

## FreeSynergy.Desktop

- [x] Repo `FreeSynergy/Desktop` erstellen
- [x] Cargo Workspace mit Dioxus 0.6
- [x] CI/CD (build, clippy, rustfmt) — FreeSynergy.Node

### fsd-shell (Taskbar + Window Manager)

- [x] Taskbar-Komponente (App-Icons, System-Tray, Uhr — live via chrono)
- [x] App-Launcher (Grid + Suche, Vollbild-Overlay)
- [x] Window-Manager (Fenster öffnen, schließen, verschieben, z-index)
- [x] Wallpaper-Anzeige (Color, URL, File, Default)
- [x] Notification-System (Toast, 4 Severity-Level)
- [x] WindowFrame: Minimize/Maximize verdrahten
- [x] AppRegistry: App-spezifischen Content in WindowFrame injizieren

### fsd-conductor (Container Management)

- [x] Service-Liste (Podman-Integration, 5s polling)
- [x] Log-Viewer (Podman-Integration, 3s polling, Clear + Follow)
- [x] Start / Stop / Restart pro Service (fsn-container verdrahten)
- [x] Ressourcen-Anzeige (CPU, RAM, Volumes)
- [x] Health-Status (✓/⚠/✗) live
- [x] Abhängigkeits-Graph visualisieren
- [ ] Bot-Management (wenn fsn-auth fertig)

### fsd-store (Package Manager)

- [x] Modul-Browser (Stub)
- [x] Suche + Filter (Stub)
- [x] Install-Wizard (Stub, Schritt-Indikator)
- [x] Echten Katalog aus fsn-store laden
- [x] Update-Check + Update durchführen
- [x] Modul entfernen

### fsd-studio (Plugin/Modul-Ersteller)

- [x] Docker-Compose → FSN-Modul Konverter (Stub)
- [x] WASM-Plugin Template-Generator (Stub)
- [x] i18n-Editor (Stub, Kategorie-Navigation)
- [x] Konverter: echte YAML-Parsing-Logik (fsn-wizard verdrahten)
- [x] AI-Erweiterung (optional): Natürlichsprache → Modul-Metadaten

### fsd-settings

- [x] Appearance: Theme, Wallpaper, CSS (Stub)
- [x] Language: Sprache wählen (Stub)
- [x] Desktop: Taskbar-Position, Autostart (Stub)
- [x] Service Roles: welcher Container für welche Funktion (Logik fehlt)
- [ ] Accounts: OIDC-Accounts verwalten

### fsd-profile

- [x] User-Profil (Stub)
- [x] User-Profil anzeigen + bearbeiten
- [ ] OIDC-Verbindungen
- [x] SSH-Keys verwalten

### fsd-app (Einstiegspunkt)

- [x] Shell laden (fsd-app startet Desktop)
- [x] AppRegistry: alle Apps registrieren + Content in WindowFrame injizieren
- [x] Dioxus Multiwindow Setup

---

## i18n / Translations

- [x] `.toml`-Format in fsn-i18n nativ supporten (`add_toml_str`, `toml_maps`, TOML-Fallback in t/t_with)
- [x] `init_with_toml_strs()` globale Funktion in fsn-i18n
- [x] Schnipsel-Kategorien: `wizard`, `status`, `sync` in `cli/crates/fsn-cli/locales/`
- [x] Deutsche Übersetzungen (de) für CLI-Keys — `locales/de/cli.toml`
- [x] Englische Basis-Keys (en) — `locales/en/cli.toml`, im Binary gebündelt via `include_str!()`
- [x] Hardcoded Strings migriert: `init.rs`, `status.rs`, `sync.rs`
- [x] Language detection aus `LANGUAGE`/`LANG`/`LC_ALL` env vars in `main.rs`
- [ ] Store-i18n: 51 Sprachen prüfen (Inhalte vollständig?) (deferred)
- [ ] Fehlermeldungen in allen Commands (deferred)

---

## Theme-System

- [x] `ThemeEngine::from_css()` implementieren (CSS Custom Properties → Theme)
- [x] `ThemeEngine::to_tailwind_config()` implementieren (valides JSON)
- [x] Mehrere Themes unterstützen — `ThemeRegistry` (register, set_active, names, remove)
- [x] Theme aus Store laden — `ThemeEngine::from_toml_str()` + `ThemeRegistry::register_toml_str()`
- [x] FreeSynergy-Default-Theme finalisiert (Cyan #00BCD4 + White #e6edf3 auf Dark Navy)

---

## Settings-System

- [x] Settings-Datenstruktur in fsn-core (AppSettings TOML)
- [x] Settings laden/speichern (`~/.config/fsn/settings.toml`)
- [x] Service Roles System implementieren (welcher Container = welcher Handler)
- [x] `[module.roles]` aus allen Modulen lesen + in Service-Role-Registry eintragen

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

- [x] Vollständiger Deploy-Lifecycle Test (generate → write → systemd mock)
- [x] `fsn init` Wizard Test (stdin mock)
- [x] Store-Modul-Parsing Tests (alle 21 Module parsen)

### CI/CD (GitHub Actions)

- [x] Build + Clippy + Rustfmt auf jedem Push (Lib + Node)
- [x] `cargo-deny` für License + Advisory Check
- [x] Dependabot einrichten
- [x] Nightly Fuzzing: `fsn-config`, `fsn-template` (alles was User-Input parst)

---

## Abhängigkeiten / Bibliotheken

### In FreeSynergy.Lib geladen aber noch nicht genutzt

- [x] `fsn-federation`: `activitypub_federation` — OIDC/SCIM/WebFinger/ActivityPub vollständig implementiert
- [x] `fsn-federation`: `openidconnect` — OidcClient (discover + userinfo) fertig
- [x] `fsn-auth`: `jsonwebtoken` — JwtSigner + JwtValidator (HMAC/RSA) + PermissionSet fertig
- [ ] `fsn-bridge-sdk`: komplett leer (noch kein Bedarf) (deferred)
- [x] `fsn-plugin-runtime`: `wasmtime` Host vollständig implementiert (WASI sandbox + ProcessPluginRunner)
- [x] `fsn-plugin-sdk`: PluginContext/PluginManifest/PluginResponse definiert, deploy engine verdrahtet
- [ ] `fsn-db`: `sea-orm` + `automerge` — Entities definiert, aber kein echter Betrieb

### Zu ergänzen (fehlen noch, stehen im Plan)

- [x] `russh` — fsn-host SSH vollständig implementiert
- [x] `tokio-tungstenite` — WebSocket (für Desktop Live-Updates)
- [x] `tonic` — gRPC (für Inter-Process Communication)
- [x] `schemars` — JSON-Schema Generation aus Rust-Structs
- [x] `rstest` + `insta` — bessere Test-Infrastruktur
- [x] `testcontainers` — Integration Tests mit echten Containern
- [x] `cargo-fuzz` — Fuzzing Setup (`cli/fuzz/`, Targets: fuzz_config, fuzz_template)
- [x] `backon` — Retry-Backoff (für Store-HTTP-Calls)
- [x] `rand` — kryptographisch sicherer RNG (statt nano-Zeit PRNG in init.rs)

---

## Dokumentation

- [x] `docs/ARCHITECTURE.md` pro Repo (Lib, Node, Desktop)
- [x] README.md für alle fsn-* Crates
- [ ] `migration/` Verzeichnis mit Skripten für v1 → v2 Config-Migration (deferred)
- [ ] JSON-Schema für alle Modul-Manifeste (für Validierung + UI-Generierung) (deferred)

---

## Sonstiges / Kleinigkeiten

- [x] `REFACTORING-PLAN-v3-FINAL.md` → nach `docs/ARCHITECTURE.md` refactoren und umbenennen
- [x] `TODO.md` (diese Datei) im TODO-Abschnitt unten nicht als done markieren solange offen
- [x] `fsn tui` Command: `fsd-conductor` per `std::process::Command` starten (fsd Fallback)
- [x] Audit-Log Infrastruktur (`AuditEntry` + `AuditLog` in fsn-core; CRDT-Sync: Phase 2)
- [x] Offline-First: Store-Katalog-Cache-Strategie (`fetch_all()` → bundled Fallback)
- [x] Graceful Degradation: Plugin lädt nicht → warn + Rest läuft weiter
- [x] Error-Recovery für Netzwerk: RetryPolicy in fsn-store (backon, 3 retries, exp. backoff)

---

## Reihenfolge (empfohlen)

1. ~~**fsn-store** implementieren~~ ✓ fertig — Node nutzt jetzt fsn-store (Lib)
2. ~~**fsn-container** in Lib fertigstellen + Node-Implementierung ablösen~~ ✓ fertig — fsn-podman entfernt, Lib-Version aktiv
3. ~~**fsn-host** SSH (Remote-Deploy)~~ ✓ fertig — russh, exec, write_file, remote systemd, `fsn deploy --host <name>`
4. ~~**fsn-crypto** age-Encryption~~ ✓ fertig — vault.age mit passphrase KDF, VaultConfig in fsn-core
5. ~~**FreeSynergy.Desktop** Repo anlegen + fsd-conductor als erstes~~ ✓ fertig — fsd-shell, fsd-conductor, fsd-app compilierbar (braucht libxdo-devel)
6. **i18n** `.ftl`-Migration
7. ~~**fsn-plugin-runtime** WASM Host~~ ✓ fertig — wasmtime Host + WASI sandbox + ProcessPluginRunner + deploy engine verdrahtet
8. ~~**fsn-auth + fsn-federation**~~ ✓ fertig — JWT (HMAC/RSA), RBAC, OIDC, SCIM, WebFinger vollständig
9. **fsn-db** echter Betrieb (sea-orm Migrations laufen lassen)
