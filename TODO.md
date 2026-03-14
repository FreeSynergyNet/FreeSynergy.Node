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
- [ ] `SchemaForm` — generiert Formulare automatisch aus JSON-Schema (nutzt `schemars`)

---

## E4 — Qualität + Robustheit

- [x] Globaler Panic-Handler in `fsn-cli/src/main.rs` (tracing::error) — `fsd-app` ← Desktop
- [ ] Globaler Panic-Handler in `fsd-app` (Notification + UI-Feedback) ← Desktop
- [x] Tracing-Span-Konventionen in `fsn-types` dokumentieren (`#[instrument]`-Regeln) → `fsn-types/src/lib.rs` `tracing_conventions` mod
- [x] `FeatureFlags`-Struct in `fsn-config` (JSON-Config, zur Laufzeit lesbar) → `fsn-config/src/lib.rs`
- [ ] `insta`-Snapshot-Tests für alle Dioxus-Komponenten (`.svg` / `.html` Snapshots) ← Desktop
- [ ] `fsd-showcase` Crate — Komponenten-Galerie, nur gebaut mit `#[cfg(debug_assertions)]` ← Desktop

---

## E7 — ServiceHost / Supervisor

Zentrale Lifecycle-Komponente für alle laufenden Services. Kein verteiltes start/stop überall — ein Supervisor kennt Policies, Abhängigkeiten und Health.

- [x] `ServiceHost`-Struct in `fsn-container` → `fsn-container/src/supervisor.rs`
- [x] `start_service(id: ModuleId, config: &ServiceConfig)`
- [x] `graceful_shutdown(id: ModuleId)`
- [x] `health_check_loop()` — interner Task, aktualisiert `HealthStatus`
- [x] `RestartPolicy` enum: `Always`, `OnFailure`, `Never` + Backoff-Config
- [x] `ServiceHost` als Actor (tokio task + mpsc) statt Mutex-Struct

---

## E8 — TUI Accessibility

- [ ] Semantische Navigation: alle TUI-Screens haben konsistente Tastatur-Shortcuts + Hilfe-Overlay ← Desktop (FreeSynergy.Desktop)
- [ ] AT-SPI Support prüfen (Linux Accessibility Bus) — ratatui hat keinen nativen AT-SPI-Support; Workaround: `--no-tui` Modus mit plain-text Ausgabe via `HealthLevel::indicator_text()`
- [x] Screenreader-freundliche Ausgabe: alle Status-Symbole (✓/⚠/✗) haben Text-Fallback → `fsn-health`: `indicator_text()`, `indicator_with_text()`, AT-SPI-Notiz in Docs

---

## F — Langzeit-Vision

Nicht für nächste Sprints — aber wichtig festzuhalten, damit Architektur-Entscheidungen heute die richtige Richtung haben.

- [x] `VISION.md` schreiben: „Kein klassisches OS — dynamisch gerenderte, federierte Service-Views. Jede Interaktion ist ein Intent, geroutet zum besten Provider (lokal oder federated)"
- [ ] **Intent-Routing**: `fsn intent "show mails"` → routed zu bestem verfügbaren Mail-Modul (lokal/federated/remote) — Vorarbeit: ServiceRole-Registry (bereits vorhanden) um Intent-Mapping erweitern
- [ ] **Spatial/Card-based Desktop** als Alternative zum klassischen Window-Manager (Obsidian Canvas / Raycast-ähnlich) — erst nach stabilem Window-Manager evaluieren

---

## Reihenfolge (empfohlen)

1. **E7** ServiceHost / Supervisor — zentrales Lifecycle-Management
2. **E4** Qualität — Panic-Handler, FeatureFlags, Snapshot-Tests
3. **E8** TUI Accessibility
4. **F** Vision dokumentieren, Intent-Routing evaluieren
