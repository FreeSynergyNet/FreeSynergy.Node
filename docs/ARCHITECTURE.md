# FreeSynergy — Architecture

---

## 1. Grundprinzipien

### 1.1 Code-Wiederverwendung

Jede Funktionalität wird zuerst als eigenständige Library gebaut. `fsn-*` Libraries in FreeSynergy.Lib wissen nichts von FreeSynergy.Node — sie sind für Wiki.rs, Decidim.rs und jeden anderen nutzbar.

### 1.2 Standards

- **WASM-First** für Plugins (wasmtime + wit-bindgen Component Model)
- **CRDT von Tag 1** (Automerge)
- **ActivityPub von Tag 1** (activitypub_federation)
- **Nur Tera** für Templates
- **Englisch** im Code und in Kommentaren
- **Deutsch** in der Kommunikation hier und in Claude Code

### 1.3 UX-Konsistenz

Desktop, Web und TUI müssen sich **gleich anfühlen**. Gleiche Navigation, gleiche Shortcuts, gleiche Fenster-Metapher.

### 1.4 Universelle Befehlsschnittstelle

Jeder Befehl ist über **alle drei Interfaces** nutzbar:
- **CLI**: `fsn conductor start <service>`
- **TUI**: `fsn conductor` (Dioxus terminal)
- **GUI**: `fsd-conductor` (Dioxus desktop/web)

Core-Logik liegt in der Library. Die drei Frontends sind dünne Wrapper.

---

## 2. Repository-Struktur

```
FreeSynergy/Lib            ← Wiederverwendbare Bibliotheken (Cargo Workspace)      [eigenes Repo]
FreeSynergy/Node           ← CLI + Deployment-Engine (Cargo Workspace)             [eigenes Repo]
FreeSynergy/Desktop        ← Desktop-Umgebung (Cargo Workspace, nutzt fsn-*)       [eigenes Repo]
FreeSynergy/Node.Store     ← Plugin-Registry für Node (Daten, kein Code)           [eigenes Repo]
FreeSynergy/Wiki.Store     ← Plugin-Registry für Wiki.rs (zukünftig)               [eigenes Repo]
FreeSynergy/Decidim.Store  ← Plugin-Registry für Decidim.rs (zukünftig)            [eigenes Repo]
```

### FreeSynergy/Lib — Bibliotheken

Alle Präfixe sind `fsn-` (FreeSynergy Network).

```
fsn-types/              Shared Types (Resource, Meta, TypeSystem, Capability)
fsn-error/              Fehlerbehandlung + Auto-Repair + Repairable-Trait
fsn-config/             TOML laden/speichern mit Validierung + Auto-Repair
fsn-i18n/               Fluent-basierte Schnipsel (actions, nouns, status, errors, ...)
fsn-sync/               CRDT-Sync (Automerge-Wrapper)
fsn-store/              Universeller Store-Client (Download, Registry, Suche)
fsn-plugin-sdk/         WASM Plugin SDK (Traits, wit-bindgen Interfaces)
fsn-plugin-runtime/     WASM Host (wasmtime)
fsn-federation/         OIDC + SCIM + ActivityPub + WebFinger
fsn-auth/               OAuth2 + JWT + Permissions
fsn-bridge-sdk/         Bridge-Interface-Traits
fsn-container/          Container-Abstraktion (Podman via bollard)
fsn-template/           Tera-Wrapper
fsn-health/             Health-Check Framework
fsn-crypto/             age-Encryption, mTLS, Key-Management
fsn-db/                 Datenbank-Abstraktion (SeaORM + rusqlite)
fsn-theme/              Theme-System (CSS-Variablen, TUI-Farben)
fsn-help/               Kontextsensitives Hilfe-System
```

### FreeSynergy/Node — Deployment-Engine

```
cli/crates/
  fsn-node-core/        Node-spezifische Logik + Datentypen (Config, State, Health, Store)
  fsn-deploy/           Quadlet-Generation, Zentinel, Reconciliation, Hooks
  fsn-dns/              DNS-Provider Integrationen (Hetzner, Cloudflare)
  fsn-host/             Host-Management, SSH, Remote-Install, Provisioning
  fsn-wizard/           Container-Assistent (Docker Compose → FSN-Modul)
  fsn-node-cli/         CLI Binary (clap) — `fsn` Kommando, kein UI-Code
  fsn-installer/        Server-Setup-Tooling (Erstinstallation)
```

