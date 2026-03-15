# FreeSynergy — Implementierungsplan

**Stand:** März 2026 · **Version:** 5.0 FINAL  
**Autor:** KalEl + Claude  
**Sprachen im Code:** Englisch (Code + Kommentare), Deutsch (Kommunikation)  
**Mindest-Sprachen:** Deutsch + Englisch (immer unterstützt, Standard)

---

# VISION

FreeSynergy ist ein Ökosystem für dezentrale, freiwillige Zusammenarbeit. Es besteht aus drei Säulen:

**Node** — Der Kopf. Verwaltet Infrastruktur, Container, Hosts. Läuft auf Servern. Synchronisiert sich mit anderen Nodes. Braucht keinen Menschen, um zu funktionieren. Node ist die Maschine, die im Hintergrund arbeitet.

**Desktop** — Das Gesicht. Die Schnittstelle zum Menschen. Zeigt was Node tut. Ermöglicht Konfiguration, Überwachung, Kommunikation. Läuft überall: Browser, Desktop-App, Terminal, Mobilgerät. Desktop braucht Node, aber Node braucht nicht Desktop.

**Die Bibliotheken (Lib)** — Das Rückgrat. Wiederverwendbare Bausteine, die Node, Desktop, Wiki.rs, Decidim.rs und jedes zukünftige Projekt nutzen. Eigenständig, standardbasiert, offen.

**Die Idee:** Jeder kann einen Node aufsetzen. Jeder Node ist souverän. Nodes können freiwillig zusammenarbeiten — über Federation, über den Message-Bus, über geteilte Identitäten. Kein Zwang, kein Zentralserver, kein Vendor-Lock-in. Offene Standards überall.

**Der Message-Bus** verbindet nicht nur FreeSynergy-Services, sondern jedes Open-Source-Programm das eine API hat: ERPs, CRMs, Projektmanagement, LLMs, Wikis, Messenger, Monitoring, CI/CD — alles. Die Abstraktionsschicht macht es egal, woher Daten kommen und wohin sie gehen.

**LLMs als Vermittler:** Ein lokales LLM (Ollama, llama.cpp) oder ein Cloud-LLM (Claude API) sitzt im System und hilft dem Menschen, mit den Programmen zu reden. Es übersetzt natürliche Sprache in API-Calls, fasst Logs zusammen, erklärt Fehler, schlägt Konfigurationen vor — ohne Daten nach außen zu tragen, weil es im System läuft.

---

# TEIL A — ARCHITEKTUR

## A.1 Grundprinzipien

1. **Bibliotheken zuerst** — Jede Funktionalität als `fsn-*` Library, nutzbar in allen Projekten.
2. **Standards vor Eigenentwicklung** — OCI, WASM Component Model, ActivityPub, OIDC, SCIM, Automerge.
3. **WASM-First** — Plugins immer als WASM. Native nur mit Begründung.
4. **CRDT von Tag 1** — Automerge, kein nachträgliches Einbauen.
5. **Offline-First** — Alles muss ohne Netzwerk funktionieren.
6. **Graceful Degradation** — Kaputtes Plugin/Host ≠ kaputtes System.
7. **O(N+M)** — Message-Bus statt N×M Einzelverbindungen.
8. **Ein Codebase, alle Plattformen** — Dioxus für WGUI/GUI/TUI/Web.
9. **Node = Kopf, Desktop = Gesicht** — Getrennte Repos, klare Aufgaben.
10. **Kleine i18n-Schnipsel** — Wiederverwendbar, zusammensetzbar.

## A.2 Repository-Struktur

**Alle Repos liegen unter dem GitHub-Account `FreeSynergy/`. Im Code und in der Dokumentation heißen sie `FreeSynergy.{Name}`. In der Repo-URL reicht der Name ohne Prefix.**

```
FreeSynergy/
├── Lib                 ← Wiederverwendbare Bibliotheken (Cargo Workspace)
├── Node                ← Server-Infrastruktur, Deployment, Host-Sync
├── Desktop             ← UI, Dashboard, Bot-Management, Store-Browser
├── Store               ← Plugin-Registry (Code: store-sdk + Daten: Katalog)
│
├── Node.Store          ← ARCHIVIEREN → Inhalt nach Store migrieren
└── UI                  ← ARCHIVIEREN → ersetzt durch Dioxus + fsn-ui
```

**Namenskonvention:**
- Repo-Name: `Node`, `Desktop`, `Lib`, `Store`
- Vollständiger Projektname: `FreeSynergy.Node`, `FreeSynergy.Desktop`
- Crate-Prefix: `fsn-` (überall, einheitlich)
- CSS-Variablen-Prefix: `--fsn-`
- i18n-Key-Prefix: keiner (direkt z.B. `action-save`)

