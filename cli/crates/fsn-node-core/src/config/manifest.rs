// Module plugin manifest — re-exported from fsn-plugin-sdk.
//
// All plugin protocol types live in fsn-plugin-sdk; this module provides
// a stable import path for the rest of fsn-*.

/// Re-export all plugin protocol types from `fsn-plugin-sdk`.
pub use fsn_plugin_sdk::{
    ModuleManifest,
    ManifestInputs,
    ManifestOutputFile,
    PluginContext,
    InstanceInfo,
    PeerService,
    PeerRoute,
    PluginResponse,
    OutputFile,
    ShellCommand,
    LogLine,
    LogLevel,
};