**Kein UI-Code in Node.** Das UI gehört in Desktop.

### FreeSynergy/Desktop — Desktop-Umgebung

```
crates/
  fsd-shell/            Desktop Shell (Taskbar, Window Manager, Wallpaper)
  fsd-conductor/        Container/Service/Bot Management (vormals "Admin")
  fsd-store/            Package Manager (Browser, Install, Updates)
  fsd-studio/           Plugin/Modul/Sprachdatei-Ersteller (+AI optional)
  fsd-settings/         System Settings
  fsd-profile/          User Profile
  fsd-app/              App-Launcher Binary (startet alles)
```

Jedes `fsd-*` Crate kann als **eigenständiges Fenster oder Prozess** gestartet werden (Dioxus Multiwindow). `fsd-app` ist das Einstiegsprogramm das die Shell lädt.

---

## 3. Datenbank-Empfehlung: SeaORM + rusqlite

### Warum SeaORM?

Nach Analyse aller Optionen ist **SeaORM 2.0 mit rusqlite-Backend** die beste Wahl:

- **Async + Sync**: SeaORM 2.0 hat ein offizielles `sea-orm-sync` Crate mit rusqlite-Backend — perfekt für CLI-Tools wo async Overkill wäre, und das async-Backend für den Server/UI
- **Entity-First Workflow**: Entities definieren → Schema generiert. Passt zu unserem OOP-Ansatz
- **Migrationen eingebaut**: `sea-orm-cli` für Schema-Migrations
- **Multi-DB-fähig**: Startet mit SQLite, kann auf Postgres wechseln wenn nötig (Wiki.rs wird Postgres brauchen)
- **Admin Panel**: SeaORM Pro bietet gratis RBAC-Admin-Panel
- **Wiederverwendbar**: Dieselbe `fsn-db` Library kann in Node (SQLite), Wiki.rs (Postgres) und Decidim.rs (Postgres) eingesetzt werden

### Write-Buffering Engine (wie ownCloud)

Für das Problem mit vielen gleichzeitigen Schreibzugriffen (das Du von ownCloud kennst):

```rust
/// fsn-db: Write-Buffer für SQLite
pub struct WriteBuffer {
    queue: Vec<BufferedWrite>,
    flush_interval: Duration,    // z.B. 100ms
    max_batch_size: usize,       // z.B. 500 Operationen
    db: DatabaseConnection,
}

impl WriteBuffer {
    /// Schreibt nicht sofort, sondern puffert
    pub async fn enqueue(&mut self, write: BufferedWrite) -> Result<()>;

    /// Flush: Schreibt alle gepufferten Operationen in einer Transaktion
    pub async fn flush(&mut self) -> Result<FlushResult>;

    /// Automatischer Flush per Timer oder Batch-Größe
    pub async fn run_auto_flush(&mut self);
}
```

Das kombiniert SQLite-Vorteile (embedded, keine Infra) mit Batch-Writes (keine Lock-Contention bei vielen Zugriffen).

### Schema in fsn-db (wiederverwendbar)

```rust
// fsn-db bietet Basis-Entities die jedes Projekt erweitern kann
pub mod entities {
    pub mod resource;     // Basis-Resource mit Metadaten
    pub mod permission;   // RBAC-Permissions
    pub mod sync_state;   // CRDT-Sync-Zustand
    pub mod plugin;       // Installierte Plugins
    pub mod audit_log;    // Audit-Trail
}

// Node erweitert mit eigenen Entities
pub mod node_entities {
    pub mod host;
    pub mod project;
    pub mod module;
    pub mod container;
}
```

---

## 4. Desktop-Architektur (FreeSynergy.Desktop)

### 4.1 Übersicht

FreeSynergy.Desktop ist ein **eigenes Programm und eigenes Repository**. Es ist wie ein echter Desktop (KDE-ähnlich), gebaut mit Dioxus Multiwindow. Jede App (`fsd-*`) läuft als eigenständiges Fenster — auf dem Desktop parallel zu anderen, im Web als Tab, im TUI als Panel.