## A.3 Node vs. Desktop — Abgrenzung

| | **Node** | **Desktop** |
|---|---|---|
| **Aufgabe** | Infrastruktur, Container, Hosts, Sync, Federation | UI, Dashboard, Mensch-Maschine-Schnittstelle |
| **Läuft auf** | Server (Linux, FCOS) | Überall: Browser, Desktop-App, Terminal, Mobil |
| **Braucht Desktop?** | Nein — arbeitet autonom | Ja — zeigt was Node tut |
| **Braucht Node?** | — | Ja — verbindet sich mit Node(s) |
| **Benutzer** | Maschine, Cron, API | Mensch |
| **Repo** | `FreeSynergy/Node` | `FreeSynergy/Desktop` |
| **Binary** | `fsn-node` (Daemon + CLI) | `fsn-desktop` (Dioxus App) |

**Auf einem Host:** Man installiert immer `Node`. Wenn ein Mensch direkt mit dem Host interagieren soll, installiert man zusätzlich `Desktop`. Auf einem reinen Server (Headless) braucht man Desktop nicht — man steuert über einen anderen Desktop oder via Bots.

## A.4 Crate-Übersicht (alle `fsn-` benannt)

### FreeSynergy/Lib — Wiederverwendbare Libraries

```
fsn-types/              Shared Types, Resource, Meta, TypeSystem, Capability, Requirement
fsn-error/              Fehlerbehandlung, Auto-Repair, Repairable-Trait
fsn-config/             TOML laden/speichern, Validierung, Auto-Repair, JSON-Schema
fsn-i18n/               Fluent Schnipsel-System (Deutsch + Englisch Minimum, ~50 geplant)
fsn-db/                 Datenbank (SeaORM 2.0 + rusqlite + WriteBuffer)
fsn-sync/               CRDT-Sync (Automerge)
fsn-store/              Universal Store-Client (OCI-kompatibel)
fsn-pkg/                Package-Manager (OCI Distribution Spec, API-Manifests)
fsn-plugin-sdk/         WASM Plugin SDK (wit-bindgen Component Model)
fsn-plugin-runtime/     WASM Host (wasmtime)
fsn-federation/         OIDC + SCIM + ActivityPub + WebFinger
fsn-auth/               OAuth2 + JWT + Permissions + RBAC
fsn-bus/                Universal Message Bus (Event-Routing)
fsn-channel/            Plattform-Adapter (Matrix, Telegram, Discord, ...)
fsn-bot/                Bot-Framework (Commands, plattformunabhängig)
fsn-llm/                LLM-Integration (Ollama, Claude API, lokale Modelle)
fsn-bridge-sdk/         Bridge-Interface-Traits
fsn-container/          Container-Abstraktion (Podman via bollard)
fsn-template/           Tera-Wrapper
fsn-health/             Health-Check Framework
fsn-crypto/             age, mTLS, Key-Management
fsn-theme/              Theme-System (CSS-Vars, TUI-Palette, wechselbar)
fsn-help/               Kontextsensitives Hilfe-System
fsn-ui/                 Dioxus UI-Komponenten (Buttons, Windows, Forms, ...)
```

### FreeSynergy/Node — Server-Anwendung

```
crates/
  fsn-node-core/        Node-spezifische Logik (Host, Project, Module)
  fsn-deploy/           Quadlet-Generation, Zentinel-Konfig
  fsn-host/             Host-Management, SSH, Remote-Install, Sync
  fsn-wizard/           Container-Assistent (YAML → Modul)
  fsn-node-cli/         CLI + Daemon Binary
```

### FreeSynergy/Desktop — UI-Anwendung

```
crates/
  fsn-desktop-app/      Dioxus App (WGUI/GUI/TUI/Web)
  fsn-desktop-views/    Views: Home, Admin, Store, Bots, Help, Settings
```

---

# TEIL B — BIBLIOTHEKEN-STACK

## B.1 Alle Dependencies

### Kern

| Crate | Zweck |
|---|---|
| `dioxus` 0.7.x | UI: WGUI + GUI + TUI + Web + Mobile |
| `serde` + `toml` + `serde_json` + `serde_yaml` | Serialisierung |
| `sea-orm` 2.0 + `sea-orm-sync` | ORM (async + sync) |
| `automerge` 0.5+ / 3.x | CRDT |
| `tera` 1 | Templates |
| `fluent` + `fluent-bundle` | i18n |
| `schemars` + `jsonschema` | JSON-Schema |

### Netzwerk

| Crate | Zweck |
|---|---|
| `reqwest` (rustls) | HTTP-Client |
| `axum` | HTTP-Server |
| `tokio-tungstenite` | WebSocket |
| `russh` | SSH |
| `rustls` + `rcgen` | TLS + Zertifikate |
| `tonic` | gRPC |
| `hickory-dns` | DNS |

