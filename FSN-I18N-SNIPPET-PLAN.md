# fsn-i18n — Snippet-System: Detaillierter Plan

**Ergänzung zu:** FREESYNERGY-PLAN-v5-FINAL.md  
**Betrifft:** `fsn-i18n` Library in FreeSynergy/Lib

---

## 1. Die Grundidee

Statt für jedes Projekt alle Texte neu zu übersetzen, gibt es eine **zentrale Snippet-Bibliothek** mit Standard-Bausteinen. Jedes Projekt (Node, Desktop, Wiki.rs, Decidim.rs) nutzt diese Basis und ergänzt nur projektspezifische Texte.

```
fsn-i18n (Library)
├── locales/de/actions.ftl      "Speichern", "Löschen", "Bearbeiten", ...
├── locales/de/nouns.ftl        "Modul", "Server", "Projekt", ...
├── locales/de/status.ftl       "Online", "Offline", "Fehler", ...
├── locales/de/errors.ftl       "Datei nicht gefunden: {name}", ...    ← NEU: Error-Snippets!
├── locales/de/validation.ftl   "Pflichtfeld", "Ungültige E-Mail", ...
├── locales/de/phrases.ftl      "{action} {item}?", ...
├── locales/de/time.ftl         "vor {count} Minuten", ...
├── locales/de/help.ftl         Hilfe-Texte
├── locales/en/...              (identische Struktur)
└── locales/{50 weitere}/...

Node-Projekt
├── locales/de/modules.ftl      "Zentinel", "Kanidm", ...  ← Nur Node-spezifisch!
└── locales/en/modules.ftl

Desktop-Projekt  
├── locales/de/views.ftl        "Dashboard", "App-Launcher", ...  ← Nur Desktop-spezifisch!
└── locales/en/views.ftl

Wiki.rs
├── locales/de/wiki.ftl         "Seite", "Revision", ...  ← Nur Wiki-spezifisch!
└── locales/en/wiki.ftl
```

**Ergebnis:** "Speichern", "Löschen", "Datei nicht gefunden" — einmal übersetzt, überall genutzt. Für 50 Sprachen. Für alle Projekte.

---

## 2. Error-Snippets — Ja, das macht absolut Sinn

### Warum?

Fehler sind **extrem repetitiv**. In jedem Projekt gibt es dieselben Grundfehler:
- Datei nicht gefunden
- Verbindung fehlgeschlagen
- Zugriff verweigert
- Ungültiges Format
- Timeout
- Konfigurationsfehler

Der einzige Unterschied ist ein **Detail** — ein Dateiname, eine URL, ein Feldname, eine Fehlernummer.

### Wie?

Fehler-Snippets nutzen **Fluent-Variablen** für die Details:

