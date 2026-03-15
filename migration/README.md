# FSN Config Migrations

Migration scripts for converting old FreeSynergy.Node config formats to newer ones.

## Available Migrations

### v1 → v2

Converts legacy YAML config files (`.yml`) to the current TOML format (`.toml`).

**Run:**
```bash
cargo run --manifest-path migration/v1_to_v2/Cargo.toml -- /path/to/fsn/root
```

**What it does:**
- Scans for `.yml` files under the given root (max depth 4)
- Converts each YAML file to TOML using serde_yaml → serde_json → toml
- Writes `{name}.toml` next to the original `.yml`
- Skips files that already have a `.toml` counterpart

**Manual steps after migration** (see script output for details):
1. Review migrated TOML files for type coercions
2. Update `[module.type]` → `[module.types]` (now an array)
3. Move project `email` under `[project.contact]`
4. Relocate module definitions to `Node.Store/modules/{type}/{name}/{name}.toml`
5. Encrypt `vault.toml` → `vault.age`