### Auth & Federation

| Crate | Zweck |
|---|---|
| `openidconnect` | OIDC |
| `oauth2` | OAuth2 |
| `jsonwebtoken` | JWT |
| `activitypub_federation` | ActivityPub |
| `webfinger` | Discovery |
| `age` | Secrets |

### Plugins

| Crate | Zweck |
|---|---|
| `wasmtime` + `wasmtime-wasi` | WASM Runtime (Standard) |
| `wit-bindgen` | Component Model |
| `libloading` + `abi_stable` | Native (Ausnahme) |

### Bots & Channels (alle hinter Feature-Flags)

| Crate | Zweck |
|---|---|
| `teloxide` | Telegram |
| `matrix-sdk` | Matrix |
| `serenity` + `poise` | Discord |
| `slack-morphism` | Slack |
| `xmpp-rs` | XMPP |
| `lettre` | E-Mail |
| `irc` | IRC |

### LLM

| Crate | Zweck |
|---|---|
| `reqwest` | Ollama HTTP API, Claude API, OpenAI-kompatible APIs |
| `serde_json` | Streaming JSON-Responses |
| `tokio::sync::mpsc` | Streaming Token-Output |

### Container & Deployment

| Crate | Zweck |
|---|---|
| `bollard` | Podman/Docker API |
| `oci-distribution` | OCI Registry Client |
| `tokio-cron-scheduler` | Scheduling |
| `backon` | Retry |

### Qualität

| Crate | Zweck |
|---|---|
| `thiserror` + `anyhow` | Errors |
| `tracing` + `tracing-subscriber` | Logging |
| `opentelemetry` + `opentelemetry-otlp` | Observability |
| `rstest` + `insta` + `mockall` | Testing |
| `cargo-fuzz` | Fuzzing |
| `testcontainers` | Integration Tests |
| `cargo-deny` | License/Advisory |

---

# TEIL C — DATENMODELL (fsn-types)

```rust
pub trait ResourceMeta {
    fn id(&self) -> &Uuid;
    fn name(&self) -> &str;
    fn description(&self) -> &LocalizedString;
    fn version(&self) -> &SemVer;
    fn resource_type(&self) -> &ResourceType;
    fn created_at(&self) -> &DateTime<Utc>;
    fn updated_at(&self) -> &DateTime<Utc>;
    fn tags(&self) -> &[String];
}

pub enum ResourceType {
    Container(ContainerType),
    Service(ServiceType),
    Bot(BotType),
    Website(WebsiteType),
    File(FileType),
    Bridge(BridgeType),
}

pub enum ContainerPurpose {
    IAM, Mail, Git, Chat, Wiki, Tasks, Tickets, Maps,
    Monitoring, Database, Cache, Proxy, Custom(String),
}

pub trait Capability: Send + Sync {
    fn capability_id(&self) -> &str;
    fn api_endpoints(&self) -> Vec<ApiEndpoint>;
    fn events_emitted(&self) -> Vec<EventType>;
    fn data_formats(&self) -> Vec<DataFormat>;
}

pub trait Requirement: Send + Sync {
    fn requirement_id(&self) -> &str;
    fn required_capabilities(&self) -> Vec<String>;
    fn optional_capabilities(&self) -> Vec<String>;
}

pub struct Host {
    pub id: Uuid,
    pub hostname: String,
    pub address: HostAddress,
    pub mode: HostMode,        // Active | Passive
    pub status: HostStatus,    // Online | Offline | Degraded
    pub last_ping: Option<DateTime<Utc>>,
    pub has_desktop: bool,     // Ist Desktop installiert?
}

pub struct Permission {
    pub subject: Subject,      // User | Group | ServiceAccount | FederatedUser
    pub resource: ResourceRef,
    pub action: Action,        // Read | Write | Admin | Execute | Deploy
    pub scope: Scope,          // Local | Project | Federation
}
```

---

# TEIL D — MESSAGE BUS (fsn-bus)

## D.1 Architektur

```
PRODUZENTEN                    fsn-bus                    KONSUMENTEN
                          ┌─────────────────┐
 Git (Forgejo, Gitea)  ──►│                 │──► Matrix
 Tasks (Vikunja, ...)  ──►│  Routing Engine  │──► Telegram
 Wiki (Outline, ...)   ──►│  Transform (Tera)│──► Discord
 Chat (Matrix, ...)    ──►│  Buffer/Retry    │──► Slack
 Mail (Stalwart)       ──►│  LLM-Enrichment │──► E-Mail
 CRM (SuiteCRM, ...)   ──►│                 │──► Wiki
 ERP (ERPNext, ...)    ──►│                 │──► Datenbank
 CI/CD (Woodpecker)    ──►│                 │──► Webhook
 Monitoring (OO, ...)  ──►│                 │──► ActivityPub
 LLM (Ollama, Claude)  ──►│                 │──► Ticket-System
 Tickets (Pretix, ...) ──►│                 │──► LLM (Zusammenfassung)
 Maps (uMap)           ──►│                 │──► Audit-Log
 Auth (Kanidm, KC)     ──►│                 │──► Desktop-Notification
 Webhook (beliebig)    ──►│                 │──► Bot-Response
 Bot-Command           ──►│                 │──► Dateisystem
 Timer/Cron            ──►└─────────────────┘──► Custom (Plugin)
```