```ftl
# locales/de/errors.ftl

# ── Dateisystem ──────────────────────────────────────
error-file-not-found = Datei nicht gefunden: { $path }
error-file-read-failed = Datei konnte nicht gelesen werden: { $path } — { $reason }
error-file-write-failed = Datei konnte nicht geschrieben werden: { $path } — { $reason }
error-file-permission-denied = Keine Berechtigung für: { $path }
error-file-already-exists = Datei existiert bereits: { $path }
error-file-too-large = Datei zu groß: { $path } ({ $size } — Maximum: { $max })
error-dir-not-found = Verzeichnis nicht gefunden: { $path }
error-dir-not-empty = Verzeichnis ist nicht leer: { $path }

# ── Netzwerk ─────────────────────────────────────────
error-connection-failed = Verbindung zu { $target } fehlgeschlagen: { $reason }
error-connection-timeout = Zeitüberschreitung bei Verbindung zu { $target } (nach { $seconds }s)
error-connection-refused = Verbindung zu { $target } abgelehnt
error-dns-failed = DNS-Auflösung für { $host } fehlgeschlagen
error-tls-failed = TLS-Handshake mit { $host } fehlgeschlagen: { $reason }
error-http-error = HTTP-Fehler { $status } von { $url }: { $message }
error-api-error = API-Fehler von { $service }: { $message }
error-rate-limited = Zu viele Anfragen an { $service } — bitte warten ({ $seconds }s)

# ── Authentifizierung ─────────────────────────────────
error-auth-failed = Authentifizierung fehlgeschlagen: { $reason }
error-auth-token-expired = Token abgelaufen — bitte erneut anmelden
error-auth-token-invalid = Ungültiges Token: { $reason }
error-auth-permission-denied = Keine Berechtigung für { $action } auf { $resource }
error-auth-user-not-found = Benutzer nicht gefunden: { $user }
error-auth-user-locked = Benutzer gesperrt: { $user }

# ── Konfiguration ─────────────────────────────────────
error-config-invalid = Ungültige Konfiguration in { $file }: { $reason }
error-config-missing-field = Pflichtfeld fehlt in { $file }: { $field }
error-config-invalid-value = Ungültiger Wert für { $field }: "{ $value }" — erwartet: { $expected }
error-config-parse-failed = Konfiguration konnte nicht gelesen werden: { $file } — { $reason }
error-config-version-mismatch = Konfigurationsversion { $found } nicht kompatibel (erwartet: { $expected })

# ── Datenbank ─────────────────────────────────────────
error-db-connection-failed = Datenbankverbindung fehlgeschlagen: { $reason }
error-db-query-failed = Datenbankabfrage fehlgeschlagen: { $reason }
error-db-migration-failed = Migration { $version } fehlgeschlagen: { $reason }
error-db-record-not-found = Datensatz nicht gefunden: { $entity } mit ID { $id }
error-db-duplicate = Doppelter Eintrag: { $entity } "{ $value }" existiert bereits
error-db-constraint-violated = Integritätsbedingung verletzt: { $constraint }

# ── Container / Deployment ────────────────────────────
error-container-not-found = Container nicht gefunden: { $name }
error-container-start-failed = Container { $name } konnte nicht gestartet werden: { $reason }
error-container-health-failed = Healthcheck fehlgeschlagen für { $name }: { $reason }
error-image-pull-failed = Image konnte nicht heruntergeladen werden: { $image } — { $reason }

# ── Sync / CRDT ───────────────────────────────────────
error-sync-failed = Synchronisation mit { $peer } fehlgeschlagen: { $reason }
error-sync-conflict = Konflikt bei { $path } — lokale und entfernte Änderung gleichzeitig
error-sync-peer-unreachable = Peer { $peer } nicht erreichbar

# ── Plugin / Store ────────────────────────────────────
error-plugin-not-found = Plugin nicht gefunden: { $id }
error-plugin-install-failed = Installation von { $name } fehlgeschlagen: { $reason }
error-plugin-incompatible = Plugin { $name } v{ $version } nicht kompatibel (erwartet: { $expected })
error-plugin-wasm-failed = WASM-Plugin { $name } Ausführungsfehler: { $reason }
error-store-unreachable = Store nicht erreichbar: { $url } — verwende lokalen Cache

# ── Validierung (Formulare, Input) ────────────────────
error-validation-required = { $field } ist ein Pflichtfeld
error-validation-too-short = { $field } ist zu kurz (Minimum: { $min } Zeichen)
error-validation-too-long = { $field } ist zu lang (Maximum: { $max } Zeichen)
error-validation-invalid-email = Ungültige E-Mail-Adresse: { $value }
error-validation-invalid-url = Ungültige URL: { $value }
error-validation-invalid-format = Ungültiges Format für { $field }: erwartet { $format }
error-validation-out-of-range = { $field } außerhalb des gültigen Bereichs ({ $min }–{ $max })
error-validation-pattern-mismatch = { $field } entspricht nicht dem Muster: { $pattern }

# ── Allgemein ─────────────────────────────────────────
error-unknown = Unbekannter Fehler: { $message }
error-internal = Interner Fehler — bitte melden: { $code }
error-not-implemented = Funktion noch nicht implementiert: { $feature }
error-deprecated = { $feature } ist veraltet — verwende stattdessen: { $alternative }
error-operation-cancelled = Vorgang abgebrochen
error-timeout = Zeitüberschreitung bei { $operation } (nach { $seconds }s)
```

### Nutzung im Code

