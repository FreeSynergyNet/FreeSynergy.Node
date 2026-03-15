// Vault config – encrypted secrets store.
//
// File format: vault.age  (age armored text, git-ignored)
// Fallback:    vault.toml (plaintext, dev-mode only)
//
// Rules (per RULES.md):
//   - All keys MUST have the "vault_" prefix
//   - Never logged, never shown in Debug output
//   - Auto-generated on first install, never overwritten on re-run
//
// Encryption:
//   - age passphrase-based (scrypt KDF)
//   - Armored text format so vault.age is diff-able if stored in encrypted git
//   - plaintext vault.toml is accepted for local dev without a passphrase

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

/// Vault – all values are SecretString (zero-on-drop, never logged).
#[derive(Debug, Default)]
pub struct VaultConfig {
    values: HashMap<String, SecretString>,
}

/// Raw deserialization target – plain strings, converted to SecretString after load.
#[derive(Deserialize, Serialize, Default)]
struct RawVault(HashMap<String, String>);

impl VaultConfig {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Load vault from `vault_dir`.
    /// Tries vault.age first (needs passphrase), falls back to vault.toml (dev).
    /// Returns empty vault if neither exists.
    pub fn load(vault_dir: &Path, passphrase: Option<&str>) -> Result<Self> {
        let age_path  = vault_dir.join("vault.age");
        let toml_path = vault_dir.join("vault.toml");

        if age_path.exists() {
            let pass = passphrase.context(
                "vault.age found but no passphrase provided – \
                 set FSN_VAULT_PASS or pass --vault-pass"
            )?;
            Self::load_encrypted(&age_path, pass)
        } else if toml_path.exists() {
            Self::load_plaintext(&toml_path)
        } else {
            Ok(Self::default())
        }
    }

    /// Load plaintext vault.toml (development only – no passphrase).
    pub fn load_plaintext(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let raw: RawVault = toml::from_str(&content)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(Self::from_raw(raw))
    }

    /// Decrypt vault.age and return VaultConfig.
    pub fn load_encrypted(path: &Path, passphrase: &str) -> Result<Self> {
        let ciphertext = std::fs::read(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let plaintext = decrypt_age(&ciphertext, passphrase)
            .with_context(|| format!("decrypting {}", path.display()))?;
        let raw: RawVault = toml::from_str(&String::from_utf8(plaintext)?)
            .with_context(|| "parsing decrypted vault TOML")?;
        Ok(Self::from_raw(raw))
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    /// Encrypt vault contents and write vault.age.
    pub fn save_encrypted(&self, path: &Path, passphrase: &str) -> Result<()> {
        let toml_str  = toml::to_string(&self.to_raw())?;
        let ciphertext = encrypt_age(toml_str.as_bytes(), passphrase)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &ciphertext)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Write plaintext vault.toml (dev/init use only).
    pub fn save_plaintext(&self, path: &Path) -> Result<()> {
        let toml_str = toml::to_string(&self.to_raw())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml_str)
            .with_context(|| format!("writing {}", path.display()))
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Insert or overwrite a vault key (must have `vault_` prefix).
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let k = key.into();
        debug_assert!(k.starts_with("vault_"), "vault key must start with vault_: {k}");
        self.values.insert(k, SecretString::from(value.into()));
    }

    // ── Read access ───────────────────────────────────────────────────────────

    pub fn get(&self, key: &str) -> Option<&SecretString> {
        self.values.get(key)
    }

    /// Expose a secret for template rendering. Use sparingly.
    pub fn expose(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.expose_secret())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn from_raw(raw: RawVault) -> Self {
        Self {
            values: raw.0.into_iter()
                .map(|(k, v)| (k, SecretString::from(v)))
                .collect(),
        }
    }

    fn to_raw(&self) -> RawVault {
        RawVault(
            self.values.iter()
                .map(|(k, v)| (k.clone(), v.expose_secret().to_owned()))
                .collect(),
        )
    }
}

/// Never serialize vault values (emit empty map for safety).
impl Serialize for VaultConfig {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        s.serialize_map(Some(0))?.end()
    }
}

// ── age crypto helpers ────────────────────────────────────────────────────────

fn encrypt_age(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    use age::armor::{ArmoredWriter, Format};

    let pass = age::secrecy::SecretString::new(passphrase.to_owned());
    let encryptor = age::Encryptor::with_user_passphrase(pass);

    let mut output = Vec::new();
    {
        let armor   = ArmoredWriter::wrap_output(&mut output, Format::AsciiArmor)?;
        let mut writer = encryptor.wrap_output(armor)?;
        writer.write_all(plaintext)?;
        let armor = writer.finish()?;
        armor.finish()?;
    }
    Ok(output)
}

fn decrypt_age(ciphertext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    use age::armor::ArmoredReader;

    let pass    = age::secrecy::SecretString::new(passphrase.to_owned());
    let armored = ArmoredReader::new(ciphertext);

    let decryptor = match age::Decryptor::new(armored)? {
        age::Decryptor::Passphrase(d) => d,
        _ => bail!("vault.age was not encrypted with a passphrase"),
    };

    let mut reader   = decryptor.decrypt(&pass, None)?;
    let mut plaintext = Vec::new();
    reader.read_to_end(&mut plaintext)?;
    Ok(plaintext)
}
