# FreeSynergy — Master TODO

Stand: 2026-03

---

## Deferred (low priority, kein Blocker)

- [ ] `examples/` Verzeichnis in jeder Lib-Crate
- [ ] Store-i18n: 51 Sprachen prüfen (Inhalte vollständig?)
- [ ] Fehlermeldungen in allen CLI-Commands
- [ ] `fsn-bridge-sdk`: noch kein Bedarf, komplett leer
- [ ] `migration/` Verzeichnis mit Skripten für v1 → v2 Config-Migration
- [ ] JSON-Schema für alle Modul-Manifeste (Validierung + UI-Generierung)

---

## E3 — CSS / Theme-Erweiterungen (fsn-theme)

Neue Theme-Assets und `theme.toml`-Sektionen für Glass + Animation.

- [ ] `fsn-theme/assets/glass.css` — Glassmorphism-Klassen (`.glass`, `.glass-card`, `.glass-sidebar`)
- [ ] `fsn-theme/assets/animations.css` — `fadeInUp`, `slideInRight`, Skeleton-Loading, `@media (prefers-reduced-motion)`
- [ ] `theme.toml` um `[glass]`-Sektion erweitern (`bg_opacity`, `blur`, `border_opacity`)
- [ ] `theme.toml` um `[animation]`-Sektion erweitern (`fast`, `base`, `slow` in ms)
- [ ] `theme.toml` um `[shadows]`-Sektion erweitern (`sm`, `md`, `lg`, `xl`)
- [ ] `ThemeEngine`: CSS-Variablen `--transition-fast/base/slow`, `--shadow-*` aus `theme.toml` generieren
- [ ] Dark Mode via `@media (prefers-color-scheme: dark)` in generierten CSS
- [ ] `@media (prefers-contrast: more)` — High-Contrast-Variante für alle Glass-Klassen
- [ ] `ThemeProvider` trait: `glass()`, `shadow(level)`, `animation(kind)` → Implementierungen: `TomlTheme`, `SystemTheme`, `HighContrastTheme`

---

## E4 — Qualität + Robustheit

- [ ] Globaler Panic-Handler in `fsd-app` (tracing::error + Notification statt White Screen)
- [ ] Tracing-Span-Konventionen in `fsn-types` dokumentieren (`#[instrument]`-Regeln)
- [ ] `FeatureFlags`-Struct in `fsn-config` (JSON-Config, zur Laufzeit lesbar)
- [ ] `insta`-Snapshot-Tests für alle Dioxus-Komponenten (`.svg` / `.html` Snapshots)
- [ ] `fsd-showcase` Crate — Komponenten-Galerie, nur gebaut mit `#[cfg(debug_assertions)]`

---

## E5 — Component Library (fsn-components in FreeSynergy.Lib)

Neues Crate `fsn-components` — alle UI-Primitives einmal definiert, überall genutzt.

- [ ] Crate `fsn-components` in Lib anlegen (Dioxus + feature `desktop`/`web`)
- [ ] Enum-basiertes Varianten-System: `ButtonVariant` (Primary/Secondary/Ghost/Danger), `ButtonSize` (Sm/Md/Lg)
- [ ] `Button`-Komponente mit Varianten, Loading-State, Left-/Right-Icon
- [ ] `Input`, `Select`, `Textarea`, `Checkbox` — einheitlich, mit `aria-*`
- [ ] `FormField`-Wrapper: Label + Input + Error-Message (DRY, kein Copy-Paste mehr)
- [ ] `Card`, `Badge`, `Divider`, `Spinner`, `Tooltip`
- [ ] `Toast`-System: `ToastProvider` Context + `use_toast()` Hook (ersetzt separate Notification-Impl in fsd-shell)
- [ ] `ToastBus` + `ErrorBus` als globale `mpsc::channel`-basierte Busse — können auch aus non-Dioxus-Code (CLI, Background-Services) gesendet werden
- [ ] `SchemaForm` — generiert Formulare automatisch aus JSON-Schema (nutzt `schemars`)
- [ ] `fsd-showcase` verdrahtet alle Komponenten aus `fsn-components`

