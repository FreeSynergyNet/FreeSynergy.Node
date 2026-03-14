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

## E3 — CSS / Theme-Erweiterungen (fsn-theme) ✓ DONE

- [x] `fsn-theme/assets/glass.css` — Glassmorphism-Klassen (`.glass`, `.glass-card`, `.glass-sidebar`)
- [x] `fsn-theme/assets/animations.css` — `fadeInUp`, `slideInRight`, Skeleton-Loading, `@media (prefers-reduced-motion)`
- [x] `theme.toml` um `[glass]`-Sektion erweitern (`bg_opacity`, `blur`, `border_opacity`)
- [x] `theme.toml` um `[animation]`-Sektion erweitern (`fast`, `base`, `slow` in ms)
- [x] `theme.toml` um `[shadows]`-Sektion erweitern (`sm`, `md`, `lg`, `xl`)
- [x] `ThemeEngine`: CSS-Variablen `--transition-fast/base/slow`, `--shadow-*` aus `theme.toml` generieren
- [x] Dark Mode via `@media (prefers-color-scheme: dark)` in generierten CSS
- [x] `@media (prefers-contrast: more)` — High-Contrast-Variante für alle Glass-Klassen
- [x] `ThemeProvider` trait: `glass()`, `shadow(level)`, `animation(kind)` → Implementierungen: `TomlTheme`, `SystemTheme`, `HighContrastTheme`

---

## E4 — Qualität + Robustheit

- [ ] Globaler Panic-Handler in `fsd-app` (tracing::error + Notification statt White Screen)
- [ ] Tracing-Span-Konventionen in `fsn-types` dokumentieren (`#[instrument]`-Regeln)
- [ ] `FeatureFlags`-Struct in `fsn-config` (JSON-Config, zur Laufzeit lesbar)
- [ ] `insta`-Snapshot-Tests für alle Dioxus-Komponenten (`.svg` / `.html` Snapshots)
- [ ] `fsd-showcase` Crate — Komponenten-Galerie, nur gebaut mit `#[cfg(debug_assertions)]`

---

## E5 — Component Library (fsn-components in FreeSynergy.Lib) ✓ DONE

- [x] Crate `fsn-components` in Lib anlegen (Dioxus + feature `desktop`/`web`)
- [x] Enum-basiertes Varianten-System: `ButtonVariant` (Primary/Secondary/Ghost/Danger), `ButtonSize` (Sm/Md/Lg)
- [x] `Button`-Komponente mit Varianten, Loading-State, Left-/Right-Icon
- [x] `Input`, `Select`, `Textarea`, `Checkbox` — einheitlich, mit `aria-*`
- [x] `FormField`-Wrapper: Label + Input + Error-Message (DRY, kein Copy-Paste mehr)
- [x] `Card`, `Badge`, `Divider`, `Spinner`, `Tooltip`
- [x] `Toast`-System: `ToastProvider` Context + `use_toast()` Hook
- [x] `ToastBus` + `ErrorBus` als globale broadcast-channel-basierte Busse
- [ ] `SchemaForm` — generiert Formulare automatisch aus JSON-Schema (nutzt `schemars`)
- [x] `fsd-showcase` verdrahtet alle Komponenten aus `fsn-components`

---

## E6 — Render-Abstraktion (fsn-render) ✓ DONE

- [x] `ViewRenderer`-Trait definieren: `render(&self, ctx: &RenderCtx)`, `handle_event(&mut self, event: UserEvent)`, `update(&mut self, delta: Duration) -> bool`
- [x] Eigenes Crate `fsn-render` (Trait + RenderCtx + UserEvent + FeatureFlags)
- [x] TUI-Implementierung: `RatatuiRenderer` (feature `tui`)
- [x] Dioxus-Implementierung: `DioxusRenderer` (feature `dioxus`)
- [x] `RenderCtx`: enthält Theme + i18n locale + FeatureFlags (injiziert, nicht global)

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

~~1. **E3** Theme-Erweiterungen~~ ✓
~~2. **E5** fsn-components anlegen~~ ✓
~~3. **E1** Desktop-Layout~~ ✓
~~4. **E2** App-Layouts~~ ✓
~~5. **E6** Render-Abstraktion~~ ✓
1. **E7** ServiceHost / Supervisor — zentrales Lifecycle-Management
2. **E4** Qualität — Panic-Handler, FeatureFlags, Snapshot-Tests
3. **E8** TUI Accessibility
4. **F** Vision dokumentieren, Intent-Routing evaluieren
