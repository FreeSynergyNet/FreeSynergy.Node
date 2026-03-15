// Integration test: JSON Schema generation for module manifests.
//
// Verifies that the schemars-derived JSON Schema for ServiceClass is
// well-formed and contains the expected top-level properties.

use fsn_node_core::config::service::ServiceClass;

#[test]
fn service_class_schema_generates_without_panic() {
    let schema = schemars::schema_for!(ServiceClass);
    let json = serde_json::to_string(&schema).expect("schema must serialize to JSON");
    assert!(!json.is_empty());
}

#[test]
fn service_class_schema_contains_required_properties() {
    let schema = schemars::schema_for!(ServiceClass);
    let json = serde_json::to_value(&schema).expect("schema to value");

    // Top-level definitions must exist
    let defs = &json["definitions"];
    assert!(defs.is_object(), "schema must have definitions");

    // Root object must reference ServiceClass (or be it directly)
    let title = json["title"].as_str().unwrap_or("");
    assert_eq!(title, "ServiceClass", "root title must be ServiceClass");
}

#[test]
fn service_class_schema_module_property_present() {
    let schema = schemars::schema_for!(ServiceClass);
    let json = serde_json::to_value(&schema).expect("schema to value");

    // The `module` property (mapped from `meta`) must be in the schema
    let properties = &json["properties"];
    assert!(
        properties.is_object(),
        "schema must have a properties object"
    );
    assert!(
        properties.get("module").is_some(),
        "schema must contain 'module' property (serde rename from meta)"
    );
    assert!(
        properties.get("container").is_some(),
        "schema must contain 'container' property"
    );
}