```rust
use fsn_i18n::I18n;

// Einfach
let msg = i18n.t("error-file-not-found", &[("path", "/etc/config.toml")]);
// → "Datei nicht gefunden: /etc/config.toml"

// Im Error-Handling
impl Display for AppError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            AppError::FileNotFound(path) => {
                write!(f, "{}", i18n.t("error-file-not-found", &[("path", path)]))
            }
            AppError::ConnectionFailed { target, reason } => {
                write!(f, "{}", i18n.t("error-connection-failed", &[
                    ("target", target),
                    ("reason", reason),
                ]))
            }
        }
    }
}

// Wiki.rs nutzt dieselben Snippets
let msg = i18n.t("error-db-record-not-found", &[
    ("entity", "WikiPage"),
    ("id", &page_id.to_string()),
]);
// → "Datensatz nicht gefunden: WikiPage mit ID 42"
```

### Erweiterbar durch andere Projekte

```ftl
# Wiki.rs ergänzt eigene Error-Snippets in wiki-errors.ftl
# Diese ERGÄNZEN die Basis, ERSETZEN sie nicht

error-wiki-page-locked = Seite "{ $title }" wird gerade bearbeitet von { $user }
error-wiki-revision-conflict = Bearbeitungskonflikt auf "{ $title }" — bitte Änderungen zusammenführen
error-wiki-export-failed = Export von "{ $title }" als { $format } fehlgeschlagen: { $reason }
```

```ftl
# Decidim.rs ergänzt eigene
error-decidim-vote-closed = Abstimmung "{ $title }" ist bereits geschlossen
error-decidim-proposal-duplicate = Ähnlicher Vorschlag existiert bereits: "{ $existing }"
```

---

## 3. Alle Snippet-Kategorien (vollständig)

| Datei | Inhalt | Beispiele | Ca. Einträge |
|---|---|---|---|
| `actions.ftl` | Verben, Aktionen | save, delete, edit, search, confirm, cancel, next, back, open, close, copy, paste, undo, redo, refresh, retry, download, upload, install, update, remove, start, stop, restart, configure, deploy, sync, login, logout | ~50 |
| `nouns.ftl` | Substantive | module, server, project, host, plugin, store, bot, user, group, role, file, folder, database, container, service, bridge, theme, language, password, token, key, certificate, backup, log, event, notification, alert, task, ticket, page, message, channel | ~60 |
| `status.ftl` | Zustände | online, offline, error, loading, syncing, running, stopped, paused, degraded, healthy, unhealthy, pending, active, inactive, locked, expired, connected, disconnected, updating, installing | ~30 |
| `errors.ftl` | Fehlermeldungen | (siehe oben) | ~60 |
| `validation.ftl` | Validierung | required, too-short, too-long, invalid-email, invalid-url, invalid-format, out-of-range, pattern-mismatch, already-exists, not-found | ~20 |
| `phrases.ftl` | Zusammengesetzte Sätze | confirm-delete, select-item, welcome-to, are-you-sure, no-results, loading-data, operation-successful, operation-failed | ~30 |
| `time.ftl` | Zeitangaben | just-now, seconds-ago, minutes-ago, hours-ago, days-ago, weeks-ago, months-ago, years-ago, today, yesterday, tomorrow | ~20 |
| `help.ftl` | Hilfe-Texte | help-getting-started, help-navigation, help-keyboard-shortcuts | ~20 |
| `labels.ftl` | UI-Labels | name, description, version, type, status, created, updated, search-placeholder, no-items, select-all, deselect-all, show-more, show-less, filter-by, sort-by, ascending, descending | ~40 |
| `confirmations.ftl` | Bestätigungen | confirm-delete-item, confirm-restart, confirm-deploy, confirm-logout, unsaved-changes | ~15 |
| `notifications.ftl` | Benachrichtigungen | saved-successfully, deleted-successfully, updated-successfully, installed-successfully, connection-restored, sync-completed | ~20 |

**Gesamt: ca. 350–400 Standard-Snippets** in der Basis-Bibliothek. Einmal übersetzen → überall nutzen.

---

## 4. Migration: Alte Sprachdateien → Snippets

### Vorher (Node.Store/Node/i18n/de.toml)

```toml
[proxy.zentinel]
name = "Zentinel Reverse Proxy"
description = "Sicherer Reverse Proxy für alle Services"
category = "Infrastruktur"

[iam.kanidm]
name = "Kanidm"
description = "Identity & Access Management"
```

### Nachher

