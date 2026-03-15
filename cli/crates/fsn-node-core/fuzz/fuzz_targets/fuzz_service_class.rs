//! Fuzz target: parse arbitrary bytes as a ServiceClass module manifest.
//!
//! Exercises TOML deserialization of the module manifest format including
//! the custom `de_service_types` deserializer and all nested structs.
//!
//! Run (from cli/):
//!   cargo fuzz run fuzz_service_class --manifest-path crates/fsn-node-core/fuzz/Cargo.toml
#![no_main]

use fsn_node_core::config::service::ServiceClass;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Errors are expected for most inputs — we only care about panics
        let _ = toml::from_str::<ServiceClass>(s);
    }
});