```
┌─────────────────────────────────────────────────────────────────┐
│ FreeSynergy                         [Wallpaper / Hintergrund]  │
│                                                                  │
│  ┌──────────────┐  ┌──────────────────────────────────────┐    │
│  │  Conductor   │  │  Store                               │    │
│  │              │  │                                      │    │
│  │  [Container] │  │  [Paket suchen...]                   │    │
│  │  [Bots]      │  │  ┌────┐ fsn-nginx    [Installieren] │    │
│  │  [Ressourcen]│  │  └────┘ fsn-postgres [Installieren] │    │
│  └──────────────┘  └──────────────────────────────────────┘    │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│ [⚙ Apps] [Conductor] [Store] [Studio] [Settings]  🔔 12:34 DE │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Taskbar (fsd-shell)

Wie KDE Plasma — immer sichtbar, konfigurierbar:

- **App-Launcher**: Grid aller installierten Apps + Suche (Win-Taste / Klick)
- **Laufende Apps**: Icons mit Fenster-Vorschau bei Hover (wie KDE)
- **System Tray**: Sync-Status, Netzwerk, Notifications
- **Sprachanzeige**: aktive Sprache, wechselbar per Klick
- **Uhr + Datum**

### 4.3 Apps (fsd-*)

| App | Zweck | Standalone? |
|---|---|---|
| `fsd-shell` | Taskbar, Window Manager, Wallpaper | Nein (läuft immer) |
| `fsd-conductor` | Container/Service/Bot Management | Ja |
| `fsd-store` | Package Manager | Ja |
| `fsd-studio` | Plugin/Modul/i18n-Ersteller (+AI) | Ja |
| `fsd-settings` | System Settings | Ja |
| `fsd-profile` | User Profile | Ja |

Standalone = kann auch ohne Shell als eigenes Fenster/Prozess gestartet werden.

### 4.4 Conductor (vormals "Admin")

**Conductor** dirigiert Container, Services und Bots — wie ein Orchesterdirigent:

- Container installieren, starten, stoppen, neustarten
- Ressourcen konfigurieren (CPU, RAM, Volumes, Netzwerk)
- Bots laden und steuern
- Logs und Status in Echtzeit
- Service-Abhängigkeiten visualisieren
- Health-Status aller laufenden Services

```
fsn conductor start <service>     ← CLI
fsn conductor                      ← TUI
fsd-conductor                      ← GUI
```

### 4.5 Store (Package Manager)

**Trennung der Verantwortung:**
- **Store** = Discovery + Download + Abhängigkeiten auflösen + Updates + Entfernen
- Bei Installation: **Setup-Wizard** aus Paket-Metadaten (Konfiguration VOR dem ersten Start)
- **Conductor** = Laufzeit-Management (Starten, Stoppen, Ressourcen, Logs)

Wie `apt install` + interaktiver Konfig-Dialog → dann läuft's → Management in Conductor.

```
fsn store search <query>           ← CLI
fsn store install <package>        ← CLI (triggert Setup-Wizard)
fsn store update                   ← CLI
fsd-store                          ← GUI
```

### 4.6 Studio (Plugin/Modul/i18n-Ersteller)

Studio ist das Werkzeug um Inhalte für das FSN-Ökosystem zu erstellen:

- **Module Builder**: YAML/Docker-Compose → FSN-Modul (= heutiger Wizard, aus Node extrahiert)
- **Plugin Builder**: WASM-Plugin generieren (wit-bindgen Templates)
- **i18n Editor**: Sprachdateien visuell bearbeiten und erstellen
- **AI-Erweiterung** (optional): Natürlichsprachliche Beschreibung → Modul-Metadaten generiert

```
fsn studio                         ← TUI
fsd-studio                         ← GUI
```

### 4.7 Settings

| Bereich | Inhalt |
|---|---|
| **Appearance** | Wallpaper (URL oder Datei-Upload), CSS-Datei (URL oder Upload), Logo, Theme, Dark/Light |
| **Language** | Sprache wählen, Sprachdateien aus Store laden |
| **Service Roles** | Welcher Container für welche Funktion (Auth, Mail, Storage, Git, …) |
| **Accounts** | Verbundene OIDC-Accounts |
| **Desktop** | Taskbar-Position, Autostart-Apps |

### 4.8 Service Roles (erweiterter MIME-Standard)

Wie MIME, aber für **Funktionen** statt Dateitypen. Container registrieren welche Rollen sie erfüllen können. Settings wählt den aktiven Handler pro Rolle.

```toml
[service-roles]
auth     = "kanidm"       # Welcher Container ist Auth-Provider?
mail     = "stalwart"     # Mail-Handler
git      = "forgejo"      # Git-Handler
storage  = "seaweedfs"    # Storage-Handler
wiki     = "outline"      # Wiki-Handler
chat     = "tuwunel"      # Chat-Handler
tasks    = "vikunja"      # Task-Handler
```

Container-Metadaten deklarieren welche Rollen sie unterstützen:

```toml
[module.roles]
provides = ["auth", "iam"]   # Diese Rollen kann dieser Container übernehmen
requires = ["mail"]          # Diese Rollen müssen erfüllt sein
```

---

## 5. UI-Architektur (Fenster-System)

### 5.1 Alle Einblendungen sind Fenster

Konsistentes Verhalten überall:

```rust
pub struct Window {
    pub id: WindowId,
    pub title: LocalizedString,
    pub content: Box<dyn WindowContent>,
    pub closable: bool,             // Immer true
    pub buttons: Vec<WindowButton>, // OK, Cancel, Apply
    pub size: WindowSize,
    pub scrollable: bool,           // Automatisch wenn Inhalt > Fenster
    pub help_topic: Option<String>, // Für kontextsensitive Hilfe
}