---

## E6 — Render-Abstraktion (fsn-render)

`ViewRenderer`-Trait als gemeinsame Abstraktionsschicht für TUI (ratatui), Desktop (Dioxus) und späteres Web/Mobile. Business-Logik bleibt renderer-agnostisch.

- [ ] `ViewRenderer`-Trait definieren: `render(&self, ctx: &RenderCtx)`, `handle_event(&mut self, event: UserEvent)`, `update(&mut self, delta: Duration) -> bool`
- [ ] Trait in `fsn-types` oder eigenem Crate `fsn-render` (je nach Größe entscheiden)
- [ ] TUI-Implementierung: `RatatuiRenderer` wraps bestehende `FormNode`-Logik
- [ ] Dioxus-Implementierung: `DioxusRenderer` als Wrapper für fsd-* Komponenten
- [ ] `RenderCtx`: enthält Theme + i18n + FeatureFlags (injiziert, nicht global)

---

## E7 — ServiceHost / Supervisor

Zentrale Lifecycle-Komponente für alle laufenden Services. Kein verteiltes start/stop überall — ein Supervisor kennt Policies, Abhängigkeiten und Health.

- [ ] `ServiceHost`-Struct in `fsn-container` oder `fsn-core`
- [ ] `start_service(id: ModuleId, config: &ServiceConfig)`
- [ ] `graceful_shutdown(id: ModuleId)`
- [ ] `health_check_loop()` — interner Task, aktualisiert `HealthStatus`
- [ ] `RestartPolicy` enum: `Always`, `OnFailure`, `Never` + Backoff-Config
- [ ] `ServiceHost` als Actor (tokio task + mpsc) statt Mutex-Struct

---

## E8 — TUI Accessibility

- [ ] Semantische Navigation: alle TUI-Screens haben konsistente Tastatur-Shortcuts + Hilfe-Overlay
- [ ] AT-SPI Support prüfen (Linux Accessibility Bus) — falls machbar mit ratatui
- [ ] Screenreader-freundliche Ausgabe: alle Status-Symbole (✓/⚠/✗) haben Text-Fallback

---

## F — Langzeit-Vision

Nicht für nächste Sprints — aber wichtig festzuhalten, damit Architektur-Entscheidungen heute die richtige Richtung haben.

- [ ] `VISION.md` schreiben: „Kein klassisches OS — dynamisch gerenderte, federierte Service-Views. Jede Interaktion ist ein Intent, geroutet zum besten Provider (lokal oder federated)"
- [ ] **Intent-Routing**: `fsn intent "show mails"` → routed zu bestem verfügbaren Mail-Modul (lokal/federated/remote) — Vorarbeit: ServiceRole-Registry (bereits vorhanden) um Intent-Mapping erweitern
- [ ] **Spatial/Card-based Desktop** als Alternative zum klassischen Window-Manager (Obsidian Canvas / Raycast-ähnlich) — erst nach stabilem Window-Manager evaluieren

---

## Reihenfolge (empfohlen)

1. **E3** Theme-Erweiterungen — Fundament (glass + animations CSS, ThemeProvider trait, neue theme.toml-Sektionen)
2. **E5** fsn-components anlegen — Button, Input, FormField, ToastBus (erste 5 Komponenten + fsd-showcase)
3. **E1** Desktop-Layout — CSS Grid Shell, WindowFrame mit Glass, SplitView
4. **E2** App-Layouts — AppShell + 3 Standard-Layouts
5. **E6** Render-Abstraktion — ViewRenderer trait (bevor GUI-Komplexität explodiert)
6. **E7** ServiceHost / Supervisor — zentrales Lifecycle-Management
7. **E4** Qualität — Panic-Handler, FeatureFlags, Snapshot-Tests
8. **E8** TUI Accessibility
9. **F** Vision dokumentieren, Intent-Routing evaluieren
