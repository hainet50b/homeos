use crate::output::OutputFormat;
use std::fmt;
use std::io::Write;

pub mod reasons {
    #![allow(dead_code)]

    pub const PACKAGE_NOT_FOUND: &str = "package-not-found";
    pub const PLUGIN_NOT_FOUND: &str = "plugin-not-found";
    pub const ALREADY_EXISTS: &str = "already-exists";
    pub const VALIDATION_ERROR: &str = "validation-error";
    pub const CIRCULAR_DEPENDENCY: &str = "circular-dependency";
    pub const DEPENDENCY_NOT_FOUND: &str = "dependency-not-found";
    pub const DEPENDENT_EXISTS: &str = "dependent-exists";
    pub const SCRIPT_FAILED: &str = "script-failed";
    pub const SCRIPT_NOT_FOUND: &str = "script-not-found";
    pub const SCRIPT_UNMODIFIED: &str = "script-unmodified";
    pub const GIT_NOT_FOUND: &str = "git-not-found";
    pub const GIT_CLONE_FAILED: &str = "git-clone-failed";
    pub const NOT_A_VALID_HOMEOS_REPO: &str = "not-a-valid-homeos-repo";
    pub const NOT_A_VALID_HOMEOS_PLUGIN: &str = "not-a-valid-homeos-plugin";
    pub const NOT_INITIALIZED: &str = "not-initialized";
    pub const DATA_DIR_NOT_EMPTY: &str = "data-dir-not-empty";
    pub const DATA_DIR_NOT_FOUND: &str = "data-dir-not-found";
    pub const DIRECTORY_NOT_FOUND: &str = "directory-not-found";
    pub const NOT_FOUND_ON_GITHUB: &str = "not-found-on-github";
    pub const NETWORK_ERROR: &str = "network-error";
    pub const PACKAGE_INSTALLED: &str = "package-installed";
    pub const INTERNAL_ERROR: &str = "internal-error";
}

#[derive(Debug)]
pub struct HomeosError {
    pub reason: &'static str,
    pub message: String,
}

