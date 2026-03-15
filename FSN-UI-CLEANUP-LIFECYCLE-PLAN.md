# FreeSynergy — UI-Überarbeitung + Code-Aufräumung + Service-Lifecycle

**Stand:** März 2026 · **Ergänzung zu allen vorherigen Plänen

---

# TEIL 1 — DIOXUS RENDERER: WAS GEHT, WAS NICHT

## 1.1 Verfügbare Renderer (Stand Dioxus 0.7)

| Renderer | Technologie | Status | Für uns |
|---|---|---|---|
| **Desktop (Webview)** | System-Webview (WebKitGTK auf Linux) | Stabil | ← Unser WGUI-Modus |
| **Native (Blitz)** | WGPU-basiert, eigener HTML/CSS-Renderer | Experimentell | ← Unser GUI-Modus (Zukunft) |
| **Web** | WASM + DOM | Stabil | ← Unser Web-Modus |
| **Mobile** | WebView (iOS/Android) | Stabil | ← Mobile |
| **TUI** | Deprecated in 0.7 | Deprecated! | ← Problem, siehe unten |

### Zu Deiner Frage: GTK vs. KDE/Qt

Dioxus Desktop nutzt aktuell **WebKitGTK** auf Linux — das ist ein Webview, kein natives GTK-Toolkit. Es sieht nicht "GTK-typisch" aus, sondern rendert HTML/CSS wie ein Browser. Du hast volle Kontrolle über das Aussehen.

**Dioxus Native (Blitz)** nutzt **WGPU** direkt — es ist weder GTK noch Qt. Es ist ein eigener GPU-Renderer. Das ist langfristig der bessere Weg, weil es auf allen Plattformen identisch aussieht. Allerdings ist Blitz noch experimentell.

**KDE/Qt:** Dioxus hat keinen Qt-Renderer. Es gibt auch keinen Plan dafür. Aber da Dioxus im Webview-Modus vollständig CSS-gesteuert ist, kannst Du das Aussehen komplett per Theme steuern — es muss nicht "GTK" aussehen.

### TUI-Problem und Lösung

Dioxus hat den TUI-Renderer in 0.7 **deprecated**. Die offizielle Empfehlung ist, auf Blitz (Native) zu warten.

**Unsere Lösung:**
1. **Kurzfristig:** Desktop (Webview) als Standard, Web als Alternative
2. **Mittelfristig:** Blitz (Native) wenn es stabil ist
3. **TUI-Fallback:** Für Headless-Server eine separate, schlanke TUI mit `ratatui` direkt (nicht über Dioxus), die nur die Node-CLI-Funktionen abbildet — keine volle Desktop-Experience

```toml
# settings.toml
[ui]
mode = "desktop"      # "desktop" (Webview) | "native" (Blitz, experimentell) | "web" | "tui" (CLI-only)
```

## 1.2 Das X-Button-Problem

Das Problem, dass der X-Button erst nach Maximieren funktioniert, ist ein bekannter WebKitGTK-Bug mit bestimmten Fensterdekoration-Einstellungen. Fix:

```rust
// In der Dioxus Desktop-Config
let cfg = dioxus::desktop::Config::new()
    .with_window(
        WindowBuilder::new()
            .with_title("FreeSynergy Desktop")
            .with_decorations(true)        // System-Dekoration nutzen
            .with_inner_size(LogicalSize::new(1280, 800))
            .with_min_inner_size(LogicalSize::new(800, 600))
    );
```

Wenn system-Dekoration aktiv ist, funktioniert X/Minimize/Maximize vom System.

---

# TEIL 2 — FARBKONZEPT: SOFORT SICHTBAR

## 2.1 Das Problem

Schwarze Schrift auf dunkelcyan = unlesbar. Lösung: Komplett neues Farbkonzept mit hohem Kontrast.

## 2.2 Neues Theme: "Midnight Blue" (Default)

