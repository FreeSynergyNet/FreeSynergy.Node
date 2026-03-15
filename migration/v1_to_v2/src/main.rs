// migration/v1_to_v2 — Config migration script: v1 → v2 format.
//
// Run: cargo run --manifest-path migration/v1_to_v2/Cargo.toml -- <project-root>
//
// What changed in v2:
//   - modules/*.yml  → Node.Store/modules/{type}/{name}/{name}.toml
//   - projects/*.yml → projects/{slug}/{slug}.project.toml
//   - hosts/*.yml    → hosts/{slug}.host.toml
//   - vault.yml      → vault.toml (or vault.age for encrypted secrets)
//   - [module.type]  → [module.types] (array, multi-type support)
//   - `email`        → moved under [project.contact] sub-table
//   - `tags`         → TOML array syntax: tags = ["a", "b"]

use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    println!("FSN Config Migration: v1 → v2");
    println!("Root: {}", root.display());
    println!();

    let mut found = 0;
    let mut migrated = 0;

    // Scan for legacy YAML configs
    for entry in walkdir::WalkDir::new(&root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().map(|x| x == "yml").unwrap_or(false)
        })
    {
        found += 1;
        let path = entry.path();
        println!("[FOUND] {}", path.display());

        // Determine config type from path
        if let Some(parent) = path.parent() {
            let parent_name = parent.file_name().unwrap_or_default().to_string_lossy();
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();

            let new_ext = "toml";
            let new_name = format!("{stem}.{new_ext}");
            let new_path = parent.join(&new_name);

            if new_path.exists() {
                println!("  → SKIP: {} already exists", new_path.display());
                continue;
            }

            // Read YAML
            let yaml_content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("  → ERROR reading {}: {e}", path.display());
                    continue;
                }
            };

            // Convert using serde_yaml → serde_json → toml
            let value: serde_json::Value = match serde_yaml::from_str(&yaml_content) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("  → ERROR parsing YAML {}: {e}", path.display());
                    continue;
                }
            };

            let toml_string = match toml::to_string_pretty(&value) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "  → ERROR converting {} to TOML: {e}",
                        path.display()
                    );
                    continue;
                }
            };

            match std::fs::write(&new_path, &toml_string) {
                Ok(_) => {
                    println!("  → MIGRATED → {}", new_path.display());
                    migrated += 1;
                }
                Err(e) => {
                    eprintln!("  → ERROR writing {}: {e}", new_path.display());
                }
            }
        }

        let _ = parent_name; // suppress unused warning
    }

    println!();
    println!("Summary: found {found} YAML files, migrated {migrated}.");
    if found > migrated {
        println!(
            "  {} files were skipped (already have TOML equivalent or errors).",
            found - migrated
        );
    }
    println!();
    println!("Manual steps still required after migration:");
    println!("  1. Review migrated TOML files — YAML booleans/numbers may need quoting fixes");
    println!("  2. Move [module.type] = \"foo\" → [module.types] = [\"foo\"]");
    println!("  3. Move project `email` field under [project.contact]");
    println!("  4. Move module definitions to Node.Store/modules/{type}/{name}/{name}.toml");
    println!("  5. Encrypt vault.toml → vault.age with: age-keygen + age-encrypt");
}

// suppress dead code warning for parent_name — needed for clarity
#[allow(dead_code)]
fn _phantom() {}