## D.2 Produzenten-Ökosystem

Der Bus ist **offen für jeden Produzenten**. Jedes Programm, das ein Event senden kann (Webhook, API-Call, AMQP, Datei), kann Produzent sein. Hier eine nicht-abschließende Liste:

### Projekt-Management & Aufgaben
Vikunja, Taiga, OpenProject, Leantime, Focalboard, Plane, WeKan, Kanboard

### CRM & ERP
SuiteCRM, ERPNext, Odoo, Dolibarr, CiviCRM, InvoiceNinja

### Git & CI/CD
Forgejo, Gitea, GitLab (self-hosted), Woodpecker CI, Drone CI, Concourse

### Wiki & Dokumentation
Outline, CryptPad, BookStack, Wiki.js (→ Wiki.rs), HedgeDoc, Docmost

### Kommunikation
Matrix (Tuwunel), Mattermost, Rocket.Chat, Zulip, XMPP

### LLM / AI
Ollama, llama.cpp, vLLM, LocalAI, Claude API, OpenAI-kompatible APIs

### Monitoring & Logs
OpenObserver, Grafana, Prometheus, Uptime Kuma

### Identität & Auth
Kanidm, KeyCloak, Authentik, LLDAP

### Ticketing & Events
Pretix, Mobilizon, Open Event

### Sonstiges
Nextcloud, Immich (Fotos), Paperless-ngx (Dokumente), n8n (Workflows)

**Jeder dieser Services braucht nur EINEN Adapter zum Bus. Der Bus routet an alle Konsumenten.**

## D.3 LLM-Integration (fsn-llm)

### Warum LLM im System?

Das LLM sitzt **innerhalb** des Systems — zwischen Mensch und Maschinen. Es ist ein Vermittler, kein externer Service. Es hat Zugriff auf den Bus, auf die Konfiguration, auf Logs — aber es sendet **keine Daten nach außen** (wenn lokal via Ollama).

### Was das LLM kann

| Funktion | Beispiel |
|---|---|
| **Natürliche Sprache → API** | "Starte Kanidm neu" → `POST /api/modules/kanidm/restart` |
| **Log-Zusammenfassung** | 5000 Zeilen Logs → "Der Fehler liegt an fehlender DB-Verbindung" |
| **Fehler-Erklärung** | Error-Code → Menschenlesbare Erklärung + Lösungsvorschlag |
| **Config-Vorschläge** | "Wie konfiguriere ich SMTP?" → Vorausgefülltes Formular |
| **Event-Enrichment** | Bus-Event + Kontext → Reichhaltigere Nachricht |
| **Bot-Responses** | Komplexe Fragen im Chat → Intelligente Antwort |
| **Dokumentation** | "Erkläre mir die Federation" → Kontextsensitive Hilfe |
| **Wizard-Assistent** | YAML-Input → "Ich sehe ein Nextcloud-Image. Brauchst du Redis dazu?" |

### fsn-llm API

```rust
pub struct LlmEngine {
    provider: Box<dyn LlmProvider>,
    system_context: SystemContext,     // Aktueller Zustand (Hosts, Module, etc.)
    bus: Arc<MessageBus>,
}

pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str, options: &LlmOptions) -> Result<String>;
    async fn stream(&self, prompt: &str, options: &LlmOptions) -> Result<TokenStream>;
    fn name(&self) -> &str;
    fn is_local(&self) -> bool;        // true = Daten bleiben im System
}

pub struct OllamaProvider { base_url: Url, model: String }
pub struct ClaudeProvider { api_key: String, model: String }
pub struct OpenAiCompatProvider { base_url: Url, api_key: Option<String>, model: String }

impl LlmEngine {
    /// Natürliche Sprache → Aktion
    pub async fn interpret_command(&self, input: &str) -> Result<InterpretedAction>;
    
    /// Event anreichern (z.B. für bessere Chat-Nachrichten)
    pub async fn enrich_event(&self, event: &BusEvent) -> Result<EnrichedEvent>;
    
    /// Logs zusammenfassen
    pub async fn summarize_logs(&self, logs: &[LogEntry], question: &str) -> Result<String>;
    
    /// Fehler erklären
    pub async fn explain_error(&self, error: &str, context: &str) -> Result<ErrorExplanation>;
    
    /// Config-Vorschlag generieren
    pub async fn suggest_config(&self, module: &str, requirements: &str) -> Result<ConfigSuggestion>;
}
```

