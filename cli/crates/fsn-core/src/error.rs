use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsnError {
    #[error("Config file not found: {path}")]
    ConfigNotFound { path: String },

    #[error("Config parse error in {path}: {source}")]
    ConfigParse {
        path: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("Config validation failed for {path}: {reason}")]
    ConfigInvalid { path: String, reason: String },

    #[error("Module class not found: {class}")]
    ServiceClassNotFound { class: String },

    #[error("Constraint violation: {message}")]
    ConstraintViolation { message: String },

    #[error("Template error: {0}")]
    Template(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