pub enum WindowButton {
    Ok,          // Bestätigen + Schließen
    Cancel,      // Abbrechen + Schließen
    Apply,       // Übernehmen (bleibt offen)
    Custom { label_key: String, action: WindowAction },
}
```

### 5.2 Container-Render-Modi (Metadaten pro Modul)

```toml
[module.ui]
supports_web      = true   # Hat Web-Interface
supports_tui      = false  # Hat TUI-Interface (selten)
supports_desktop  = true   # Kann als Desktop-App eingebettet werden
supports_api_only = true   # Nur API, kein UI

open_mode         = "iframe"   # "iframe" | "external_browser" | "embedded" | "api"
web_url_template  = "https://{{ domain }}/{{ service_path }}"
```

### 5.3 Scrolling (auch in TUI)

Jedes Formular und jede Liste ist **automatisch scrollbar** wenn der Inhalt nicht passt:

```rust
pub trait Scrollable {
    fn content_height(&self) -> u32;
    fn viewport_height(&self) -> u32;
    fn scroll_offset(&self) -> u32;
    fn needs_scroll(&self) -> bool {
        self.content_height() > self.viewport_height()
    }
}
```

Maus-Scrolling + Tastatur (PgUp/PgDn/Home/End) in allen Interfaces.

### 5.4 Hilfe-System (fsn-help)

```rust
pub struct HelpSystem {
    topics: HashMap<String, HelpTopic>,
    i18n: I18n,
}

pub struct HelpTopic {
    pub id: String,
    pub title_key: String,       // i18n-Key
    pub content_key: String,     // i18n-Key
    pub related: Vec<String>,    // Verwandte Themen
    pub context: HelpContext,    // Wo diese Hilfe angezeigt wird
}

impl HelpSystem {
    /// Kontextsensitive Hilfe: Was ist gerade aktiv?
    pub fn help_for_context(&self, ctx: &str) -> Option<&HelpTopic>;

    /// Suche in Hilfetexten
    pub fn search(&self, query: &str) -> Vec<&HelpTopic>;

    /// Anzeigen als Fenster
    pub fn show_help_window(&self, topic: &str) -> Window;
}
```

Aufruf: **F1** (Desktop/Web), **?** (TUI), Menü, oder Hilfe-Button in jedem Fenster.

---

## 6. Theme-System (fsn-theme)

### 6.1 Eine Datei regiert alles

Der Benutzer (oder eine KI die die Website baut) liefert **eine einzige Theme-Datei** ab. Diese wird für Dioxus (Desktop/Web) UND TUI interpretiert.

### 6.2 Theme-Format: `theme.toml`

```toml
[theme]
name    = "FreeSynergy Default"
version = "1.0.0"
author  = "KalEl"

