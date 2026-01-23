//! Error types for the Ganesha Vision system.

use thiserror::Error;

/// Result type alias using the Ganesha Vision error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for the Ganesha Vision system.
#[derive(Error, Debug)]
pub enum Error {
    /// Database connection or query error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// UUID parsing error
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Image processing error
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    /// Base64 decoding error
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid input or state
    #[error("Invalid: {0}")]
    Invalid(String),

    /// Database migration error
    #[error("Migration error: {0}")]
    Migration(String),

    /// Session error
    #[error("Session error: {0}")]
    Session(String),
}

impl Error {
    /// Create a not found error with a custom message.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }

    /// Create an invalid error with a custom message.
    pub fn invalid(msg: impl Into<String>) -> Self {
        Error::Invalid(msg.into())
    }

    /// Create a migration error with a custom message.
    pub fn migration(msg: impl Into<String>) -> Self {
        Error::Migration(msg.into())
    }

    /// Create a session error with a custom message.
    pub fn session(msg: impl Into<String>) -> Self {
        Error::Session(msg.into())
    }

    /// Check if this is a not found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }

    /// Check if this is a database error.
    pub fn is_database(&self) -> bool {
        matches!(self, Error::Database(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::not_found("skill 'click_button'");
        assert_eq!(err.to_string(), "Not found: skill 'click_button'");
    }

    #[test]
    fn test_error_is_not_found() {
        let err = Error::not_found("test");
        assert!(err.is_not_found());

        let err = Error::invalid("test");
        assert!(!err.is_not_found());
    }
}
