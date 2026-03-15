// Error types for FreeSynergy.Node.
//
// Uses FsyError from fsn-error (FreeSynergy.Lib) as the canonical error type.
// Node-specific error conditions map to FsyError variants via helper constructors
// (FsyError::config(), FsyError::not_found(), FsyError::parse(), etc.).

pub use fsn_error::FsyError;

/// Node-local alias for readability in Node-specific code.
pub type FsnError = FsyError;