[colors]
primary        = "#2563eb"
primary_hover  = "#1d4ed8"
primary_text   = "#ffffff"

secondary      = "#64748b"
secondary_hover = "#475569"
secondary_text = "#ffffff"

bg_base    = "#ffffff"
bg_surface = "#f8fafc"
bg_overlay = "#f1f5f9"
bg_sidebar = "#1e293b"

text_primary   = "#0f172a"
text_secondary = "#475569"
text_muted     = "#94a3b8"
text_inverse   = "#ffffff"

success = "#22c55e"
warning = "#f59e0b"
error   = "#ef4444"
info    = "#3b82f6"

border_default = "#e2e8f0"
border_focus   = "#2563eb"

[typography]
font_family   = "Inter, system-ui, sans-serif"
font_mono     = "JetBrains Mono, monospace"
font_size_base = "16px"
font_size_sm  = "14px"
font_size_lg  = "20px"
font_size_xl  = "24px"
font_size_2xl = "30px"
line_height   = "1.5"

[spacing]
unit      = "4px"
radius_sm = "4px"
radius_md = "8px"
radius_lg = "12px"

[tui]
# Wird automatisch aus [colors] abgeleitet, kann überschrieben werden
primary_fg   = "blue"
primary_bg   = "default"
sidebar_fg   = "white"
sidebar_bg   = "dark_gray"
border_style = "rounded"    # "plain" | "rounded" | "double" | "thick"
status_ok    = "green"
status_error = "red"
status_warn  = "yellow"
```

### 6.3 CSS-Variablen Konvention (für Website-KI)

```
Datei: theme.css

Variablen-Namensschema (Präfix IMMER --fsn-):
  --fsn-color-primary: #2563eb;
  --fsn-color-primary-hover: #1d4ed8;
  --fsn-color-bg-base: #ffffff;
  --fsn-color-bg-surface: #f8fafc;
  --fsn-color-text-primary: #0f172a;
  --fsn-color-success: #22c55e;
  --fsn-color-warning: #f59e0b;
  --fsn-color-error: #ef4444;
  --fsn-font-family: 'Inter', system-ui, sans-serif;
  --fsn-font-mono: 'JetBrains Mono', monospace;
  --fsn-font-size-base: 16px;
  --fsn-spacing-unit: 4px;
  --fsn-radius-md: 8px;

Liefere NUR :root { ... } — kein Layout, keine Komponenten.
FreeSynergy.Node konvertiert diese automatisch in theme.toml.
```

### 6.4 Konvertierung

```rust
/// fsn-theme: Konvertiert zwischen Formaten
pub struct ThemeEngine {
    theme: Theme,
}

impl ThemeEngine {
    pub fn from_toml(path: &Path) -> Result<Self>;
    pub fn from_css(path: &Path) -> Result<Self>;  // CSS Custom Properties → Theme
    pub fn to_css(&self) -> String;                 // → Dioxus Web
    pub fn to_tui_palette(&self) -> TuiPalette;    // → TUI
    pub fn to_tailwind_config(&self) -> String;    // → Tailwind
}
```

### 6.5 Mehrere Themes, wechselbar

Themes werden wie Plugins über den Store verteilbar und in Settings wechselbar:

```toml
[appearance]
active_theme     = "freesynergy-default"
available_themes = ["freesynergy-default", "freesynergy-dark", "helfa-green"]
```

---

## 7. i18n — Schnipsel-System

### Kleine, wiederverwendbare Bausteine

```
locales/{lang}/
  actions.ftl     → save, delete, edit, search, confirm, cancel, ...
  nouns.ftl       → module, server, project, host, plugin, store, ...
  status.ftl      → online, offline, error, loading, syncing, ...
  errors.ftl      → file-not-found, invalid-config, connection-failed, ...
  phrases.ftl     → select-item, confirm-delete, welcome-to, ...
  time.ftl        → ago, minutes, hours, days, just-now, ...
  validation.ftl  → required-field, invalid-email, too-short, ...
  help.ftl        → help-dashboard, help-wizard, help-store, ...