### Sicherheit

- **Lokales LLM bevorzugt** (Ollama) — keine Daten verlassen das System
- **Cloud-LLM optional** (Claude API) — nur wenn Benutzer es explizit aktiviert
- **Kein Training auf Benutzerdaten** — LLM ist Werkzeug, nicht Lernender
- **Transparenz** — Jede LLM-Aktion wird im Audit-Log vermerkt
- **Abschaltbar** — LLM ist Optional, System funktioniert komplett ohne

---

# TEIL E — PACKAGE-MANAGER (fsn-pkg)

## E.1 OCI Distribution Spec als Standard

Jedes Paket (Plugin, Modul, Bridge, Theme, Bot-Command) folgt dem OCI-Standard.

## E.2 Package-Manifest mit API-Definition

```toml
[package]
id = "iam/kanidm"
name = "Kanidm"
version = "2.1.0"
type = "container"
purpose = "iam"

[package.capabilities]
provides = ["oidc-provider", "scim-server", "radius"]

[package.requirements]
requires = ["database/postgres"]

[package.api]
type = "rest"
base_path = "/api/v1"
auth = "oauth2"
openapi_spec = "openapi.yaml"

[package.events]
emits = ["user-created", "user-updated", "login-success", "login-failed"]
listens = ["user-provisioned"]

[[package.routes]]
event = "login-failed"
target = "matrix://!security:server.com"
template = "templates/login-failed.tera"

[package.container]
image = "docker.io/kanidm/server:latest"
healthcheck = "CMD /usr/bin/healthcheck"

[package.ui]
supports_web = true
supports_tui = false
open_mode = "iframe"
```

Bei Installation: Events werden automatisch auf dem Bus registriert, API-Kompatibilität wird geprüft, Abhängigkeiten aufgelöst.

---

# TEIL F — UI-SYSTEM (Desktop)

## F.1 Rendering-Modi

```toml
# settings.toml
[ui]
mode = "wgui"       # "wgui" | "gui" | "tui" | "web"
```

| Modus | Technologie | Beschreibung |
|---|---|---|
| **WGUI** | Dioxus Webview | Default. Volles CSS, Glassmorphism, Animationen |
| **GUI** | Dioxus Blitz | Nativer Renderer ohne Webview |
| **TUI** | Dioxus TUI | Terminal. Scrollbar, Tastatur-Navigation |
| **Web** | Dioxus WASM | Browser. Identisch mit WGUI |

## F.2 Komponenten-Bibliothek (fsn-ui)

**Alle UI-Elemente einmal definiert, überall identisch. Kein View erstellt eigene Buttons.**

```
fsn-ui/src/components/
├── button.rs           Button (Primary, Secondary, Danger, Ghost, Link)
├── icon_button.rs      Icon-Only Button
├── window.rs           Fenster (Title, Content, Footer, Scrollbar, Help)
├── modal.rs            Modal-Dialog
├── form.rs             Form-Container
├── input.rs            Text-Input (mit Validierung)
├── textarea.rs         Mehrzeilig
├── select.rs           Dropdown
├── multi_select.rs     Multi-Auswahl
├── toggle.rs           Toggle/Switch
├── checkbox.rs         Checkbox
├── radio.rs            Radio-Buttons
├── slider.rs           Slider
├── table.rs            Datentabelle (sortierbar, filterbar)
├── card.rs             Karte (für App-Launcher, Module, etc.)
├── badge.rs            Status-Badge
├── progress.rs         Fortschrittsbalken
├── spinner.rs          Lade-Animation
├── toast.rs            Toast-Notification
├── tabs.rs             Tab-Navigation
├── sidebar.rs          Sidebar-Navigation
├── status_bar.rs       Status-Leiste
├── breadcrumb.rs       Breadcrumb-Navigation
├── search_bar.rs       Suche
├── scroll_container.rs Scrollbar (automatisch wenn nötig)
├── tooltip.rs          Tooltip
├── context_menu.rs     Rechtsklick-Menü
├── app_launcher.rs     App-Karten-Grid (Home-Screen)
├── notification.rs     Benachrichtigungs-Center
├── help_panel.rs       Hilfe-Panel (F1)
├── theme_switcher.rs   Theme wechseln
├── lang_switcher.rs    Sprache wechseln
├── llm_chat.rs         LLM-Chat-Widget (eingebetteter Assistent)
└── code_block.rs       Code/Config-Anzeige mit Syntax-Highlighting
```

## F.3 Design: Glassmorphism, Kontrast, Animation

