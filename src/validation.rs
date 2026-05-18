use crate::error::{HomeosError, reasons};

/// Validate a package or plugin name against the strict whitelist:
/// `^[a-z0-9][a-z0-9._-]*$`. Rejects empty strings, names starting with
/// a character outside `[a-z0-9]`, names containing characters outside
/// `[a-z0-9._-]`, and names containing the `..` substring.
///
/// This guards against shell injection, path traversal, leading-dash
/// flag confusion, and any other character class that could surprise
/// downstream filesystem or process operations.
pub fn validate_name(name: &str) -> Result<(), HomeosError> {
    if name.is_empty() {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            "Name must not be empty",
        ));
    }

    let first = name.chars().next().unwrap();
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("Name '{name}' must start with a lowercase letter or digit"),
        ));
    }

    for c in name.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-') {
            return Err(HomeosError::new(
                reasons::VALIDATION_ERROR,
                format!("Name '{name}' contains invalid character. Allowed: [a-z0-9._-]"),
            ));
        }
    }

    if name.contains("..") {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("Name '{name}' must not contain '..'"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_accepts_lowercase_alphanumeric() {
        // Arrange
        let name = "neovim";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_accepts_digits_dots_underscores_hyphens() {
        // Arrange
        let names = ["dnf-copr-mise", "a.b.c", "ab_cd", "0name", "rust1", "x"];

        // Act & Assert
        for name in names {
            assert!(validate_name(name).is_ok(), "expected '{name}' to be valid");
        }
    }

    #[test]
    fn test_validate_name_accepts_starting_digit() {
        // Arrange
        let name = "7zip";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_rejects_empty_string() {
        // Arrange
        let name = "";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must not be empty"));
    }

    #[test]
    fn test_validate_name_rejects_leading_dash() {
        // Arrange — would be misinterpreted as a flag if passed to a CLI
        let name = "-rf";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must start with"));
    }

    #[test]
    fn test_validate_name_rejects_leading_dot() {
        // Arrange — hidden-file convention, also the parent-dir prefix
        let name = ".hidden";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must start with"));
    }

    #[test]
    fn test_validate_name_rejects_leading_underscore() {
        // Arrange — pattern requires alphanumeric start
        let name = "_foo";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_uppercase() {
        // Arrange
        let name = "Foo";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_forward_slash() {
        // Arrange — path traversal via subdirectory
        let name = "foo/bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("invalid character"));
    }

    #[test]
    fn test_validate_name_rejects_backslash() {
        // Arrange — Windows path separator
        let name = "foo\\bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_whitespace() {
        // Arrange
        let cases = ["foo bar", "foo\tbar", "foo\nbar"];

        // Act & Assert
        for name in cases {
            let err = validate_name(name).unwrap_err();
            assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        }
    }

    #[test]
    fn test_validate_name_rejects_control_characters() {
        // Arrange
        let name = "foo\x01bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_nul_byte() {
        // Arrange — terminator on Unix path syscalls
        let name = "foo\0bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_double_dot_substring() {
        // Arrange — parent-directory traversal
        let name = "foo..bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("'..'"));
    }

    #[test]
    fn test_validate_name_rejects_dot_dot_alone() {
        // Arrange — bare parent-directory reference
        let name = "..";

        // Act
        let result = validate_name(name);

        // Assert
        // Rejected on leading-character rule (first char is '.', not alphanumeric)
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_non_ascii_unicode() {
        // Arrange
        let name = "café";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_percent_encoded() {
        // Arrange — common URL-encoded path traversal payload
        let name = "%2e%2e";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_special_shell_characters() {
        // Arrange — shell metacharacters
        let cases = [
            "foo;bar", "foo|bar", "foo$bar", "foo`bar", "foo&bar", "foo*bar",
        ];

        // Act & Assert
        for name in cases {
            let err = validate_name(name).unwrap_err();
            assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        }
    }

    #[test]
    fn test_validate_name_rejects_trailing_dot_dot() {
        // Arrange
        let name = "foo..";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("'..'"));
    }

    #[test]
    fn test_validate_name_accepts_single_character() {
        // Arrange
        let name = "a";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_accepts_single_digit() {
        // Arrange
        let name = "1";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }
}