```

Zusammengesetzt im Code:
```rust
// t("action-save") → "Save" / "Speichern"
// t_phrase("phrase-confirm-delete", [("item", t("noun-module"))])
//   → "Delete module?" / "Modul löschen?"
```

---

## 8. Error-Handling + Auto-Repair

Siehe Plan v2 — unverändert. Zusammenfassung:
- **Repairable-Trait** auf allen Konfig-Typen
- **AutoRepaired** → Toast-Notification
- **NeedsUserDecision** → Dialog mit Optionen
- **Unrecoverable** → Fehler anzeigen, nicht öffnen
- Backup immer anbieten bevor repariert wird

---

## 9. Container-Assistent (fsn-wizard / fsd-studio)

Der Container-Assistent lebt in **fsd-studio** (GUI) und **fsn-wizard** (Library):
- YAML/Docker-Compose eingeben (Text, URL, Datei)
- Automatische Typ-Erkennung (Image-Name, Ports, Volumes)
- Modul-Generation mit Standard-Werten
- Erklärungen was fehlt (APIs, Abhängigkeiten)
- Benutzer wählt Purpose/Service-Role
- Optional: AI-gestützte Generierung

---

## 10. Typ-System + Schnittstellen

Siehe Plan v2 — unverändert. Zusammenfassung:
- **Capability-Trait**: Was kann ein Service? (APIs, Events, Formate)
- **Requirement-Trait**: Was braucht ein Service?
- **TypeRegistry**: Validiert Abhängigkeiten, findet kompatible Bridges
- Pro Typ: TOML-Definition im Store mit APIs, Events, Bridge-Kompatibilität
- **Service Roles** (siehe 4.8): Extended MIME für Funktionen

---

## 11. CRDT + Sync + Federation + Store + Bridges + Permissions

Alle Details aus Plan v2 bleiben bestehen. Hier nur die Entscheidungen:

| Thema | Entscheidung |
|---|---|
| CRDT | **Automerge** (3 wenn stabil, sonst 0.5 stable). Beitrag zum Projekt möglich. |
| Plugin-Interface | **wit-bindgen** (WASM Component Model Standard) |
| ActivityPub | **activitypub_federation** (Lemmy, Axum-kompatibel) |
| Datenbank | **SeaORM 2.0** + rusqlite (sync) + sqlx (async/Postgres) |
| Templates | **Tera** (einziger Template-Engine) |

---

## 12. Verbesserungsvorschläge

### 12.1 Versionierung & Changelog

Jede `fsn-*` Library bekommt **eigene SemVer-Versionierung**. CHANGELOG.md pro Crate, nicht nur global. Nutze `cargo-release` für koordinierte Releases.

### 12.2 Feature Flags überall

Jede Library sollte granulare Feature-Flags haben:

```toml
[features]
default      = ["sqlite"]
sqlite       = ["sea-orm/rusqlite"]
postgres     = ["sea-orm/sqlx-postgres"]
sync         = ["automerge"]
federation   = ["activitypub_federation", "openidconnect"]
wasm-plugins = ["wasmtime"]
```

Das hält die Compile-Zeiten kurz. Wiki.rs braucht vielleicht `federation` + `postgres` aber kein `wasm-plugins`.

### 12.3 CI/CD von Anfang an

- **GitHub Actions**: Build, Test, Clippy, Rustfmt auf jedem Push
- **cargo-deny**: License-Check, Advisory-DB-Check
- **Dependabot**: Automatische Dependency-Updates
- **Nightly Fuzzing**: cargo-fuzz auf fsn-config, fsn-sync, fsn-template (alles was User-Input parst)

### 12.4 Dokumentation

- **Jede fsn-* Crate**: README.md + `#[doc]` auf allen pub Items
- **docs.rs** automatisch (bei Publish auf crates.io)
- **Architektur-Docs**: `docs/ARCHITECTURE.md` pro Repo
- **Beispiele**: `examples/` Verzeichnis in jeder Library

### 12.5 Error-Recovery für Netzwerk

```rust
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
    pub on_failure: FailureAction,  // Cache nutzen, Offline-Modus, Benutzer fragen
}
```

Wenn der Store nicht erreichbar ist → lokalen Cache nutzen. Wenn ein Host offline ist → markieren, nicht crashen.

### 12.6 Offline-First