```css
:root {
    /* Hintergrund — tiefes Dunkelblau, nicht schwarz */
    --fsn-bg-base: #0c1222;
    --fsn-bg-surface: #162032;
    --fsn-bg-elevated: #1e2d45;
    --fsn-bg-sidebar: #0a0f1a;
    --fsn-bg-card: #1a2538;
    --fsn-bg-input: #0f1a2e;
    --fsn-bg-hover: #243352;

    /* Text — HELLER Kontrast auf dunklem Hintergrund */
    --fsn-text-primary: #e8edf5;       /* Fast weiß — Haupttext */
    --fsn-text-secondary: #a0b0c8;     /* Helles Blaugrau — Sekundär */
    --fsn-text-muted: #5a6e88;         /* Gedämpft */
    --fsn-text-bright: #ffffff;        /* Reinweiß — für Buttons, Hervorhebungen */

    /* Primär — Leuchtendes Blau */
    --fsn-primary: #4d8bf5;
    --fsn-primary-hover: #3a78e8;
    --fsn-primary-text: #ffffff;
    --fsn-primary-glow: rgba(77, 139, 245, 0.35);

    /* Akzent — Cyan */
    --fsn-accent: #22d3ee;
    --fsn-accent-hover: #06b6d4;

    /* Status — Leuchtend und klar unterscheidbar */
    --fsn-success: #34d399;
    --fsn-success-bg: rgba(52, 211, 153, 0.12);
    --fsn-warning: #fbbf24;
    --fsn-warning-bg: rgba(251, 191, 36, 0.12);
    --fsn-error: #f87171;
    --fsn-error-bg: rgba(248, 113, 113, 0.12);
    --fsn-info: #60a5fa;

    /* Borders — Sichtbar aber nicht aufdringlich */
    --fsn-border: rgba(148, 170, 200, 0.18);
    --fsn-border-focus: #4d8bf5;
    --fsn-border-hover: rgba(148, 170, 200, 0.3);

    /* Sidebar — Dunkler als Content */
    --fsn-sidebar-bg: #0a0f1a;
    --fsn-sidebar-text: #a0b0c8;
    --fsn-sidebar-active: #4d8bf5;
    --fsn-sidebar-active-bg: rgba(77, 139, 245, 0.15);
    --fsn-sidebar-hover-bg: rgba(255, 255, 255, 0.05);

    /* Glassmorphism */
    --fsn-glass-bg: rgba(22, 32, 50, 0.75);
    --fsn-glass-border: rgba(148, 170, 200, 0.12);
    --fsn-glass-blur: 16px;

    /* Schatten */
    --fsn-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    --fsn-shadow-glow: 0 0 24px rgba(77, 139, 245, 0.2);

    /* Animation */
    --fsn-transition: all 180ms ease;

    /* Abstände */
    --fsn-radius-sm: 6px;
    --fsn-radius-md: 10px;
    --fsn-radius-lg: 14px;

    /* Schrift */
    --fsn-font: 'Inter', system-ui, sans-serif;
    --fsn-font-mono: 'JetBrains Mono', monospace;
    --fsn-font-size: 15px;
}
```

**Kontrast-Ratio:**
- `#e8edf5` auf `#0c1222` = **14.2:1** (WCAG AAA)
- `#a0b0c8` auf `#0c1222` = **7.8:1** (WCAG AAA)
- `#4d8bf5` auf `#0c1222` = **5.6:1** (WCAG AA)

---

# TEIL 3 — DESKTOP-LAYOUT & UX

## 3.1 Sidebar-Position (konfigurierbar)

```toml
# settings.toml
[sidebar]
position = "left"         # "left" | "right" | "top" | "bottom"
collapsible = true        # Ein-/Ausziehbar
default_collapsed = false
width = 240               # Pixel (wenn left/right)
```

## 3.2 Menü-System

```
┌─ Menüleiste (oben) ─────────────────────────────────────────┐
│  FreeSynergy  │  Datei  │  Ansicht  │  Services  │  Hilfe   │
└──────────────────────────────────────────────────────────────┘

FreeSynergy:
  Über FreeSynergy...
  Einstellungen (Ctrl+,)
  ─────────────
  Beenden (Ctrl+Q)

Datei:
  Neues Projekt (Ctrl+N)
  Projekt öffnen...
  ─────────────
  Exportieren...
  Importieren...

Ansicht:
  Sidebar ein/aus (Ctrl+B)
  Sidebar-Position  ▸  Links | Rechts | Oben | Unten
  Vollbild (F11)
  Theme wechseln  ▸  Midnight Blue | Light | ...
  Rendering-Modus  ▸  Desktop | Native | Web

Services:
  Alle starten
  Alle stoppen
  ─────────────
  Service installieren... (Ctrl+I)
  Store öffnen (Ctrl+S)

Hilfe:
  Hilfe (F1)
  Tastenkürzel
  Dokumentation (öffnet Browser)
  ─────────────
  Fehler melden...
```