impl HomeosError {
    pub fn new(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

impl fmt::Display for HomeosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HomeosError {}

/// Resolve the canonical kebab-case reason for an error. If the error is a
/// `HomeosError`, returns its reason; otherwise falls back to `internal-error`.
pub fn reason_for(err: &(dyn std::error::Error + 'static)) -> &'static str {
    err.downcast_ref::<HomeosError>()
        .map(|e| e.reason)
        .unwrap_or(reasons::INTERNAL_ERROR)
}

/// Emit a top-level error following the dual-output contract:
/// - text mode: `Error: <message>` to stderr only
/// - json mode: `{"error": {"reason": ..., "message": ...}}` to stdout
///   AND `Error: <message>` to stderr
pub fn report_to<O: Write, E: Write>(
    err: &(dyn std::error::Error + 'static),
    format: OutputFormat,
    stdout: &mut O,
    stderr: &mut E,
) -> std::io::Result<()> {
    let message = err.to_string();
    let reason = reason_for(err);

    if format == OutputFormat::Json {
        let payload = serde_json::json!({
            "error": {
                "reason": reason,
                "message": message,
            }
        });
        writeln!(stdout, "{payload}")?;
    }
    writeln!(stderr, "Error: {message}")?;
    Ok(())
}

pub fn report(err: &(dyn std::error::Error + 'static), format: OutputFormat) {
    let _ = report_to(err, format, &mut std::io::stdout(), &mut std::io::stderr());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_homeos_error_display_returns_message() {
        // Arrange
        let sut = HomeosError::new(reasons::PACKAGE_NOT_FOUND, "Package 'foo' not found");

        // Act
        let result = sut.to_string();

        // Assert
        assert_eq!(result, "Package 'foo' not found");
    }

    #[test]
    fn test_homeos_error_carries_reason() {
        // Arrange
        let sut = HomeosError::new(reasons::PLUGIN_NOT_FOUND, "Plugin 'bar' not found");

        // Act
        let result = sut.reason;

        // Assert
        assert_eq!(result, "plugin-not-found");
    }

    #[test]
    fn test_reason_for_returns_homeos_error_reason() {
        // Arrange
        let err = HomeosError::new(reasons::PACKAGE_NOT_FOUND, "Package 'foo' not found");

        // Act
        let result = reason_for(&err);

        // Assert
        assert_eq!(result, "package-not-found");
    }

    #[test]
    fn test_reason_for_returns_internal_error_for_non_homeos_error() {
        // Arrange
        let err: Box<dyn std::error::Error> = "some random error".into();

        // Act
        let result = reason_for(err.as_ref());

        // Assert
        assert_eq!(result, "internal-error");
    }

    #[test]
    fn test_report_to_text_mode_writes_only_to_stderr() {
        // Arrange
        let err = HomeosError::new(reasons::PACKAGE_NOT_FOUND, "Package 'foo' not found");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Act
        report_to(&err, OutputFormat::Text, &mut stdout, &mut stderr).unwrap();

        // Assert
        assert!(stdout.is_empty());
        let stderr_text = String::from_utf8(stderr).unwrap();
        assert_eq!(stderr_text, "Error: Package 'foo' not found\n");
    }

    #[test]
    fn test_report_to_json_mode_writes_to_both() {
        // Arrange
        let err = HomeosError::new(reasons::PACKAGE_NOT_FOUND, "Package 'foo' not found");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Act
        report_to(&err, OutputFormat::Json, &mut stdout, &mut stderr).unwrap();

        // Assert
        let stdout_text = String::from_utf8(stdout).unwrap();
        let stderr_text = String::from_utf8(stderr).unwrap();
        let value: serde_json::Value = serde_json::from_str(stdout_text.trim()).unwrap();
        assert_eq!(value["error"]["reason"], "package-not-found");
        assert_eq!(value["error"]["message"], "Package 'foo' not found");
        assert_eq!(stderr_text, "Error: Package 'foo' not found\n");
    }

    #[test]
    fn test_report_to_json_mode_uses_internal_error_for_unknown() {
        // Arrange
        let err: Box<dyn std::error::Error> = "boom".into();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Act
        report_to(err.as_ref(), OutputFormat::Json, &mut stdout, &mut stderr).unwrap();

        // Assert
        let stdout_text = String::from_utf8(stdout).unwrap();
        let value: serde_json::Value = serde_json::from_str(stdout_text.trim()).unwrap();
        assert_eq!(value["error"]["reason"], "internal-error");
        assert_eq!(value["error"]["message"], "boom");
    }

    #[test]
    fn test_report_to_json_mode_stderr_matches_text_mode() {
        // Arrange — same error, both modes
        let err = HomeosError::new(reasons::SCRIPT_FAILED, "Script failed with exit code 1");
        let mut text_stderr = Vec::new();
        let mut json_stderr = Vec::new();
        let mut text_stdout = Vec::new();
        let mut json_stdout = Vec::new();

        // Act
        report_to(&err, OutputFormat::Text, &mut text_stdout, &mut text_stderr).unwrap();
        report_to(&err, OutputFormat::Json, &mut json_stdout, &mut json_stderr).unwrap();

        // Assert — stderr text identical across modes
        assert_eq!(text_stderr, json_stderr);
        assert!(text_stdout.is_empty());
        assert!(!json_stdout.is_empty());
    }

    #[test]
    fn test_report_to_json_escapes_special_characters_in_message() {
        // Arrange — message containing quotes, backslashes, and a newline
        let err = HomeosError::new(reasons::VALIDATION_ERROR, "bad \"quote\"\nand \\back");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Act
        report_to(&err, OutputFormat::Json, &mut stdout, &mut stderr).unwrap();

        // Assert — stdout must be valid JSON despite the special chars
        let stdout_text = String::from_utf8(stdout).unwrap();
        let value: serde_json::Value = serde_json::from_str(stdout_text.trim()).unwrap();
        assert_eq!(value["error"]["message"], "bad \"quote\"\nand \\back");
    }
}