```toml
# theme.toml — FreeSynergy Dark (Default)
[theme]
name = "FreeSynergy Dark"

[colors]
primary = "#3b82f6"
primary_hover = "#2563eb"
primary_glow = "rgba(59, 130, 246, 0.4)"
accent = "#06b6d4"
accent_hover = "#0891b2"

bg_base = "#0f172a"
bg_surface = "rgba(30, 41, 59, 0.8)"
bg_glass = "rgba(30, 41, 59, 0.6)"
bg_card = "rgba(51, 65, 85, 0.5)"
bg_sidebar = "rgba(15, 23, 42, 0.95)"

text_primary = "#f1f5f9"          # Hoher Kontrast!
text_secondary = "#cbd5e1"
text_muted = "#64748b"

success = "#22c55e"
warning = "#f59e0b"
error = "#ef4444"
info = "#3b82f6"

border = "rgba(148, 163, 184, 0.2)"
border_focus = "#3b82f6"
border_glow = "rgba(59, 130, 246, 0.5)"

[effects]
glass_blur = "20px"
glass_border = "rgba(148, 163, 184, 0.15)"
shadow_glow = "0 0 20px var(--fsn-color-primary-glow)"
transition = "all 200ms cubic-bezier(0.4, 0, 0.2, 1)"
```

**CSS-Variablen-Prefix: `--fsn-`** (nicht `--fsy-`!)

Anweisungen für Website-KI: Liefere `theme.css` mit `:root { --fsn-color-*: ...; }`.
FreeSynergy konvertiert automatisch für alle Rendering-Modi.

## F.4 Desktop-Layout

```
┌───────────────────────────────────────────────────────────────┐
│  FreeSynergy.Desktop         [🎨 Theme] [🌐 DE] [─][□][✕]   │
├────────────┬──────────────────────────────────────────────────┤
│            │                                                   │
│  SIDEBAR   │  CONTENT (Fenster öffnen sich hier)              │
│            │                                                   │
│  🏠 Home   │  ┌─ Glassmorphism-Fenster ─────────────────┐    │
│  ⚙️ Admin  │  │  backdrop-filter: blur(20px)             │    │
│  📦 Store  │  │  Transparenter Hintergrund               │    │
│  🤖 Bots   │  │  Animierte Übergänge                     │    │
│  🧠 AI     │  │  Scrollbar wenn nötig                    │    │
│  ❓ Help   │  │  [OK] [Abbrechen] [Übernehmen]          │    │
│            │  └──────────────────────────────────────────┘    │
│            │                                                   │
├────────────┴──────────────────────────────────────────────────┤
│ ● Online │ 3 Hosts: 2●1○ │ 14 Mod: 13●1⚠ │ Sync ✓ │ 🔔 2  │
└───────────────────────────────────────────────────────────────┘
```

### Bereiche

| Bereich | Inhalt |
|---|---|
| **Home** | App-Launcher (installierte Programme als Karten) |
| **Admin** | Hosts, Module, Projekte, Plugins, Federation, Permissions, Backups, Logs, Settings |
| **Store** | Plugin-Browser (Suche, Filter, Install, Updates) |
| **Bots** | Bot-Commands, Channel-Konfiguration, Routing-Regeln |
| **AI** | LLM-Chat, Log-Analyse, Config-Assistent, Wizard |
| **Help** | Kontextsensitiv (F1), Suche, Einführungs-Tour |

---

# TEIL G — i18n SCHNIPSEL-SYSTEM

**Deutsch + Englisch sind Mindest-Standard. Werden immer unterstützt und zuerst implementiert.**

```
locales/{lang}/
  actions.ftl       save, delete, edit, search, confirm, cancel, next, back, ...
  nouns.ftl         module, server, project, host, plugin, store, bot, ...
  status.ftl        online, offline, error, loading, syncing, running, ...
  errors.ftl        file-not-found, invalid-config, connection-failed, ...
  phrases.ftl       select-item, confirm-delete, welcome-to, ...
  time.ftl          ago, minutes, hours, days, just-now, ...
  validation.ftl    required-field, invalid-email, too-short, ...
  help.ftl          help-dashboard, help-wizard, help-store, ...
```

Plugins bringen eigene `.ftl`-Dateien mit (mindestens `de` + `en`).

---

# TEIL H — ERROR-HANDLING, CRDT, FEDERATION, WIZARD

(Unverändert aus Plan v3/v4 — Repairable-Trait, Automerge, OIDC+SCIM+ActivityPub, Container-Assistent)

---

# TEIL I — AUFRÄUM-AKTION (ALTER CODE)

## I.1 Repos archivieren

- [ ] `FreeSynergy/UI` → Archivieren (README: "Replaced by Dioxus + fsn-ui")
- [ ] `FreeSynergy/Node.Store` → Archivieren (README: "Merged into FreeSynergy/Store")