## 3.3 App-Launcher mit Icons, Gruppen, Seiten

### Icons: Dashboard Icons (Homarr)

Jedes Paket im Store kann ein `icon` Feld haben:

```toml
[package]
icon = "kanidm"           # → https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/svg/kanidm.svg
icon_dark = "kanidm-dark" # Für dunkle Themes (falls vorhanden)
```

Die Dashboard-Icons-Sammlung hat über 1800 Icons für populäre Services und Tools, in SVG, PNG und WEBP, mit Light- und Dark-Varianten. Icons werden beim Install gecacht.

### Gruppen (Accordion)

```
┌─ Home ──────────────────────────────────────────────┐
│                                                      │
│  ▼ Kommunikation                                    │
│    ┌─────┐  ┌─────┐  ┌─────┐                       │
│    │ 💬  │  │ 📧  │  │ 🔐  │                       │
│    │Chat │  │Mail │  │Auth │                        │
│    └─────┘  └─────┘  └─────┘                        │
│                                                      │
│  ▶ Entwicklung (eingeklappt)                        │
│                                                      │
│  ▼ Produktivität                                    │
│    ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐             │
│    │ 📝  │  │ ✅  │  │ 🗺  │  │ 📊  │             │
│    │Wiki │  │Tasks│  │Maps │  │Logs │              │
│    └─────┘  └─────┘  └─────┘  └─────┘             │
│                                                      │
│              Seite 1 / 3    [◄] [●○○] [►]           │
│                                                      │
└──────────────────────────────────────────────────────┘
```

- **Gruppen** sind per Drag & Drop konfigurierbar
- **Accordion**: Klick auf Gruppenname klappt ein/aus
- **Seiten**: Swipen (Touch/Mobile) oder Pfeile (Desktop) für weitere Seiten
- **Vollbild**: Desktop nimmt den ganzen Bildschirm ein (kein Rand)

### Apps extern öffnen (Settings)

```toml
[apps]
open_mode = "embedded"    # "embedded" (im Desktop) | "tab" (neuer Browser-Tab) | "window" (eigenes Fenster)
```

Bei "window": Dioxus kann mit `dioxus::desktop::new_window()` ein separates Fenster öffnen. Das Hauptfenster bleibt erreichbar über eine Taskleiste oder den Sidebar-Button.

## 3.4 Web-Version: Programme über/unter dem Desktop

```
┌──────────────────────────────────────────────────────┐
│  [≡ Menü]  FreeSynergy  [🔔 2]  [👤 Admin]          │  ← Immer sichtbar
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌─ Forgejo (eingebettet) ──────────────────────┐   │
│  │                                               │   │
│  │  (iFrame oder eingebettete Web-Oberfläche)    │   │
│  │                                               │   │
│  └───────────────────────────────────────────────┘   │
│                                                      │
├──────────────────────────────────────────────────────┤
│  [🏠 Home] [⚙ Admin] [📦 Store] [🤖 Bots] [❓ Help]│  ← Taskleiste (nach oben schiebbar)
└──────────────────────────────────────────────────────┘
```

Die Taskleiste kann:
- Unten fest stehen
- Nach oben geschoben werden (Swipe/Klick) um den Desktop zu zeigen
- Ausgeblendet werden (Auto-Hide)

---

# TEIL 4 — SERVICE-LIFECYCLE

## 4.1 Lifecycle-Befehle

Jeder Service hat standardisierte Lifecycle-Phasen:

```
init → install → configure → start → health-check → running
                                                        ↓
stop ← decommission ← backup ← running ← update ← running
```

| Phase | Beschreibung |
|---|---|
| **init** | Abhängigkeiten prüfen, Platz reservieren, Voraussetzungen schaffen |
| **install** | Image pullen, Quadlet generieren, Volumes erstellen |
| **configure** | Variablen setzen, Templates rendern, Secrets verschlüsseln |
| **start** | systemctl start, warten auf Healthcheck |
| **update** | Neues Image pullen, Quadlet updaten, Neustart (mit Rollback) |
| **backup** | Daten sichern bevor Änderungen |
| **migrate** | Daten von einem Service zu einem anderen übertragen |
| **swap** | Service A → Service B austauschen (mit Datenübernahme) |
| **decommission** | Service stoppen, Daten archivieren, Bus-Events deregistrieren |