Store-Katalog wird gecacht, Konfigurationen sind lokal, CRDT-Sync passiert wenn Verbindung da ist. Kein Feature darf eine Netzwerkverbindung voraussetzen außer explizit netzwerk-basierten Aktionen.

### 12.7 Audit-Log

```rust
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub actor: Subject,
    pub action: AuditAction,
    pub target: ResourceRef,
    pub details: Value,
    pub source_host: HostRef,
}
```

Wird per CRDT synchronisiert → verteiltes, konsistentes Audit-Log.

### 12.8 Graceful Degradation

Wenn ein Plugin nicht lädt → Rest funktioniert trotzdem. Wenn CRDT-Sync fehlschlägt → lokaler Zustand bleibt nutzbar. Wenn ein Host offline ist → andere Hosts arbeiten weiter.

### 12.9 Config-Schema als JSON-Schema

Plugin-Metadaten und Modul-Konfigurationen bringen ein **JSON-Schema** mit (Format bleibt TOML):
- Automatische Validierung
- UI-Generierung (Forms aus Schema generieren)
- Dokumentation (Schema → Docs)

### 12.10 Migration von v1

- Modul-Definitionen → migrieren in Node.Store Format
- Deployment-Logik → migrieren in fsn-deploy
- i18n-Strings → migrieren in fsn-i18n Schnipsel-Format
- Ein `migration/` Verzeichnis mit Skripten die alte Configs konvertieren

### 12.11 ratatui/rat-salsa entfernen

Da wir auf **Dioxus** umsteigen (hat `dioxus-terminal` für TUI), fällt ratatui komplett weg:
1. Alle bisherigen TUI-Nodes neu als Dioxus-Komponenten in `fsd-*`
2. `rat-widget`, `ratatui`, `rat-salsa` aus allen `Cargo.toml` entfernen
3. `fsn-tui` Crate wird aufgelöst — Komponenten wandern in `fsd-shell` oder `fsn-*` Libraries

---

## 13. Umsetzungsplan

### Phase 0: Setup ✓

- [x] `FreeSynergy/Lib` erstellen, CI einrichten
- [x] `FreeSynergy/Desktop` erstellen, CI einrichten
- [x] `FreeSynergy/UI` archivieren
- [x] `FreeSynergy/Node` bereinigen (ratatui/rat-salsa entfernen, fsn-app entfernen)
- [x] CLAUDE.md in allen Repos aktualisieren

### Phase 1: Fundament (FreeSynergy.Lib) ✓

fsn-types, fsn-error, fsn-config, fsn-i18n, fsn-theme, fsn-help, fsn-db, fsn-health

### Phase 2: CRDT + Sync (Stub)

fsn-sync (Automerge) — Stub implementiert, aktive Integration ausstehend

### Phase 3: Store + Plugins (Stub)

fsn-store, fsn-plugin-sdk, fsn-plugin-runtime — Stubs implementiert

### Phase 4: Auth + Federation (Stub)

fsn-auth, fsn-federation, fsn-crypto — Stubs implementiert

### Phase 5: Container + Templates (Stub)

fsn-container, fsn-template, fsn-health — implementiert

### Phase 6: Node Application ✓

fsn-node-core, fsn-deploy, fsn-host, fsn-wizard, fsn-node-cli, fsn-dns, fsn-installer

### Phase 7: Desktop (FreeSynergy.Desktop)

fsd-shell, fsd-conductor, fsd-store, fsd-settings, fsd-profile, fsd-studio, fsd-app — in Planung

### Phase 8: Bridges (ongoing)

fsn-bridge-sdk + erste WASM-Bridge-Plugins

---

## 14. Vollständiger Bibliotheken-Stack

### Kern

| Crate | Version | Zweck |
|---|---|---|
| `dioxus` | 0.7.x | UI: TUI + Desktop + Web + Mobile |
| `serde` + `toml` + `serde_json` | 1 / 0.8 / 1 | Serialisierung |
| `sea-orm` | 2.0 | ORM (async: sqlx, sync: rusqlite) |
| `sea-orm-sync` | 2.0 | Sync SQLite für CLI |
| `automerge` | 0.5+ / 3.x | CRDT |
| `tera` | 1 | Templates |
| `fluent` | 0.16 | i18n |
| `activitypub_federation` | 0.7 | ActivityPub |

