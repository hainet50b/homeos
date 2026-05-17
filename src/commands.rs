pub mod cd;
pub mod completion;
pub mod init;
pub mod package;
pub mod plugin;

/// Detect the user's shell for spawning interactive subshells.
/// Uses `SHELL` env var if set, otherwise falls back to `pwsh` on Windows
/// or `/bin/sh` on Unix.
pub fn detect_shell() -> String {
    resolve_shell(std::env::var("SHELL").ok())
}

fn resolve_shell(shell_env: Option<String>) -> String {
    shell_env.unwrap_or_else(|| {
        if cfg!(windows) {
            "pwsh".to_string()
        } else {
            "/bin/sh".to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_shell_uses_env_var_when_set() {
        // Arrange
        let shell_env = Some("/usr/bin/fish".to_string());

        // Act
        let shell = resolve_shell(shell_env);

        // Assert
        assert_eq!(shell, "/usr/bin/fish");
    }

    #[test]
    fn test_resolve_shell_falls_back_when_env_unset() {
        // Arrange
        let shell_env = None;

        // Act
        let shell = resolve_shell(shell_env);

        // Assert
        if cfg!(windows) {
            assert_eq!(shell, "pwsh");
        } else {
            assert_eq!(shell, "/bin/sh");
        }
    }
}