## 4.2 Init-Strategien (Hooks im Paket)

```toml
[package.lifecycle]
# Was passiert bei Installation?
on_install = [
    { action = "run", command = "scripts/init-db.sh" },
    { action = "bus_emit", event = "ServiceInstalled", data = { name = "kanidm" } },
]

# Was passiert wenn ein ANDERER Service nachträglich installiert wird?
on_peer_install = [
    # Wenn ein Wiki nachträglich installiert wird, schicke Handbuch-Daten
    { trigger = "wiki.*", action = "run", command = "scripts/export-docs.sh" },
]

# Was passiert bei Update?
on_update = [
    { action = "backup", target = "data" },
    { action = "run", command = "scripts/migrate.sh" },
]

# Was passiert bei Swap (Austausch)?
on_swap = [
    { action = "export", format = "json", target = "/tmp/export" },
    # Der neue Service importiert dann von /tmp/export
]
```

## 4.3 Service-Swap (Austausch)

Wenn man z.B. von KeyCloak auf Kanidm wechseln will:

```
1. Benutzer wählt: "KeyCloak → Kanidm austauschen"
2. System prüft Capability-Kompatibilität:
   - KeyCloak bietet: OIDC ✓, SAML ✓, SCIM ✗
   - Kanidm bietet: OIDC ✓, SCIM ✓, SAML ✗
   - Warnung: "Kanidm hat kein SAML. 2 Services nutzen SAML. Trotzdem fortfahren?"
3. Export: KeyCloak-Daten werden exportiert (Users, Groups, Clients)
4. Transform: Daten werden vom KeyCloak-Format ins Kanidm-Format übersetzt
5. Kanidm wird installiert und konfiguriert
6. Import: Daten werden in Kanidm importiert
7. Alle Services die KeyCloak referenziert haben, werden auf Kanidm umgebogen
   (Die Rollen-Variablen wie iam.oidc-discovery-url werden automatisch aktualisiert)
8. KeyCloak wird gestoppt und archiviert
```

## 4.4 Nachträgliche Installation mit Aufträgen

Wenn ein Wiki nachträglich installiert wird:

```
1. Wiki wird installiert
2. Bus-Event: ServiceInstalled { type: "wiki", id: "outline" }
3. Alle bestehenden Services mit on_peer_install für "wiki.*":
   - Kanidm: "Hier sind meine SCIM-User-Daten für die Wiki-Berechtigungen"
   - Forgejo: "Hier sind meine Repo-Beschreibungen als Wiki-Seiten"
   - Node selbst: "Hier sind die Service-Handbücher als Wiki-Einträge"
4. Wiki empfängt diese Daten über den Bus und erstellt die Seiten
```

Das funktioniert, weil jedes Paket deklarieren kann: "Wenn ein Wiki installiert wird, möchte ich Daten liefern."

---

# TEIL 5 — CODE-AUFRÄUMUNG

## 5.1 Was SOFORT weg muss

Basierend auf den gemeldeten Fehlern:

| Problem | Ursache | Aktion |
|---|---|---|
| "Cannot connect to Podman: Socket not found" | Alter Code sucht `podman.sock` | **Löschen.** Conductor/Podman-Monitoring komplett entfernen. Ersetzen durch `systemctl --user status` Aufrufe. |
| "Store catalog TOML parse error" | `catalog.toml` hat einzeilige Records statt mehrzeilige | **Fixen.** TOML-Format korrigieren oder Parser toleranter machen. |
| Alter Wizard mit Docker-Referenzen | Docker-Compose-Logik die nie gebraucht wird | **Löschen.** Wizard komplett neu bauen (`fsn-wizard`). |

## 5.2 Generelle Aufräumung

**Prinzip: Im Zweifel löschen. Alles was wir brauchen, bauen wir sauber neu.**