**Basis-Snippets (fsn-i18n, für ALLE Projekte):**
```ftl
# nouns.ftl
noun-proxy = Proxy
noun-reverse-proxy = Reverse-Proxy
noun-iam = Identitäts- und Zugriffsverwaltung

# labels.ftl
label-infrastructure = Infrastruktur
label-secure = Sicher
```

**Modul-spezifisch (bleibt im Store, nicht in fsn-i18n):**
```ftl
# Store/Node/modules/proxy/zentinel/locales/de.ftl
module-zentinel-name = Zentinel Reverse Proxy
module-zentinel-description = Sicherer { noun-reverse-proxy } für alle Services
module-zentinel-category = { label-infrastructure }
```

**Die alten monolithischen Dateien werden NICHT mehr gebraucht** nachdem alles migriert ist. Sie können gelöscht werden. Der Ablauf:

1. Alte Datei lesen
2. Jeden Text analysieren: Was ist Standard? Was ist projektspezifisch?
3. Standard-Teile → fsn-i18n Snippets (falls nicht schon vorhanden)
4. Projektspezifische Teile → in Modul-Verzeichnissen als eigene .ftl
5. Alte Datei löschen
6. Testen dass alles funktioniert

### Migrations-Script (Rust)

```rust
/// Migriert alte TOML-i18n-Dateien in das Snippet-Format
pub fn migrate_i18n(old_toml: &Path, snippets_dir: &Path, module_dir: &Path) -> Result<MigrationReport> {
    let old = toml::from_str::<OldI18n>(&fs::read_to_string(old_toml)?)?;
    let mut report = MigrationReport::new();
    
    for (key, value) in old.entries() {
        if is_standard_snippet(key, value) {
            // → fsn-i18n Basis-Snippets (nur wenn noch nicht vorhanden)
            append_to_snippet_file(snippets_dir, &categorize(key), key, value)?;
            report.migrated_to_base += 1;
        } else {
            // → Modul-spezifisch
            append_to_module_file(module_dir, key, value)?;
            report.migrated_to_module += 1;
        }
    }
    
    report
}
```

---

## 5. API-Design (fsn-i18n)

```rust
pub struct I18n {
    bundles: HashMap<Lang, FluentBundle<FluentResource>>,
    fallback_chain: Vec<Lang>,   // z.B. [De, En] — wenn De fehlt, nimm En
}

impl I18n {
    /// Lädt die Basis-Snippets (aus fsn-i18n/locales/)
    pub fn load_base(lang: Lang) -> Result<Self>;
    
    /// Lädt zusätzliche Snippets (z.B. von einem Modul/Plugin)
    pub fn extend(&mut self, lang: Lang, resource: FluentResource) -> Result<()>;
    
    /// Lädt Snippets aus einem Verzeichnis (alle .ftl Dateien)
    pub fn extend_from_dir(&mut self, lang: Lang, dir: &Path) -> Result<()>;
    
    /// Einfache Übersetzung
    pub fn t(&self, key: &str) -> String;
    
    /// Mit Variablen
    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String;
    
    /// Verfügbare Sprachen
    pub fn available_languages(&self) -> Vec<Lang>;
    
    /// Prüft ob ein Key existiert
    pub fn has_key(&self, key: &str) -> bool;
    
    /// Alle Keys auflisten (für Debugging/Tooling)
    pub fn all_keys(&self) -> Vec<String>;
    
    /// Fehlende Übersetzungen finden (Vergleich zweier Sprachen)
    pub fn find_missing(&self, source: Lang, target: Lang) -> Vec<String>;
}
```

### Fallback-Kette

```rust
// Wenn Deutsch fehlt → Englisch. Wenn Englisch fehlt → Key zurückgeben.
let i18n = I18n::load_base(Lang::De)?
    .with_fallback(Lang::En);

// "error-file-not-found" existiert in DE → Deutsch
// "error-some-rare-thing" fehlt in DE → Fallback auf EN
// "error-completely-unknown" fehlt überall → gibt den Key zurück: "error-completely-unknown"
```

### Plugin-Integration

```rust
// Plugin lädt seine eigenen Snippets
let plugin_locales = plugin.locales_dir();  // z.B. "plugins/kanidm/locales/"
i18n.extend_from_dir(Lang::De, &plugin_locales.join("de"))?;
i18n.extend_from_dir(Lang::En, &plugin_locales.join("en"))?;

// Jetzt funktioniert:
i18n.t("module-kanidm-name")  // → "Kanidm" (aus Plugin)
i18n.t("error-file-not-found") // → "Datei nicht gefunden: ..." (aus Basis)
```