### Netzwerk

| Crate | Zweck |
|---|---|
| `reqwest` (rustls) | HTTP-Client |
| `axum` (via Dioxus) | HTTP-Server |
| `tokio-tungstenite` | WebSocket |
| `russh` | SSH |
| `rustls` + `rcgen` | TLS + Zertifikate |
| `tonic` | gRPC |

### Auth

| Crate | Zweck |
|---|---|
| `openidconnect` | OIDC |
| `oauth2` | OAuth2 |
| `jsonwebtoken` | JWT |
| `age` | Secrets |

### Plugins

| Crate | Zweck |
|---|---|
| `wasmtime` + `wasmtime-wasi` | WASM Runtime (Standard) |
| `wit-bindgen` | Component Model Interfaces |
| `libloading` + `abi_stable` | Native (nur Ausnahmen) |

### Container

| Crate | Zweck |
|---|---|
| `bollard` | Podman/Docker API |
| `serde_yaml` | YAML-Parse |
| `tokio-cron-scheduler` | Scheduling |
| `backon` | Retry mit Backoff |

### Qualität

| Crate | Zweck |
|---|---|
| `thiserror` + `anyhow` | Errors |
| `tracing` + `tracing-subscriber` | Logging |
| `opentelemetry` + `opentelemetry-otlp` | Observability |
| `rstest` + `insta` + `mockall` | Testing |
| `cargo-fuzz` | Fuzzing |
| `testcontainers` | Integration Tests |
| `schemars` | JSON-Schema Generation |
| `cargo-deny` | License/Advisory Check |

---

## 15. Zusammenfassung aller Entscheidungen

| Frage | Entscheidung |
|---|---|
| UI-Framework | **Dioxus 0.7.x** (TUI + Desktop + Web + Mobile) |
| Datenbank | **SeaORM 2.0** (rusqlite sync + sqlx async) |
| CRDT | **Automerge** (von Tag 1) |
| Plugins | **WASM-First** (wit-bindgen, wasmtime) |
| Templates | **Nur Tera** |
| Federation | **OIDC + SCIM + ActivityPub** (von Tag 1) |
| ActivityPub Crate | **activitypub_federation** |
| Theme-System | **Eine Datei** (theme.toml oder theme.css → konvertierbar) |
| CSS-Präfix | **--fsn-** (nicht --fsy-) |
| Fenster | **Alle Einblendungen sind Fenster** (OK/Cancel/Apply) |
| Hilfe | **Immer aufrufbar** (F1, ?, Menü) |
| Scrolling | **Automatisch** wenn Inhalt > Viewport |
| TUI-Framework | **Dioxus terminal** (ratatui/rat-salsa entfernt) |
| Desktop | **Eigenes Repo** (FreeSynergy/Desktop) |
| Admin-Begriff | **Conductor** (Container/Service/Bot Management) |
| Wizard | **fsd-studio** (GUI) + **fsn-wizard** (Library) |
| MIME-Erweiterung | **Service Roles** (fsn-Prefix in TOML) |
| Package Manager | **fsd-store** (Discovery+Install+Wizard), Conductor für Laufzeit |
| Crate-Präfix Lib | **fsn-** (nicht fsy-) |
| Crate-Präfix Desktop | **fsd-** |
| Sprache im Code | **Englisch** |
| Sprache hier | **Deutsch** |
| Lib-Veröffentlichung | **crates.io** (wenn APIs stabil) |
| Repo-Struktur | **Lib + Node + Desktop + Store-Repos** (je eigenes Repo) |
| Wiki.rs/Decidim.rs | **Demnächst** — fsn-* Libraries müssen stabil sein |

---

## Nächster Schritt

Phase 7: FreeSynergy.Desktop — Dioxus-App mit fsd-shell, fsd-conductor (Hosts/Services/Projekte), fsd-store (Plugin-Browser), fsd-studio (Modul-Builder), fsd-settings, fsd-profile, fsd-app.

Parallel: Phase 2–4 Stubs in FreeSynergy.Lib aktivieren (fsn-sync Automerge, fsn-auth OIDC, fsn-federation ActivityPub).