## I.2 Code löschen in Node

| Was | Warum |
|---|---|
| **Alle `ratatui::*` Imports + Code** | Ersetzt durch Dioxus |
| **Alle `rat_salsa::*` Imports + Code** | Ersetzt durch Dioxus |
| **`fsy-tui` und `fsy-core` Dependencies** | War FreeSynergy/UI, archiviert |
| **Jinja2-Templates (`.j2`)** | Ersetzt durch Tera (nach Migration) |
| **Python-Scripts in `tools/`** | Ersetzt durch Rust-Tools |
| **`fsn-install.sh`** | Ersetzt durch Rust-Installer |
| **`store/` eingebetteter Store** | Ersetzt durch fsn-store Library |
| **Alle TUI-Widgets** | Ersetzt durch fsn-ui Komponenten |
| **Alle FormNode/RenderCtx-Abstraktionen** | Ersetzt durch Dioxus |
| **Jeder Code der `fsy-` prefixed ist** | Umbenannt in `fsn-` |

## I.3 Code migrieren

| Von | Nach | Was |
|---|---|---|
| `Node/cli/src/deploy/` | `fsn-deploy` | Quadlet-Generation, Podman-Commands |
| `Node/cli/src/wizard/` (Logik) | `fsn-wizard` | Fragen, Validierung (NICHT das UI) |
| `Node/cli/src/config/` | `fsn-config` | TOML-Loading, Schema |
| `Store/crates/store-sdk/` | `fsn-store` | Store-Client |
| `Node.Store/Node/modules/` | `Store/Node/modules/` | Modul-Definitionen |
| `Node.Store/Node/i18n/` | fsn-i18n Schnipsel | Übersetzungen zerlegen |
| Jinja-Templates | Tera-Templates | Syntax anpassen (95% identisch) |

## I.4 Umbenennen

- **Alle `fsy-` → `fsn-`** in Crate-Namen, Imports, Cargo.toml, Docs
- **Alle `--fsy-` → `--fsn-`** in CSS-Variablen
- **Alle `fsy_` → `fsn_`** in Rust-Modulnamen

---

# TEIL J — DOKUMENTATION

## J.1 Pro Library (fsn-*)

Jede Library MUSS haben:
- `README.md` — Was, Warum, Quick-Start, Beispiele
- `CHANGELOG.md` — Alle Änderungen (SemVer)
- `#[doc]` auf allen `pub` Items
- `examples/` Verzeichnis mit mindestens einem lauffähigen Beispiel
- `docs/ARCHITECTURE.md` — Interne Architektur-Entscheidungen

## J.2 Schnittstellen-Dokumentation

Für jede Schnittstelle (Bus-Events, Channel-Trait, Plugin-SDK, Bridge-SDK):
- **Was** — Was macht diese Schnittstelle?
- **Warum** — Welches Problem löst sie?
- **Wie** — Wie benutzt man sie? (Code-Beispiele)
- **Grenzen** — Was kann sie NICHT?
- **Beispiele** — Mindestens 3 konkrete Anwendungsfälle

## J.3 Vision-Dokument

`docs/VISION.md` im Lib-Repo:
- Die Idee hinter FreeSynergy
- Warum Dezentralisierung
- Warum offene Standards
- Wie die Teile zusammenspielen
- Roadmap (grob)

## J.4 API-Dokumentation

- OpenAPI-Specs für alle REST-APIs
- JSON-Schemas für alle Config-Formate
- wit-Interfaces für alle WASM-Plugins
- Bus-Event-Katalog (alle Event-Typen + Payload-Schemas)

---

# TEIL K — IMPLEMENTIERUNGS-REIHENFOLGE

## Phase 0: Setup (3-5 Tage)

- [ ] `FreeSynergy/Lib` erstellen (Cargo Workspace, alle fsn-* Crates als leere Grundstruktur)
- [ ] `FreeSynergy/Desktop` erstellen (Cargo Workspace)
- [ ] CI: GitHub Actions (build, test, clippy, fmt, cargo-deny)
- [ ] `FreeSynergy/UI` archivieren
- [ ] `FreeSynergy/Node` — Branch `v2`, alter TUI-Code markieren
- [ ] `FreeSynergy/Node.Store` → Inhalt nach Store migrieren, archivieren
- [ ] CLAUDE.md + CHANGELOG.md Workflow für alle Repos
- [ ] `docs/VISION.md` schreiben

## Phase 1: Fundament (2-3 Wochen)