| Kategorie | Aktion |
|---|---|
| Alles mit `podman.sock` / Socket-Referenzen | Löschen |
| Alles mit `docker` / `docker-compose` | Löschen |
| Alles mit `ratatui` / `rat-salsa` / `crossterm` (außer Node-CLI) | Löschen |
| Alles mit `fsy-` Prefix | Umbenennen in `fsn-` |
| Python-Scripts | Löschen (Rust-Ersatz in fsn-*) |
| Jinja2-Templates | Nach Tera-Migration löschen |
| Bash-Installer (`fsn-install.sh`) | Löschen nach Rust-Installer |
| Alte Wizard-Schritte die Docker referenzieren | Löschen |
| `Conductor`-Modul (Podman-Socket-basiert) | Komplett löschen und neu bauen |
| Store-Catalog mit kaputtem TOML | Fixen oder neu generieren |

## 5.3 Memory/Kontext aufräumen

Einige Konzepte aus früheren Iterationen sind veraltet:

| Alt | Neu |
|---|---|
| "Ansible-basierte Installation" | `fsn-wizard` + `fsn-deploy` (Rust-nativ) |
| "Jinja2 Templates" | Tera (Rust-nativ, Jinja2-kompatibel) |
| "Docker-Compose Parsing" | Nur als Import-Feature im Wizard, nicht als Laufzeit-Feature |
| "Podman Socket API" | Kein Socket! Quadlet + systemctl only |
| "fsy-* Crate-Naming" | Alles `fsn-*` |
| "ratatui TUI als Haupt-UI" | Dioxus Desktop (Webview) als Standard |
| "Vault für Secrets" | Typ `secret` → automatisch age-verschlüsselt |

---

# TEIL 6 — ZUSAMMENFASSUNG ALLER NEUEN PUNKTE

| # | Punkt | Status |
|---|---|---|
| 1 | Node via Desktop installierbar | ✅ Im Plan: `fsn install --gui` |
| 2 | IAM frei wählbar | ✅ Im Plan: Kanidm empfohlen, beliebige andere |
| 3 | Typ-System + Auto-Verknüpfung | ✅ Capability-Matching, Bus-basiert |
| 4 | Variablen-Rollen-Typen | ✅ `type` + `role`, auto-fill, kein Vault nötig |
| 5 | Worker/Scaling | ✅ `supports_workers` im Manifest |
| 6 | Podman ohne Socket | ✅ Quadlet + systemctl only |
| 7 | Ein Store mit Namespaces | ✅ `shared/`, `node/`, `wiki/`, ... |
| 8 | Store-Berechtigungen | ✅ Admin, Node-Admin, User, Gast |
| 9 | Dynamische Capabilities | ✅ Mehr Services = mehr Möglichkeiten |
| 10 | Typ-Berechtigungen | 📋 Später: einheitliche Permission-Oberfläche |
| 11 | Vergessenes | ✅ Recovery, Discovery, Migration, Rollback, Dry-Run |
| 12 | Claude-Code-Plan | ✅ Im vorherigen Dokument (Pakete 0–18) |
| UI | Farbkonzept | ✅ "Midnight Blue", WCAG AAA Kontrast |
| UI | Sidebar konfigurierbar | ✅ Links/Rechts/Oben/Unten, ein/ausziehbar |
| UI | Menü-System | ✅ Menüleiste mit Tastenkürzel |
| UI | Icons (Homarr) | ✅ Dashboard-Icons CDN, gecacht |
| UI | Gruppen + Accordion | ✅ Konfigurierbare Gruppen, Drag&Drop |
| UI | Seiten (Swipe) | ✅ Smartphone-Metapher, Pfeile oder Swipe |
| UI | Vollbild | ✅ Desktop füllt ganzen Bildschirm |
| UI | Web: Programme öffnen | ✅ Embedded/Tab/Window konfigurierbar |
| UI | X-Button Fix | ✅ System-Dekoration aktivieren |
| UI | Dioxus: Kein Qt/KDE | ✅ Erklärt: Webview oder Blitz (WGPU), kein Toolkit |
| UI | TUI deprecated | ✅ Lösung: Separate CLI-TUI für Headless |
| Lifecycle | Init/Update/Swap/Migrate | ✅ Standardisierte Phasen + Hooks |
| Lifecycle | Nachträgliche Installation | ✅ on_peer_install Events |
| Cleanup | Podman-Socket Code weg | ✅ Löschen, Quadlet ersetzen |
| Cleanup | Alter Wizard weg | ✅ Komplett neu bauen |
| Cleanup | TOML-Parse-Fehler fixen | ✅ Catalog-Format korrigieren |
