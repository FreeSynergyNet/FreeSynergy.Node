// schema-gen — generate JSON Schema for FSN module manifests.
//
// Usage:
//   cargo run -p fsn-node-core --example schema-gen > module.schema.json
//
// The generated schema can be used to:
//   - Validate module TOML files in editors (e.g. VS Code + Even Better TOML)
//   - Auto-generate forms in FreeSynergy.Desktop
//   - Document the module manifest format

use fsn_node_core::config::service::ServiceClass;

fn main() {
    let schema = schemars::schema_for!(ServiceClass);
    println!("{}", serde_json::to_string_pretty(&schema).expect("schema serialization"));
}
