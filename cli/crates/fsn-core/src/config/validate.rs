// TOML validation layer — runs before deserialization.
//
// Design Pattern: Chain of Responsibility
//   1. Size guard      — reject oversized files (DoS / memory exhaustion)
//   2. Syntax check    — parse as raw toml::Value (catches corrupt files)
//   3. Safety scan     — walk all string values for dangerous patterns
//   4. Schema check    — verify required top-level sections per file type
//
// Pattern: Strategy — `TomlKind` selects which schema rules apply.
// The caller passes the file content as a &str (already read from disk);
// validation is pure (no I/O) and returns a typed FsnError on failure.

use toml::Value;

use crate::error::FsnError;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum allowed TOML file size (128 KiB). Larger files are rejected before
/// parsing to prevent memory exhaustion from maliciously crafted input.
const MAX_BYTES: usize = 128 * 1024;

/// Maximum nesting depth for TOML tables/arrays. Prevents stack overflow from
/// deeply nested structures.
const MAX_DEPTH: usize = 12;

/// Maximum length of any single string value. Prevents memory exhaustion from
/// single fields that contain megabytes of data.
const MAX_STRING_LEN: usize = 8192;

// ── File kind ────────────────────────────────────────────────────────────────

/// The type of TOML file being validated. Used to select schema rules (Strategy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TomlKind {
    /// `*.project.toml` — must have `[project]` with `name` and `domain`.
    Project,
    /// `*.host.toml` — must have `[host]` with `name`.
    Host,
    /// `*.service.toml` — must have `[service]` with `name` and `service_class`.
    Service,
    /// `*.toml` language file — must have `[meta]` with `language`.
    Language,
    /// Any other TOML file (no schema requirements beyond syntax + safety).
    Generic,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Validate TOML `content` (a raw file string) before deserialization.
///
/// Chain: size → syntax → safety → schema.
/// Returns `Ok(())` if all checks pass, `Err(FsnError::ConfigInvalid)` otherwise.
pub fn validate_toml_content(content: &str, kind: TomlKind, path_hint: &str) -> Result<(), FsnError> {
    check_size(content, path_hint)?;
    let doc = check_syntax(content, path_hint)?;
    check_safety(&doc, path_hint, 0)?;
    check_schema(&doc, kind, path_hint)?;
    Ok(())
}

// ── Step 1: Size guard ────────────────────────────────────────────────────────

fn check_size(content: &str, path: &str) -> Result<(), FsnError> {
    if content.len() > MAX_BYTES {
        return Err(FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: format!("file too large ({} bytes, max {})", content.len(), MAX_BYTES),
        });
    }
    Ok(())
}

// ── Step 2: Syntax check ──────────────────────────────────────────────────────

fn check_syntax(content: &str, path: &str) -> Result<Value, FsnError> {
    toml::from_str::<Value>(content).map_err(|e| FsnError::ConfigInvalid {
        path: path.to_string(),
        reason: format!("TOML syntax error: {e}"),
    })
}

// ── Step 3: Safety scan ───────────────────────────────────────────────────────

/// Walk every string value in the TOML document and reject dangerous patterns.
///
/// Checks for:
/// - Null bytes (can truncate strings in C-level code)
/// - Shell injection fragments (`$(…)`, backtick, `;`, `&&`, `||`)
/// - Path traversal (`../`, `..\`)
/// - Strings exceeding MAX_STRING_LEN (memory guard)
fn check_safety(val: &Value, path: &str, depth: usize) -> Result<(), FsnError> {
    if depth > MAX_DEPTH {
        return Err(FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: "structure too deeply nested".to_string(),
        });
    }
    match val {
        Value::String(s) => check_string(s, path)?,
        Value::Table(t)  => {
            for v in t.values() {
                check_safety(v, path, depth + 1)?;
            }
        }
        Value::Array(a) => {
            for v in a {
                check_safety(v, path, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_string(s: &str, path: &str) -> Result<(), FsnError> {
    // Length guard.
    if s.len() > MAX_STRING_LEN {
        return Err(FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: format!("string value too long ({} chars, max {})", s.len(), MAX_STRING_LEN),
        });
    }
    // Null byte.
    if s.contains('\0') {
        return Err(FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: "null byte in string value".to_string(),
        });
    }
    // Shell injection — only flag unquoted subshell / compound operators.
    let shell_patterns: &[&str] = &["$(", "`", " && ", " || "];
    for pat in shell_patterns {
        if s.contains(pat) {
            return Err(FsnError::ConfigInvalid {
                path: path.to_string(),
                reason: format!("potentially unsafe pattern '{}' in string value", pat.trim()),
            });
        }
    }
    // Path traversal.
    if s.contains("../") || s.contains("..\\") {
        return Err(FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: "path traversal sequence '../' in string value".to_string(),
        });
    }
    Ok(())
}

// ── Step 4: Schema check ──────────────────────────────────────────────────────

fn check_schema(doc: &Value, kind: TomlKind, path: &str) -> Result<(), FsnError> {
    match kind {
        TomlKind::Project => require_string_field(doc, &["project", "name"], path)?,
        TomlKind::Host    => require_string_field(doc, &["host", "name"], path)?,
        TomlKind::Service => {
            require_string_field(doc, &["service", "name"], path)?;
            require_string_field(doc, &["service", "service_class"], path)?;
        }
        TomlKind::Language => require_string_field(doc, &["meta", "language"], path)?,
        TomlKind::Generic  => {}
    }
    Ok(())
}

/// Assert that `doc[keys[0]][keys[1]]…` exists and is a non-empty string.
fn require_string_field(doc: &Value, keys: &[&str], path: &str) -> Result<(), FsnError> {
    let mut cur = doc;
    for &key in keys {
        cur = cur.get(key).ok_or_else(|| FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: format!("missing required field '{}'", keys.join(".")),
        })?;
    }
    match cur.as_str() {
        Some(s) if !s.is_empty() => Ok(()),
        _ => Err(FsnError::ConfigInvalid {
            path: path.to_string(),
            reason: format!("field '{}' must be a non-empty string", keys.join(".")),
        }),
    }
}