---

## 6. Qualitäts-Tools

### Fehlende Übersetzungen finden

```rust
// CLI-Tool oder im Desktop
let missing = i18n.find_missing(Lang::En, Lang::De);
// → ["error-some-new-thing", "label-experimental-feature"]
// Diese fehlen in Deutsch, existieren aber in Englisch
```

### Unbenutzte Snippets finden

```rust
// Vergleicht alle definierten Keys mit tatsächlicher Nutzung im Code
pub fn find_unused_keys(i18n: &I18n, source_dir: &Path) -> Vec<String>;
```

### Automatisches Snippet-Template für neue Sprachen

```bash
# Generiert leere .ftl-Dateien für eine neue Sprache basierend auf Englisch
fsn-i18n generate --from en --to fr --output locales/fr/
# → Erstellt alle .ftl-Dateien mit den englischen Texten als Kommentar
```

```ftl
# Auto-generated from en. Replace with French translations.
# Original: File not found: { $path }
error-file-not-found = File not found: { $path }
```

---

## 7. RTL/LTR — Textrichtung

### Wo gehört Textrichtung hin?

**NICHT in die Snippets.** Ein Snippet weiß nicht und muss nicht wissen, ob es von rechts nach links gelesen wird. RTL/LTR ist eine Eigenschaft der **Sprache**, nicht des einzelnen Textes.

Die Snippets selbst sind in jeder Sprache identisch aufgebaut — `error-file-not-found = ...` sieht in Arabisch strukturell genauso aus wie in Deutsch, nur der Text ist eben arabisch.

### Sprach-Metadaten (languages.toml)

Jede unterstützte Sprache hat Metadaten. Diese leben in `fsn-i18n/languages.toml`:

```toml
# REGEL: "name" ist IMMER Englisch — damit jeder es lesen kann.
#        "native_name" ist in der Sprache selbst — für die Sprecher.

[de]
name = "German"
native_name = "Deutsch"
direction = "ltr"
script = "latin"

[en]
name = "English"
native_name = "English"
direction = "ltr"
script = "latin"

[fr]
name = "French"
native_name = "Français"
direction = "ltr"
script = "latin"

[ar]
name = "Arabic"
native_name = "العربية"
direction = "rtl"
script = "arabic"

[he]
name = "Hebrew"
native_name = "עברית"
direction = "rtl"
script = "hebrew"

[fa]
name = "Persian"
native_name = "فارسی"
direction = "rtl"
script = "arabic"

[ur]
name = "Urdu"
native_name = "اردو"
direction = "rtl"
script = "arabic"

[ja]
name = "Japanese"
native_name = "日本語"
direction = "ltr"
script = "cjk"

[zh]
name = "Chinese"
native_name = "中文"
direction = "ltr"
script = "cjk"

[ko]
name = "Korean"
native_name = "한국어"
direction = "ltr"
script = "cjk"

# ... weitere Sprachen
```

### Rust-Typen

```rust
pub struct LangMeta {
    pub code: String,           // "de", "ar", "he"
    pub name: String,           // IMMER Englisch: "German", "Arabic", "Hebrew"
    pub native_name: String,    // In eigener Sprache: "Deutsch", "العربية", "עברית"
    pub direction: TextDirection,
    pub script: Script,
}

pub enum TextDirection {
    Ltr,  // Links nach Rechts (Deutsch, Englisch, Französisch, ...)
    Rtl,  // Rechts nach Links (Arabisch, Hebräisch, Persisch, Urdu, ...)
}

pub enum Script {
    Latin,      // Deutsch, Englisch, Französisch, Spanisch, ...
    Arabic,     // Arabisch, Persisch, Urdu
    Hebrew,     // Hebräisch
    Cyrillic,   // Russisch, Ukrainisch, Bulgarisch
    Cjk,        // Chinesisch, Japanisch, Koreanisch
    Devanagari, // Hindi, Marathi, Sanskrit
    Thai,       // Thai
    Georgian,   // Georgisch
    Armenian,   // Armenisch
    Greek,      // Griechisch
    Other(String),
}
```

