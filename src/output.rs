use clap::ValueEnum;

const ENV_VAR: &str = "HOMEOS_OUTPUT_FORMAT";

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[value(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl OutputFormat {
    pub fn resolve(output_flag: Option<OutputFormat>, json_flag: bool) -> OutputFormat {
        if let Some(format) = output_flag {
            return format;
        }
        if json_flag {
            return OutputFormat::Json;
        }
        if let Some(format) = Self::from_env() {
            return format;
        }
        OutputFormat::Text
    }

    fn from_env() -> Option<OutputFormat> {
        std::env::var_os(ENV_VAR)
            .and_then(|v| v.to_str().map(|s| s.to_string()))
            .and_then(|s| match s.as_str() {
                "text" => Some(OutputFormat::Text),
                "json" => Some(OutputFormat::Json),
                _ => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_test::EnvVarGuard;

    #[test]
    fn test_default_is_text() {
        // Arrange
        // (no setup needed)

        // Act
        let result = OutputFormat::default();

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_resolve_returns_text_when_nothing_set() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.unset();

        // Act
        let result = OutputFormat::resolve(None, false);

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_resolve_returns_json_for_output_flag() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.unset();

        // Act
        let result = OutputFormat::resolve(Some(OutputFormat::Json), false);

        // Assert
        assert_eq!(result, OutputFormat::Json);
    }

    #[test]
    fn test_resolve_returns_text_for_output_flag() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.unset();

        // Act
        let result = OutputFormat::resolve(Some(OutputFormat::Text), false);

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_resolve_returns_json_for_json_shorthand() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.unset();

        // Act
        let result = OutputFormat::resolve(None, true);

        // Assert
        assert_eq!(result, OutputFormat::Json);
    }

    #[test]
    fn test_resolve_output_flag_overrides_env_var() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("json");

        // Act
        let result = OutputFormat::resolve(Some(OutputFormat::Text), false);

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_resolve_json_flag_overrides_env_var() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("text");

        // Act
        let result = OutputFormat::resolve(None, true);

        // Assert
        assert_eq!(result, OutputFormat::Json);
    }

    #[test]
    fn test_resolve_uses_env_var_when_no_flag() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("json");

        // Act
        let result = OutputFormat::resolve(None, false);

        // Assert
        assert_eq!(result, OutputFormat::Json);
    }

    #[test]
    fn test_resolve_env_var_text_value() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("text");

        // Act
        let result = OutputFormat::resolve(None, false);

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_resolve_invalid_env_var_falls_back_to_text() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("yaml");

        // Act
        let result = OutputFormat::resolve(None, false);

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }

    #[test]
    fn test_resolve_empty_env_var_falls_back_to_text() {
        // Arrange
        let guard = EnvVarGuard::capture(ENV_VAR);
        guard.set("");

        // Act
        let result = OutputFormat::resolve(None, false);

        // Assert
        assert_eq!(result, OutputFormat::Text);
    }
}