- [ ] `fsn-types`: Resource, Host, Project, Module, Permission, TypeSystem, Capability
- [ ] `fsn-error`: Repairable, RepairAction, ValidationIssue
- [ ] `fsn-config`: TOML + Validierung + Auto-Repair + JSON-Schema
- [ ] `fsn-i18n`: Schnipsel-System (de + en als Standard)
- [ ] `fsn-db`: SeaORM-Setup, Basis-Entities, Migrationen, WriteBuffer
- [ ] `fsn-theme`: Theme laden, CSS-Vars, TUI-Palette
- [ ] `fsn-help`: HelpTopic, Kontext-Suche
- [ ] Tests + Docs für jede Crate

## Phase 2: CRDT + Sync (2 Wochen)

- [ ] `fsn-sync`: Automerge, SyncEngine, WebSocket-Transport
- [ ] `fsn-crypto`: age, mTLS, Key-Management
- [ ] Tests mit simulierten Peers

## Phase 3: Store + Plugins + Packages (3 Wochen)

- [ ] `fsn-store`: Store-Client (aus Store/crates/store-sdk/ migriert)
- [ ] `fsn-pkg`: Package-Manager (OCI, API-Manifests)
- [ ] `fsn-plugin-sdk`: WASM SDK (wit-bindgen)
- [ ] `fsn-plugin-runtime`: WASM Host (wasmtime)
- [ ] Plugin-Lifecycle + Event-Registration

## Phase 3.5: Message Bus + Bots + LLM (3-4 Wochen)

- [ ] `fsn-bus`: Events, Routing, Transform, Buffer
- [ ] `fsn-channel`: Channel-Trait, Matrix-Adapter, Telegram-Adapter
- [ ] `fsn-bot`: Commands, Registry, Standard-Commands
- [ ] `fsn-llm`: Provider-Trait, Ollama-Provider, interpret_command, summarize_logs

## Phase 4: Auth + Federation (3-4 Wochen)

- [ ] `fsn-auth`: OAuth2, JWT, RBAC
- [ ] `fsn-federation`: OIDC, SCIM, ActivityPub, WebFinger

## Phase 5: Container + Templates + Bridges (2-3 Wochen)

- [ ] `fsn-container`: Podman via bollard
- [ ] `fsn-template`: Tera-Wrapper
- [ ] `fsn-health`: Health-Checks
- [ ] `fsn-bridge-sdk`: Bridge-Trait
- [ ] Jinja → Tera Migration

## Phase 6: UI-Komponenten (2-3 Wochen)

- [ ] `fsn-ui`: Alle Komponenten (Button, Window, Form, ...)
- [ ] Glassmorphism, Animationen, Transitions
- [ ] TUI-Fallbacks
- [ ] Theme-System Integration
- [ ] Scrolling überall

## Phase 7: Node Application (3-4 Wochen)

- [ ] `fsn-node-core`: Node-Logik
- [ ] `fsn-deploy`: Quadlet, Zentinel (migriert)
- [ ] `fsn-host`: SSH, Remote-Install
- [ ] `fsn-wizard`: Container-Assistent
- [ ] `fsn-node-cli`: CLI + Daemon
- [ ] Alter Code aufräumen (TEIL I)

## Phase 8: Desktop Application (3-4 Wochen)

- [ ] `fsn-desktop-app`: Dioxus App Setup
- [ ] `fsn-desktop-views`: Home, Admin, Store, Bots, AI, Help, Settings
- [ ] WGUI/GUI/TUI/Web Modi
- [ ] Alle fsn-ui Komponenten integrieren

## Phase 9: Erweiterungen (ongoing)

- [ ] Weitere Channel-Adapter (Discord, Slack, XMPP, IRC, ...)
- [ ] Weitere Bus-Produzenten (ERP, CRM, CI/CD, ...)
- [ ] Erweiterte Bot-Commands
- [ ] WASM-Bridge-Plugins
- [ ] Themes im Store
- [ ] crates.io Veröffentlichung
- [ ] Wiki.rs + Decidim.rs starten (fsn-* Libraries nutzen)

---

# TEIL L — ZUSÄTZLICHE EMPFEHLUNGEN

1. **Feature Flags** — Jede Library hat granulare Features für kurze Compile-Zeiten
2. **CI/CD** — Build, Test, Clippy, Fmt, Deny, Dependabot, Nightly Fuzzing
3. **JSON-Schema** — Für alle Configs → Automatische UI-Generierung
4. **Audit-Log** — Per CRDT synchronisiert, jede Änderung
5. **SemVer pro Crate** — cargo-release für koordinierte Releases
6. **Migration** — `migration/` Skripte für alte → neue Formate
7. **MCP** — Model Context Protocol für Claude Code Integration
8. **Offline-Cache** — Store-Katalog, Configs, letzte Sync-Daten
9. **Netzwerk-Retry** — Exponential Backoff, Offline-Modus, Cache-Fallback
10. **Barrierefreiheit** — Keyboard-Navigation überall, Screen-Reader-Support