### API

```rust
impl I18n {
    /// Metadaten der aktuellen Sprache
    pub fn current_lang(&self) -> &LangMeta;
    
    /// Ist die aktuelle Sprache RTL?
    pub fn is_rtl(&self) -> bool {
        self.current_lang().direction == TextDirection::Rtl
    }
    
    /// Alle verfügbaren Sprachen mit Metadaten
    pub fn available_languages(&self) -> Vec<&LangMeta>;
    
    /// Sprache wechseln
    pub fn set_language(&mut self, code: &str) -> Result<()>;
}
```

### Wo RTL tatsächlich wirkt: Im UI-Layer (fsn-ui + fsn-theme)

Wenn die aktive Sprache RTL ist, passiert Folgendes automatisch:

**1. HTML/CSS (WGUI + Web):**
```html
<!-- fsn-ui setzt dir="rtl" automatisch basierend auf i18n.is_rtl() -->
<html dir="rtl" lang="ar">
```

**2. CSS verwendet logische Properties (nicht physische):**
```css
/* FALSCH — bricht bei RTL */
.sidebar { margin-left: 16px; text-align: left; }

/* RICHTIG — funktioniert bei LTR und RTL automatisch */
.sidebar { margin-inline-start: 16px; text-align: start; }
```

| Physisch (NICHT verwenden) | Logisch (IMMER verwenden) |
|---|---|
| `margin-left` | `margin-inline-start` |
| `margin-right` | `margin-inline-end` |
| `padding-left` | `padding-inline-start` |
| `text-align: left` | `text-align: start` |
| `float: left` | `float: inline-start` |
| `border-left` | `border-inline-start` |

**3. Layout spiegelt sich automatisch:**
- Sidebar: Links → Rechts
- Text: Linksbündig → Rechtsbündig
- Icons mit Richtung (Pfeile, Chevrons): Werden gespiegelt
- Fortschrittsbalken: Füllung von rechts statt links

**4. TUI:**
```rust
// fsn-theme generiert die TUI-Palette mit Richtungs-Info
pub struct TuiPalette {
    // ... Farben ...
    pub direction: TextDirection,  // TUI-Framework nutzt das für Alignment
}
```

**5. Tailwind-Integration:**
Dioxus mit Tailwind hat eingebauten RTL-Support via `rtl:` Prefix:
```rust
rsx! {
    div { class: "ms-4 rtl:me-4",  // margin-start, in RTL wird margin-end
        // ...
    }
}
```

### Regeln für fsn-ui Komponenten

Jede Komponente in `fsn-ui` MUSS:
- **Logische CSS-Properties** verwenden (nie `left`/`right` für Layout)
- **`dir`-aware** sein (Icons mit Richtung spiegeln)
- Auf `i18n.is_rtl()` reagieren wenn nötig (z.B. Slider-Richtung)
- **Keine festen Ausrichtungen** annehmen

---

## 8. Zusammenfassung

| Frage | Antwort |
|---|---|
| **Alle Sprachen in Snippets übersetzen?** | Ja. Die ~50 Sprachen aus dem Store werden in das Snippet-Format migriert. |
| **Alte Sprachdateien löschen?** | Ja, nach erfolgreicher Migration. Sie werden nicht mehr gebraucht. |
| **Errors als Snippets?** | Ja, ~60 Standard-Error-Snippets mit Variablen für Details. |
| **Können andere es erweitern?** | Ja. `extend()` oder `extend_from_dir()` — Plugins bringen eigene .ftl mit. |
| **Können andere es nutzen?** | Ja. `fsn-i18n` als Dependency → 350+ fertige Snippets in 50 Sprachen. |
| **Fallback?** | Ja. DE → EN → Key als Fallback-Kette. |
| **Tools?** | Ja. find_missing(), find_unused(), generate-Template für neue Sprachen. |
| **RTL/LTR?** | In Sprach-Metadaten (`languages.toml`), NICHT in Snippets. UI spiegelt automatisch. |
| **`name` Feld?** | IMMER Englisch ("German", "Arabic"), damit es jeder lesen kann. `native_name` in eigener Sprache. |
| **CSS?** | Immer logische Properties (`margin-inline-start`, nicht `margin-left`). |
