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
// validation is pure (no I/O) and returns a typed FsyError on failure.

use toml::Value;
use fsn_error::FsyError;

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_BYTES: usize = 128 * 1024;
const MAX_DEPTH: usize = 12;
const MAX_STRING_LEN: usize = 8192;

// ── File kind ────────────────────────────────────────────────────────────────

/// The type of TOML file being validated. Selects schema rules (Strategy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TomlKind {
    Project,
    Host,
    Service,
    Language,
    Generic,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Validate TOML `content` before deserialization.
/// Chain: size → syntax → safety → schema.
pub fn validate_toml_content(content: &str, kind: TomlKind, path: &str) -> Result<(), FsyError> {
    check_size(content, path)?;
    let doc = check_syntax(content, path)?;
    check_safety(&doc, path, 0)?;
    check_schema(&doc, kind, path)?;
    Ok(())
}

// ── Step 1: Size guard ────────────────────────────────────────────────────────

fn check_size(content: &str, path: &str) -> Result<(), FsyError> {
    if content.len() > MAX_BYTES {
        return Err(FsyError::Config(format!(
            "{path}: file too large ({} bytes, max {MAX_BYTES})", content.len()
        )));
    }
    Ok(())
}

// ── Step 2: Syntax check ──────────────────────────────────────────────────────

fn check_syntax(content: &str, path: &str) -> Result<Value, FsyError> {
    toml::from_str::<Value>(content)
        .map_err(|e| FsyError::Parse(format!("{path}: TOML syntax error: {e}")))
}

// ── Step 3: Safety scan ───────────────────────────────────────────────────────

fn check_safety(val: &Value, path: &str, depth: usize) -> Result<(), FsyError> {
    if depth > MAX_DEPTH {
        return Err(FsyError::Config(format!("{path}: structure too deeply nested")));
    }
    match val {
        Value::String(s) => check_string(s, path)?,
        Value::Table(t)  => {
            for v in t.values() { check_safety(v, path, depth + 1)?; }
        }
        Value::Array(a) => {
            for v in a { check_safety(v, path, depth + 1)?; }
        }
        _ => {}
    }
    Ok(())
}

fn check_string(s: &str, path: &str) -> Result<(), FsyError> {
    if s.len() > MAX_STRING_LEN {
        return Err(FsyError::Config(format!(
            "{path}: string value too long ({} chars, max {MAX_STRING_LEN})", s.len()
        )));
    }
    if s.contains('\0') {
        return Err(FsyError::Config(format!("{path}: null byte in string value")));
    }
    for pat in &["$(", "`", " && ", " || "] {
        if s.contains(pat) {
            return Err(FsyError::Config(format!(
                "{path}: potentially unsafe pattern '{}' in string value", pat.trim()
            )));
        }
    }
    if s.contains("../") || s.contains("..\\") {
        return Err(FsyError::Config(format!("{path}: path traversal sequence '../'")));
    }
    Ok(())
}

// ── Step 4: Schema check ──────────────────────────────────────────────────────

fn check_schema(doc: &Value, kind: TomlKind, path: &str) -> Result<(), FsyError> {
    match kind {
        TomlKind::Project  => require_string_field(doc, &["project", "name"], path)?,
        TomlKind::Host     => require_string_field(doc, &["host", "name"], path)?,
        TomlKind::Service  => {
            require_string_field(doc, &["service", "name"], path)?;
            require_string_field(doc, &["service", "service_class"], path)?;
        }
        TomlKind::Language => require_string_field(doc, &["meta", "language"], path)?,
        TomlKind::Generic  => {}
    }
    Ok(())
}

fn require_string_field(doc: &Value, keys: &[&str], path: &str) -> Result<(), FsyError> {
    let field = keys.join(".");
    let mut cur = doc;
    for &key in keys {
        cur = cur.get(key).ok_or_else(|| FsyError::Config(format!(
            "{path}: missing required field '{field}'"
        )))?;
    }
    match cur.as_str() {
        Some(s) if !s.is_empty() => Ok(()),
        _ => Err(FsyError::Config(format!(
            "{path}: field '{field}' must be a non-empty string"
        ))),
    }
}
